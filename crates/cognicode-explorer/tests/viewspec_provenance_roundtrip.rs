//! Contract tests for ViewSpec provenance fields (seed_object_id, seed_view_id,
//! applies_when) round-tripping through PostgreSQL.
//!
//! The whole file is `#[cfg(all(test, feature = "postgres"))]`-gated.
//! Per-test isolation: each test gets its own uniquely-named database
//! (drop-then-create) so the suite runs in parallel without shared-state
//! interference.
//!
//! Prerequisite: set `TEST_DATABASE_URL` to a base URL like
//! `postgres://user:pass@host:5432`. The test runner will create
//! databases named `cognicode_viewspec_test_<pid>_<n>` for each test.
//! When `TEST_DATABASE_URL` is unset, every test prints a skip
//! message and exits early.

#![cfg(all(test, feature = "postgres"))]

use std::sync::atomic::{AtomicU64, Ordering};

use cognicode_core::infrastructure::persistence::PostgresRepository;
use serde_json::json;

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
    let db_name = format!("cognicode_viewspec_test_{pid}_{n}");

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
/// given name.
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

/// Macro that mirrors `pg_test!` from pg_exploration_session_contract.rs.
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

// =================================================================
// Tests
// =================================================================

// Test: insert ViewSpec with all 3 provenance fields populated, read it back,
// verify all 3 fields match.
pg_test!(
    viewspec_roundtrip_all_provenance_fields_populated,
    |_url: String, pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);

        let id = "11111111-1111-1111-1111-111111111111";
        let workspace_id = "test-workspace";
        let owner = "test-owner";
        let title = "My Custom View";
        let applies_to = "symbol";
        let view_kind = "call_graph";
        let data_source = json!({"type": "moldql", "query": "calls from 'Foo'"});
        let renderer_kind = "graph";
        let props = json!({"max_depth": 3});
        let seed_object_id = Some("sym:UserService::create".to_string());
        let seed_view_id = Some("vertical_slice".to_string());
        let applies_when = Some("kind = 'function'".to_string());

        // Save the view spec with all provenance fields populated.
        repo.save_view_spec(
            id,
            workspace_id,
            owner,
            title,
            applies_to,
            view_kind,
            &data_source.to_string(),
            None,
            renderer_kind,
            &props.to_string(),
            seed_object_id.as_deref(),
            seed_view_id.as_deref(),
            applies_when.as_deref(),
        )
        .await
        .expect("save_view_spec must succeed");

        // Load it back.
        let row = repo
            .load_view_spec(id, workspace_id, owner)
            .await
            .expect("load_view_spec must succeed")
            .expect("view spec must exist after save");

        assert_eq!(row.id, id);
        assert_eq!(row.seed_object_id, seed_object_id);
        assert_eq!(row.seed_view_id, seed_view_id);
        assert_eq!(row.applies_when, applies_when);
    }
);

// Test: insert ViewSpec with all 3 provenance fields null, read it back,
// verify all 3 are None.
pg_test!(
    viewspec_roundtrip_all_provenance_fields_null,
    |_url: String, pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);

        let id = "22222222-2222-2222-2222-222222222222";
        let workspace_id = "test-workspace";
        let owner = "test-owner";
        let title = "Minimal View";
        let applies_to = "symbol";
        let view_kind = "overview";
        let data_source = json!({"type": "other"});
        let renderer_kind = "json";
        let props = json!({});

        // Save the view spec with all provenance fields as None.
        repo.save_view_spec(
            id,
            workspace_id,
            owner,
            title,
            applies_to,
            view_kind,
            &data_source.to_string(),
            None,
            renderer_kind,
            &props.to_string(),
            None,
            None,
            None,
        )
        .await
        .expect("save_view_spec must succeed");

        // Load it back.
        let row = repo
            .load_view_spec(id, workspace_id, owner)
            .await
            .expect("load_view_spec must succeed")
            .expect("view spec must exist after save");

        assert_eq!(row.id, id);
        assert_eq!(row.seed_object_id, None);
        assert_eq!(row.seed_view_id, None);
        assert_eq!(row.applies_when, None);
    }
);

// Test: partial provenance — seed_object_id populated, others null.
pg_test!(
    viewspec_roundtrip_partial_provenance,
    |_url: String, pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);

        let id = "33333333-3333-3333-3333-333333333333";
        let workspace_id = "test-workspace";
        let owner = "test-owner";
        let title = "Partial Provenance View";
        let applies_to = "symbol";
        let view_kind = "vertical_slice";
        let data_source = json!({"type": "moldql", "query": "focus 'Foo'"});
        let renderer_kind = "graph";
        let props = json!({"max_depth": 5});
        let seed_object_id = Some("sym:Bar::baz".to_string());

        // Save with only seed_object_id populated.
        repo.save_view_spec(
            id,
            workspace_id,
            owner,
            title,
            applies_to,
            view_kind,
            &data_source.to_string(),
            None,
            renderer_kind,
            &props.to_string(),
            seed_object_id.as_deref(),
            None,
            None,
        )
        .await
        .expect("save_view_spec must succeed");

        // Load it back.
        let row = repo
            .load_view_spec(id, workspace_id, owner)
            .await
            .expect("load_view_spec must succeed")
            .expect("view spec must exist after save");

        assert_eq!(row.id, id);
        assert_eq!(row.seed_object_id, seed_object_id);
        assert_eq!(row.seed_view_id, None);
        assert_eq!(row.applies_when, None);
    }
);
