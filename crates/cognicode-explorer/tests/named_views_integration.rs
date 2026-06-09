//! End-to-end integration tests for the `named-views` feature.
//!
//! The file is `#[cfg(all(test, feature = "postgres"))]`-gated:
//! without the `postgres` feature, this file compiles to nothing
//! and no `sqlx` enters the test dep graph.
//!
//! Per-test isolation reuses the pattern from
//! `tests/pg_bridge_contract.rs`: every test gets a uniquely-named
//! PG database (drop-then-create), so the suite runs in parallel
//! without shared-state interference.
//!
//! Prerequisite: set `TEST_DATABASE_URL` to a base URL like
//! `postgres://user:pass@host:5432`. When the env var is unset
//! every test prints a skip message and exits early.

#![cfg(all(test, feature = "postgres"))]

use std::sync::atomic::{AtomicU64, Ordering};

use cognicode_core::domain::aggregates::{CallGraph, Symbol};
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::persistence::{NamedViewRow, PostgresRepository};
use cognicode_explorer::adapters::{CallGraphRepository, FsSourceReader};
use cognicode_explorer::dto::{NamedView, NamedViewDescriptor};
use cognicode_explorer::error::ExplorerError;
use cognicode_explorer::ports::symbol_repository::SymbolRepository;
use cognicode_explorer::service::ExplorerService;
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(feature = "postgres")]
use sqlx::PgPool;

// Per-process counter so every test gets a unique DB name.
static UNIQ: AtomicU64 = AtomicU64::new(0);

/// Build a fresh per-test PostgreSQL database: drop-then-create,
/// connect, and run the embedded schema. Returns the unique test
/// URL and the pool. Returns `None` when `TEST_DATABASE_URL` is
/// unset — every test then skips cleanly.
async fn fresh_test_url() -> Option<(String, PgPool)> {
    let base = std::env::var("TEST_DATABASE_URL").ok()?;
    let n = UNIQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let db_name = format!("cognicode_view_test_{pid}_{n}");

    let admin_url = base.clone();
    let test_url = rewrite_db_name(&admin_url, &db_name);

    // Drop (defensive) then create the unique DB.
    let admin = sqlx::PgPool::connect(&admin_url).await.ok()?;
    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{db_name}\""))
        .execute(&admin)
        .await;
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .ok()?;

    // Connect to the new DB and run the embedded migrations.
    let pool = sqlx::PgPool::connect(&test_url).await.ok()?;
    let repo = PostgresRepository::from_pool(pool.clone());
    repo.run_migrations().await.ok()?;

    Some((test_url, pool))
}

fn rewrite_db_name(url: &str, new_name: &str) -> String {
    if let Some(at_idx) = url.rfind('@') {
        let (head, tail) = url.split_at(at_idx);
        if let Some(slash_idx) = tail.find('/') {
            let (host, _) = tail.split_at(slash_idx);
            return format!("{head}{host}/{new_name}");
        }
    }
    let trimmed = url.trim_end_matches('/');
    format!("{trimmed}/{new_name}")
}

/// Tiny macro: prints a skip message and returns early when
/// `TEST_DATABASE_URL` is not set.
macro_rules! pg_test {
    ($name:ident, |$url:ident: String, $pool:ident: PgPool| $body:tt) => {
        #[tokio::test]
        async fn $name() {
            let Some(($url, $pool)) = fresh_test_url().await else {
                eprintln!("skipping {}: TEST_DATABASE_URL not set", stringify!($name));
                return;
            };
            async fn inner($url: String, $pool: PgPool) {
                $body
            }
            inner($url, $pool).await
        }
    };
}

/// Build a `CallGraph` with one focusable symbol that the
/// `view_load` re-build path can resolve. The graph carries
/// `alpha` (focus) and `beta` (callee) plus a single edge.
fn build_one_symbol_graph() -> CallGraph {
    let mut g = CallGraph::new();
    let alpha = g.add_symbol(Symbol::new(
        "alpha",
        SymbolKind::Function,
        Location::new("src/a.rs", 1, 0),
    ));
    let beta = g.add_symbol(Symbol::new(
        "beta",
        SymbolKind::Function,
        Location::new("src/b.rs", 1, 0),
    ));
    g.add_dependency_with_provenance(
        &alpha,
        &beta,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    )
    .expect("alpha->beta");
    g
}

/// Build an `ExplorerService` wired with the freshly-migrated
/// `PostgresRepository` AND the in-memory call graph so the
/// `view_load` rebuild can call `contextual_view` against real
/// data.
fn build_service_with_pg(
    pool: PgPool,
    graph: Arc<CallGraph>,
) -> (Arc<ExplorerService>, Arc<PostgresRepository>) {
    let repo = Arc::new(PostgresRepository::from_pool(pool));
    let reader = Arc::new(FsSourceReader::new(PathBuf::from("/tmp")));
    let graph_repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph));
    let service =
        Arc::new(ExplorerService::new(graph_repo, reader, "/tmp").with_postgres_repo(repo.clone()));
    (service, repo)
}

// =================================================================
// Tests
// =================================================================

// Save: happy path returns `Ok(NamedView)`, row visible via repo.
pg_test!(
    view_save_happy_path_persists_row,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, repo) = build_service_with_pg(pool, graph);

        let view = service
            .save_view(
                "w1",
                "u1",
                "hotspots",
                Some("saved hotspots"),
                "function",
                "callgraph",
                "symbol:src/a.rs:alpha:1",
                3,
            )
            .await
            .expect("save must succeed");
        assert_eq!(view.workspace_id, "w1");
        assert_eq!(view.owner, "u1");
        assert_eq!(view.name, "hotspots");
        assert_eq!(view.level, "function");
        assert_eq!(view.lens, "callgraph");
        assert_eq!(view.focus_node, "symbol:src/a.rs:alpha:1");
        assert_eq!(view.max_depth, 3);
        assert!(!view.id.is_empty(), "server-generated id must be non-empty");
        assert!(!view.created_at.is_empty(), "created_at must be set");

        // Verify the row is actually in PG (independent of the service path).
        let row: Option<NamedViewRow> = sqlx::query_as(
        "SELECT id, workspace_id, owner, name, description, level, lens, focus_node, max_depth, created_at::text AS created_at FROM named_views WHERE id = $1"
    )
    .bind(&view.id)
    .fetch_optional(repo.pool())
    .await
    .expect("raw select must succeed");
        let row = row.expect("row must be present after save");
        assert_eq!(row.name, "hotspots");
        assert_eq!(row.description.as_deref(), Some("saved hotspots"));
    }
);

// Save: duplicate (workspace_id, owner, name) returns Conflict.
pg_test!(
    view_save_duplicate_returns_conflict,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, _repo) = build_service_with_pg(pool, graph);

        service
            .save_view(
                "w1",
                "u1",
                "hotspots",
                None,
                "function",
                "callgraph",
                "x",
                3,
            )
            .await
            .expect("first save ok");
        let err = service
            .save_view(
                "w1",
                "u1",
                "hotspots",
                None,
                "function",
                "callgraph",
                "x",
                3,
            )
            .await
            .expect_err("second save must fail");
        assert!(
            matches!(err, ExplorerError::Conflict(_)),
            "expected Conflict, got: {err:?}"
        );
    }
);

// Save: empty `name` is rejected at validation BEFORE PG is touched.
pg_test!(
    view_save_rejects_empty_name,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, repo) = build_service_with_pg(pool, graph);

        let err = service
            .save_view("w1", "u1", "", None, "function", "callgraph", "x", 3)
            .await
            .expect_err("empty name must be rejected");
        assert!(
            matches!(err, ExplorerError::InvalidInput(_)),
            "expected InvalidInput, got: {err:?}"
        );

        // Confirm no row was inserted.
        let n: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM named_views")
            .fetch_one(repo.pool())
            .await
            .expect("count must succeed")
            .0;
        assert_eq!(n, 0, "no row should be inserted on validation failure");
    }
);

// Save: negative `max_depth` is rejected at validation.
pg_test!(
    view_save_rejects_negative_max_depth,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, _repo) = build_service_with_pg(pool, graph);

        let err = service
            .save_view("w1", "u1", "n", None, "function", "callgraph", "x", -1)
            .await
            .expect_err("negative max_depth must be rejected");
        assert!(
            matches!(err, ExplorerError::InvalidInput(_)),
            "expected InvalidInput, got: {err:?}"
        );
    }
);

// Save: 201-char name is rejected.
pg_test!(
    view_save_rejects_name_too_long,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, _repo) = build_service_with_pg(pool, graph);
        let long_name = "x".repeat(201);

        let err = service
            .save_view(
                "w1",
                "u1",
                &long_name,
                None,
                "function",
                "callgraph",
                "x",
                3,
            )
            .await
            .expect_err("201-char name must be rejected");
        assert!(
            matches!(err, ExplorerError::InvalidInput(_)),
            "expected InvalidInput, got: {err:?}"
        );
    }
);

// Load: re-invokes contextual_view and returns Ok(ContextualView).
pg_test!(
    view_load_returns_rebuilt_view,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, _repo) = build_service_with_pg(pool, graph);

        let saved = service
            .save_view(
                "w1",
                "u1",
                "deps",
                None,
                "function",
                "callgraph",
                "symbol:src/a.rs:alpha:1",
                3,
            )
            .await
            .expect("save must succeed");

        let rebuilt = service
            .load_view(&saved.id, "w1", "u1")
            .await
            .expect("load must succeed");
        assert_eq!(rebuilt.view_id, "callgraph");
        assert!(
            !rebuilt.blocks.is_empty(),
            "callgraph lens must emit blocks"
        );
    }
);

// Load: unknown id returns NotFound.
pg_test!(
    view_load_unknown_id_returns_not_found,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, _repo) = build_service_with_pg(pool, graph);

        let err = service
            .load_view("00000000-0000-0000-0000-000000000000", "w1", "u1")
            .await
            .expect_err("unknown id must error");
        assert!(
            matches!(err, ExplorerError::NotFound(_)),
            "expected NotFound, got: {err:?}"
        );
    }
);

// Load: workspace mismatch returns NotFound (no existence leak).
pg_test!(
    view_load_workspace_mismatch_returns_not_found,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, _repo) = build_service_with_pg(pool, graph);

        let saved = service
            .save_view("w1", "u1", "v", None, "function", "callgraph", "x", 3)
            .await
            .expect("save ok");

        let err = service
            .load_view(&saved.id, "w2", "u1")
            .await
            .expect_err("scope mismatch must error");
        assert!(
            matches!(err, ExplorerError::NotFound(_)),
            "expected NotFound, got: {err:?}"
        );
    }
);

// Load: owner mismatch returns NotFound.
pg_test!(
    view_load_owner_mismatch_returns_not_found,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, _repo) = build_service_with_pg(pool, graph);

        let saved = service
            .save_view("w1", "u1", "v", None, "function", "callgraph", "x", 3)
            .await
            .expect("save ok");

        let err = service
            .load_view(&saved.id, "w1", "u2")
            .await
            .expect_err("owner mismatch must error");
        assert!(
            matches!(err, ExplorerError::NotFound(_)),
            "expected NotFound, got: {err:?}"
        );
    }
);

// List: returns only the matching scope, ordered newest-first.
pg_test!(
    view_list_returns_only_matching_scope,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, _repo) = build_service_with_pg(pool, graph);

        for (i, name) in ["a", "b", "c"].iter().enumerate() {
            service
                .save_view("w1", "u1", name, None, "function", "callgraph", "x", 3)
                .await
                .expect("save ok");
            if i < 2 {
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
        }
        service
            .save_view("w1", "u2", "d", None, "function", "callgraph", "x", 3)
            .await
            .expect("save u2 ok");

        let list = service.list_views("w1", "u1").await.expect("list ok");
        assert_eq!(list.len(), 3, "expected 3 entries for (w1, u1)");
        // newest-first: insertion order reversed.
        let names: Vec<String> = list
            .iter()
            .map(|d: &NamedViewDescriptor| d.name.clone())
            .collect();
        assert_eq!(names, vec!["c", "b", "a"]);
        for d in &list {
            assert_eq!(d.workspace_id, "w1");
            assert_eq!(d.owner, "u1");
        }
    }
);

// List: empty scope returns Ok(vec![]).
pg_test!(
    view_list_empty_scope_returns_ok_empty_vec,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, _repo) = build_service_with_pg(pool, graph);

        let list = service
            .list_views("w_other", "u_other")
            .await
            .expect("list must succeed on empty scope");
        assert!(list.is_empty());
    }
);

// List: long descriptions are truncated in the descriptor.
pg_test!(
    view_list_truncates_long_descriptions,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, _repo) = build_service_with_pg(pool, graph);

        let long: String = "d".repeat(1500);
        service
            .save_view(
                "w1",
                "u1",
                "v",
                Some(&long),
                "function",
                "callgraph",
                "x",
                3,
            )
            .await
            .expect("save ok");

        let list = service.list_views("w1", "u1").await.expect("list ok");
        assert_eq!(list.len(), 1);
        let desc = list[0].description.as_deref().expect("description present");
        assert_eq!(desc.chars().count(), 201, "200 chars + ellipsis");
        assert!(desc.ends_with('\u{2026}'));
    }
);

// List: short descriptions are preserved verbatim.
pg_test!(
    view_list_preserves_short_descriptions,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, _repo) = build_service_with_pg(pool, graph);

        service
            .save_view(
                "w1",
                "u1",
                "v",
                Some("hello"),
                "function",
                "callgraph",
                "x",
                3,
            )
            .await
            .expect("save ok");

        let list = service.list_views("w1", "u1").await.expect("list ok");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].description.as_deref(), Some("hello"));
    }
);

// Delete: removes the row.
pg_test!(view_delete_removes_row, |_url: String, pool: PgPool| {
    let graph = Arc::new(build_one_symbol_graph());
    let (service, repo) = build_service_with_pg(pool, graph);

    let saved = service
        .save_view("w1", "u1", "v", None, "function", "callgraph", "x", 3)
        .await
        .expect("save ok");
    let removed = service
        .delete_view(&saved.id, "w1", "u1")
        .await
        .expect("delete ok");
    assert!(removed);

    // Load must now return NotFound.
    let err = service
        .load_view(&saved.id, "w1", "u1")
        .await
        .expect_err("load after delete must error");
    assert!(matches!(err, ExplorerError::NotFound(_)));

    // Raw select must yield zero rows.
    let n: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM named_views WHERE id = $1")
        .bind(&saved.id)
        .fetch_one(repo.pool())
        .await
        .expect("count ok")
        .0;
    assert_eq!(n, 0, "row must be gone after delete");
});

// Delete: scope mismatch does NOT remove the row.
pg_test!(
    view_delete_mismatch_does_not_remove,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, repo) = build_service_with_pg(pool, graph);

        let saved = service
            .save_view("w1", "u1", "v", None, "function", "callgraph", "x", 3)
            .await
            .expect("save ok");
        let removed = service
            .delete_view(&saved.id, "w2", "u1")
            .await
            .expect("delete ok");
        assert!(!removed, "scope mismatch must NOT remove row");

        // Row still in PG.
        let n: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM named_views WHERE id = $1")
            .bind(&saved.id)
            .fetch_one(repo.pool())
            .await
            .expect("count ok")
            .0;
        assert_eq!(n, 1, "row must still be present after mismatched delete");
    }
);

// Delete: unknown id returns Ok(false).
pg_test!(
    view_delete_unknown_id_returns_false,
    |_url: String, pool: PgPool| {
        let graph = Arc::new(build_one_symbol_graph());
        let (service, _repo) = build_service_with_pg(pool, graph);

        let removed = service
            .delete_view("00000000-0000-0000-0000-000000000000", "w1", "u1")
            .await
            .expect("delete ok");
        assert!(!removed, "unknown id must return false");
    }
);

// =================================================================
// Adapter import sanity: ensures the explorer re-exports stay in sync.
// =================================================================

#[test]
fn named_view_dto_is_in_dto_module() {
    // Smoke test: the explorer DTO is reachable through the
    // module path. Compiler fails if a rename breaks the path.
    let _: NamedView = NamedView {
        id: "i".into(),
        workspace_id: "w".into(),
        owner: "u".into(),
        name: "n".into(),
        description: None,
        level: "function".into(),
        lens: "callgraph".into(),
        focus_node: "symbol:x".into(),
        max_depth: 1,
        created_at: "2026-06-09T00:00:00Z".into(),
    };
}
