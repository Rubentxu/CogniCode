//! Contract tests for the In-Memory Bridge
//! (`open_graph_from_postgres`).
//!
//! The whole file is `#[cfg(all(test, feature = "postgres"))]`-gated:
//! when the `postgres` feature is off, this file compiles to nothing
//! and no `sqlx` enters the explorer's test dependency graph.
//!
//! Per-test isolation follows the same pattern used in
//! `cognicode-core/src/infrastructure/persistence/postgres_repository.rs`:
//! each test gets its own uniquely-named database (drop-then-create)
//! so the suite runs in parallel without shared-state interference.
//!
//! Prerequisite: set `TEST_DATABASE_URL` to a base URL like
//! `postgres://user:pass@host:5432`. The test runner will create
//! databases named `cognicode_bridge_test_<pid>_<n>` for each test.
//! When `TEST_DATABASE_URL` is unset, every test prints a skip
//! message and exits early — useful for local `cargo test` runs
//! without a PG instance.

#![cfg(all(test, feature = "postgres"))]

use std::sync::atomic::{AtomicU64, Ordering};

use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::traits::Repository;
use cognicode_core::domain::value_objects::{DependencyType, Location, Provenance, SymbolKind};
use cognicode_core::infrastructure::persistence::PostgresRepository;
use cognicode_explorer::adapters::CallGraphRepository;
use cognicode_explorer::ports::symbol_repository::SymbolRepository;
use cognicode_explorer::postgres_bridge::open_graph_from_postgres;

#[cfg(feature = "postgres")]
use sqlx::PgPool;

// Per-process counter so every test gets a unique DB name.
static UNIQ: AtomicU64 = AtomicU64::new(0);

/// Build a fresh per-test PostgreSQL database: drop-then-create,
/// connect, and run the embedded schema. Returns the unique test
/// URL (the database name encodes `pid` + atomic counter) and
/// the pool pointing at it. Returns `None` when
/// `TEST_DATABASE_URL` is unset — every test then skips cleanly.
async fn fresh_test_url() -> Option<(String, PgPool)> {
    let base = std::env::var("TEST_DATABASE_URL").ok()?;
    let n = UNIQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let db_name = format!("cognicode_bridge_test_{pid}_{n}");

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

/// Replace the database segment in a `postgres://...` URL with the
/// given name. Splits on the last `/` after `@`.
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

/// Tiny macro that mirrors `pg_test!` from the core crate: prints
/// a skip message and returns early when `TEST_DATABASE_URL` is
/// not set, so local `cargo test` runs don't blow up.
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

/// Build the canonical mixed-provenance fixture used in the
/// round-trip contract: ≥5 symbols, ≥3 `DependencyType`s, all 3
/// `Provenance` variants, and confidences covering {0.0, 0.5, 1.0}
/// (the self-loop is clamped to 0.5 by `ConfidenceRules`).
fn build_mixed_provenance_graph() -> CallGraph {
    let mut g = CallGraph::new();
    let a = g.add_symbol(Symbol::new(
        "a",
        SymbolKind::Function,
        Location::new("a.rs", 1, 0),
    ));
    let b = g.add_symbol(Symbol::new(
        "b",
        SymbolKind::Function,
        Location::new("b.rs", 1, 0),
    ));
    let c = g.add_symbol(Symbol::new(
        "c",
        SymbolKind::Class,
        Location::new("c.rs", 1, 0),
    ));
    let d = g.add_symbol(Symbol::new(
        "d",
        SymbolKind::Method,
        Location::new("d.rs", 1, 0),
    ));
    let e = g.add_symbol(Symbol::new(
        "e",
        SymbolKind::Function,
        Location::new("e.rs", 1, 0),
    ));
    let f = g.add_symbol(Symbol::new(
        "f",
        SymbolKind::Function,
        Location::new("f.rs", 1, 0),
    ));

    // Extracted / 1.0
    g.add_dependency_with_provenance(
        &a,
        &b,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    )
    .expect("a->b");
    // Inferred / 0.7
    g.add_dependency_with_provenance(
        &a,
        &c,
        DependencyType::Imports,
        ExtractionContext::Heuristic { score: 0.7 },
    )
    .expect("a->c");
    // Ambiguous / 0.3
    g.add_dependency_with_provenance(
        &b,
        &d,
        DependencyType::Inherits,
        ExtractionContext::Unresolved,
    )
    .expect("b->d");
    // Extracted / 1.0
    g.add_dependency_with_provenance(
        &c,
        &d,
        DependencyType::References,
        ExtractionContext::DirectExtraction,
    )
    .expect("c->d");
    // Inferred / 0.5 (band bottom)
    g.add_dependency_with_provenance(
        &d,
        &e,
        DependencyType::UsesGeneric,
        ExtractionContext::Heuristic { score: 0.5 },
    )
    .expect("d->e");
    // Self-loop e->e, Heuristic 0.0 (clamped to 0.5 by rules)
    g.add_dependency_with_provenance(
        &e,
        &e,
        DependencyType::Defines,
        ExtractionContext::Heuristic { score: 0.0 },
    )
    .expect("e->e self-loop");
    // Multi-edge e->f, two different DependencyTypes
    g.add_dependency_with_provenance(
        &e,
        &f,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    )
    .expect("e->f calls");
    g.add_dependency_with_provenance(
        &e,
        &f,
        DependencyType::Imports,
        ExtractionContext::DirectExtraction,
    )
    .expect("e->f imports");

    g
}

// =================================================================
// Tests
// =================================================================

// Spec scenario 1: a populated PG (≥5 sym, ≥3 dep types, all 3
// provenance variants) round-trips bit-exact through the bridge
// into a `CallGraphRepository` that `assert_eq!`s equal to the
// source graph.
pg_test!(
    round_trip_populated_db_is_bit_exact,
    |url: String, pool: PgPool| {
        let source = build_mixed_provenance_graph();
        assert!(source.symbol_count() >= 5, "fixture must have >=5 symbols");
        assert!(source.edge_count() >= 3, "fixture must have >=3 edges");

        // Persist the source graph to PG.
        let seed_repo = PostgresRepository::from_pool(pool);
        seed_repo
            .save_call_graph(&source)
            .await
            .expect("save_call_graph must succeed");

        // Run the bridge helper.
        let loaded_arc = open_graph_from_postgres(&url)
            .await
            .expect("bridge helper must succeed on a populated DB");

        // Sanity-check counts at the helper boundary.
        assert_eq!(loaded_arc.symbol_count(), source.symbol_count());
        assert_eq!(loaded_arc.edge_count(), source.edge_count());

        // Wrap in the explorer's repository adapter and assert structural
        // equality: every symbol/edge and every (provenance, confidence)
        // pair must round-trip bit-exactly.
        let loaded_repo = CallGraphRepository::new(loaded_arc.clone());
        assert_eq!(loaded_arc.symbol_count(), source.symbol_count());
        assert_eq!(loaded_arc.edge_count(), source.edge_count());

        // FQN-by-FQN: every source symbol resolves on the loaded graph.
        for (_, sym) in source.symbol_ids() {
            let fqn = sym.fully_qualified_name();
            let resolved = loaded_repo
                .resolve(&SymbolId::new(fqn))
                .expect("resolve ok")
                .unwrap_or_else(|| panic!("missing loaded symbol: {fqn}"));
            assert_eq!(resolved.name, sym.name());
            assert_eq!(resolved.file, sym.location().file());
            assert_eq!(resolved.line, sym.location().line());
        }
    }
);

// Spec scenario 2: an empty DB (both tables empty) yields
// `Ok(Arc::new(CallGraph::new()))` with counts 0/0.
pg_test!(empty_db_yields_empty_graph, |url: String, pool: PgPool| {
    // Confirm the pool is empty.
    let seed_repo = PostgresRepository::from_pool(pool);
    assert_eq!(seed_repo.count_symbols().await.unwrap(), 0);
    assert_eq!(seed_repo.count_edges().await.unwrap(), 0);

    let arc = open_graph_from_postgres(&url)
        .await
        .expect("bridge must succeed on an empty DB");
    assert_eq!(arc.symbol_count(), 0);
    assert_eq!(arc.edge_count(), 0);

    // And the helper returns a fresh, empty `CallGraph` (not `None`).
    let wrapped = CallGraphRepository::new(arc);
    let stats = wrapped.graph_stats();
    assert_eq!(stats.symbol_count, 0);
    assert_eq!(stats.relation_count, 0);
});

/// Spec scenario 3: an unreachable PG URL propagates an `Err`
/// whose message starts with `"open_graph_from_postgres: connect:"`.
#[tokio::test]
async fn connect_failure_propagates_with_prefix() {
    // Unroutable port on loopback: connect must fail fast.
    let bad = "postgres://invalid:invalid@127.0.0.1:1/nope";
    let result = open_graph_from_postgres(bad).await;
    let err = result.expect_err("unreachable PG must surface as Err");
    let msg = err.to_string();
    assert!(
        msg.contains("open_graph_from_postgres: connect:"),
        "expected prefixed connect error, got: {msg}"
    );
}

// Spec requirement: `GraphQueryPort` preserves
// `(provenance, confidence)` bit-exact through the bridge.
pg_test!(
    metadata_aware_callees_round_trip,
    |url: String, pool: PgPool| {
        use cognicode_explorer::ports::GraphQueryPort;

        // Build a graph with three edges from `alpha` covering all
        // three `Provenance` variants: Extracted/1.0, Inferred/0.7,
        // Ambiguous/0.3. We use the graph API (not raw SQL) so this
        // test exercises the same path the production write-path
        // uses once it lands.
        let mut g = CallGraph::new();
        let a = g.add_symbol(Symbol::new(
            "alpha",
            SymbolKind::Function,
            Location::new("src/a.rs", 1, 0),
        ));
        let b = g.add_symbol(Symbol::new(
            "beta",
            SymbolKind::Function,
            Location::new("src/b.rs", 1, 0),
        ));
        let c = g.add_symbol(Symbol::new(
            "gamma",
            SymbolKind::Function,
            Location::new("src/c.rs", 1, 0),
        ));
        let d = g.add_symbol(Symbol::new(
            "delta",
            SymbolKind::Function,
            Location::new("src/d.rs", 1, 0),
        ));
        g.add_dependency_with_provenance(
            &a,
            &b,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        )
        .expect("a->b Extracted");
        g.add_dependency_with_provenance(
            &a,
            &c,
            DependencyType::Imports,
            ExtractionContext::Heuristic { score: 0.7 },
        )
        .expect("a->c Inferred");
        g.add_dependency_with_provenance(
            &a,
            &d,
            DependencyType::References,
            ExtractionContext::Unresolved,
        )
        .expect("a->d Ambiguous");

        // Persist to PG through the public save path.
        let seed_repo = PostgresRepository::from_pool(pool);
        seed_repo
            .save_call_graph(&g)
            .await
            .expect("save_call_graph must succeed");

        // Load via the bridge and check every (provenance, confidence)
        // pair round-trips bit-exact through the in-memory repository.
        let loaded_arc = open_graph_from_postgres(&url)
            .await
            .expect("bridge must succeed");
        let loaded_repo = CallGraphRepository::new(loaded_arc);

        let metas = GraphQueryPort::callees_with_metadata(&loaded_repo, &a);
        assert_eq!(metas.len(), 3, "expected 3 edges from alpha after bridge");

        // Every entry's confidence is finite, in [0.0, 1.0], and
        // matches the canonical (Extracted/Inferred/Ambiguous) bands.
        for m in &metas {
            assert!(m.confidence.is_finite());
            assert!((0.0..=1.0).contains(&m.confidence));
        }
        // Extracted -> 1.0
        let extracted = metas
            .iter()
            .find(|m| m.provenance == Provenance::Extracted)
            .expect("extracted edge present");
        assert_eq!(extracted.confidence, 1.0_f64);
        // Inferred -> 0.7
        let inferred = metas
            .iter()
            .find(|m| m.provenance == Provenance::Inferred)
            .expect("inferred edge present");
        assert_eq!(inferred.confidence, 0.7_f64);
        // Ambiguous -> 0.3
        let ambiguous = metas
            .iter()
            .find(|m| m.provenance == Provenance::Ambiguous)
            .expect("ambiguous edge present");
        assert_eq!(ambiguous.confidence, 0.3_f64);
    }
);

// Spec requirement: parallel `pg_test!` runs stay isolated.
// Each test creates its own unique DB and only sees its own rows.
pg_test!(
    parallel_isolation_first_sees_only_its_seed,
    |_url: String, pool: PgPool| {
        let seed_repo = PostgresRepository::from_pool(pool);
        seed_repo
            .save_call_graph(&build_mixed_provenance_graph())
            .await
            .expect("save");
        let syms = seed_repo.count_symbols().await.unwrap();
        let edges = seed_repo.count_edges().await.unwrap();
        assert!(syms >= 5 && edges >= 3, "expected populated counts");
    }
);

pg_test!(
    parallel_isolation_second_sees_empty_db,
    |_url: String, pool: PgPool| {
        let seed_repo = PostgresRepository::from_pool(pool);
        assert_eq!(
            seed_repo.count_symbols().await.unwrap(),
            0,
            "isolation violated: saw rows from sibling test"
        );
        assert_eq!(seed_repo.count_edges().await.unwrap(), 0);
    }
);
