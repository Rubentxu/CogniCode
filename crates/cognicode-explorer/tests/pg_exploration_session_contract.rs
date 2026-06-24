//! Contract tests for the Exploration Session Postgres persistence.
//!
//! The whole file is `#[cfg(all(test, feature = "postgres"))]`-gated:
//! when the `postgres` feature is off, this file compiles to nothing.
//!
//! Per-test isolation follows the same pattern used in
//! `pg_bridge_contract.rs`: each test gets its own uniquely-named
//! database (drop-then-create) so the suite runs in parallel without
//! shared-state interference.
//!
//! Prerequisite: set `TEST_DATABASE_URL` to a base URL like
//! `postgres://user:pass@host:5432`. The test runner will create
//! databases named `cognicode_session_test_<pid>_<n>` for each test.
//! When `TEST_DATABASE_URL` is unset, every test prints a skip
//! message and exits early.

#![cfg(all(test, feature = "postgres"))]

use std::sync::atomic::{AtomicU64, Ordering};

use chrono::Utc;
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
    let db_name = format!("cognicode_session_test_{pid}_{n}");

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

/// Macro that mirrors `pg_test!` from pg_bridge_contract.rs.
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

/// Build a canonical exploration session payload.
fn build_session_payload(workspace_id: &str) -> (String, String, String, String, String) {
    let id = format!("session:{}", Utc::now().timestamp_millis());
    let events = serde_json::to_string(&json!([
        {"object_id": "sym:UserService::create", "view_id": "call_graph", "ts": "2026-06-24T10:00:00Z"},
        {"object_id": "sym:UserRepository::save", "view_id": "call_graph", "ts": "2026-06-24T10:00:01Z"}
    ]))
    .unwrap();
    let navigation_mode = "pane_stack".to_string();
    let panes = serde_json::to_string(&json!([
        {"object_id": "sym:UserService", "view_id": "call_graph"}
    ]))
    .unwrap();
    (id, workspace_id.to_string(), events, navigation_mode, panes)
}

// =================================================================
// Tests
// =================================================================

// Test: save and load a single exploration session round-trips correctly.
pg_test!(
    save_and_load_exploration_session,
    |_url: String, pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        let (id, workspace_id, events, navigation_mode, panes) =
            build_session_payload("ws-1");

        // Save the session.
        repo.save_exploration_session(&id, &workspace_id, &events, &navigation_mode, &panes)
            .await
            .expect("save_exploration_session must succeed");

        // Load it back.
        let row = repo
            .load_exploration_session(&id, &workspace_id)
            .await
            .expect("load_exploration_session must succeed")
            .expect("session must exist after save");

        assert_eq!(row.id, id);
        assert_eq!(row.workspace_id, workspace_id);
        assert_eq!(row.navigation_mode, navigation_mode);

        // Events and panes are JSON strings in the row.
        let loaded_events: serde_json::Value =
            serde_json::from_str(&row.events.to_string())
                .expect("events must parse as JSON");
        assert!(loaded_events.is_array());
        assert_eq!(loaded_events.as_array().unwrap().len(), 2);

        let loaded_panes: serde_json::Value =
            serde_json::from_str(&row.panes.to_string())
                .expect("panes must parse as JSON");
        assert!(loaded_panes.is_array());
    }
);

// Test: list_exploration_sessions returns all sessions for a workspace.
pg_test!(
    list_exploration_sessions_returns_all_for_workspace,
    |_url: String, pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);

        // Save 3 sessions for ws-1 and 1 for ws-2.
        for i in 0..3 {
            let (id, ws_id, events, nav, panes) =
                build_session_payload("ws-1");
            repo.save_exploration_session(&id, &ws_id, &events, &nav, &panes)
                .await
                .expect("save must succeed");
        }
        let (id2, ws2, events2, nav2, panes2) =
            build_session_payload("ws-2");
        repo.save_exploration_session(&id2, &ws2, &events2, &nav2, &panes2)
            .await
            .expect("save must succeed");

        // List for ws-1.
        let ws1_rows = repo
            .list_exploration_sessions("ws-1")
            .await
            .expect("list must succeed");
        assert_eq!(ws1_rows.len(), 3, "expected 3 sessions for ws-1");

        // List for ws-2.
        let ws2_rows = repo
            .list_exploration_sessions("ws-2")
            .await
            .expect("list must succeed");
        assert_eq!(ws2_rows.len(), 1, "expected 1 session for ws-2");
    }
);

// Test: list_exploration_sessions returns empty Vec when no sessions exist.
pg_test!(
    list_exploration_sessions_empty_when_none_exist,
    |_url: String, pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);

        let rows = repo
            .list_exploration_sessions("nonexistent-workspace")
            .await
            .expect("list must succeed");
        assert!(rows.is_empty(), "expected empty list for unknown workspace");
    }
);

// Test: load_exploration_session returns None for unknown id.
pg_test!(
    load_exploration_session_returns_none_for_unknown_id,
    |_url: String, pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);

        let result = repo
            .load_exploration_session("unknown-id", "ws-1")
            .await
            .expect("load must succeed");
        assert!(result.is_none(), "expected None for unknown session id");
    }
);

// Test: parallel isolation — each test's DB is independent.
pg_test!(
    parallel_isolation_first,
    |_url: String, pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        let (id, ws_id, events, nav, panes) = build_session_payload("ws-isolated");
        repo.save_exploration_session(&id, &ws_id, &events, &nav, &panes)
            .await
            .expect("save must succeed");
        let rows = repo
            .list_exploration_sessions(&ws_id)
            .await
            .expect("list must succeed");
        assert_eq!(rows.len(), 1, "expected exactly 1 session");
    }
);

pg_test!(
    parallel_isolation_second,
    |_url: String, pool: PgPool| {
        let repo = PostgresRepository::from_pool(pool);
        // This test's DB was created fresh — no sessions from sibling test.
        let rows = repo
            .list_exploration_sessions("ws-isolated")
            .await
            .expect("list must succeed");
        assert_eq!(rows.len(), 0, "isolation violated: saw sessions from sibling test");
    }
);
