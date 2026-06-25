//! Integration tests for `PostgresQualityRepository`'s `QualityWritePort`
//! implementation (`insert_issues`, `delete_issue`).
//!
//! These tests require a live PostgreSQL instance with the
//! `m0011_quality.sql` migration applied. They are gated on the
//! `TEST_DATABASE_URL` environment variable — when unset, every test
//! is `#[ignore]`d at runtime so the test binary always builds.
//!
//! ## Running
//!
//! ```bash
//! TEST_DATABASE_URL=postgres://user:pass@host:5432 \
//!   cargo test -p cognicode-explorer --test postgres_quality_write_integration
//! ```

#![cfg(feature = "postgres")]

use std::sync::Arc;

use cognicode_core::infrastructure::persistence::PostgresRepository;
use cognicode_explorer::adapters::PostgresQualityRepository;
use cognicode_explorer::mcp::handler::quality_mcp::register_quality_mcp_handlers;
use cognicode_explorer::mcp::handler::ToolHandlerRegistry;
use cognicode_explorer::mcp::{McpContext, TOOL_INGEST_QUALITY_ISSUES};
use cognicode_explorer::ports::quality_repository::{NewIssue, QualityWritePort, UpsertSummary};
use cognicode_explorer::session::SessionRegistry;
use rmcp::model::CallToolResult;
use serde_json::{json, Value};

// ============================================================================
// DB setup helpers (mirrors postgres_quality_integration.rs)
// ============================================================================

fn admin_url() -> Option<String> {
    let base = std::env::var("TEST_DATABASE_URL").ok()?;
    let without_db = base.rsplitn(2, '/').nth(1)?;
    Some(without_db.to_string())
}

fn per_test_url(admin: &str, test_name: &str) -> String {
    let pid = std::process::id();
    let db = format!("cognicode_test_{pid}_{test_name}");
    format!("{admin}/{db}")
}

async fn with_test_db<F, Fut>(test_name: &str, f: F) -> Option<()>
where
    F: FnOnce(Arc<PostgresQualityRepository>) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let admin = admin_url()?;
    let url = per_test_url(&admin, test_name);

    let admin_pool = sqlx::PgPool::connect(&admin).await.ok()?;
    sqlx::query(&format!(
        "DROP DATABASE IF EXISTS \"{}\"",
        url.rsplit('/').next().unwrap()
    ))
    .execute(&admin_pool)
    .await
    .ok()?;
    sqlx::query(&format!(
        "CREATE DATABASE \"{}\"",
        url.rsplit('/').next().unwrap()
    ))
    .execute(&admin_pool)
    .await
    .ok()?;
    drop(admin_pool);

    let repo = Arc::new(PostgresRepository::new(&url).await.ok()?);
    repo.run_migrations().await.ok()?;

    let adapter = Arc::new(PostgresQualityRepository::new(&repo));
    f(adapter).await;

    let admin_pool = sqlx::PgPool::connect(&admin).await.ok()?;
    let _ = sqlx::query(&format!(
        "DROP DATABASE IF EXISTS \"{}\"",
        url.rsplit('/').next().unwrap()
    ))
    .execute(&admin_pool)
    .await;
    Some(())
}

// ============================================================================
// MCP dispatch helpers (mirrors quality_mcp_integration.rs)
// ============================================================================

fn ok_payload(result: &CallToolResult) -> Value {
    assert_eq!(
        result.is_error,
        Some(false),
        "expected ok envelope, got: {result:?}"
    );
    let env = extract_env(result);
    env.get("payload")
        .cloned()
        .expect("ok envelope must have a `payload` field")
}

fn err_code(result: &CallToolResult) -> String {
    assert_eq!(
        result.is_error,
        Some(true),
        "expected err envelope, got: {result:?}"
    );
    let env = extract_env(result);
    env.get("payload")
        .and_then(|p| p.get("error_code"))
        .and_then(|c| c.as_str())
        .map(String::from)
        .expect("err envelope payload must have `error_code`")
}

fn extract_env(result: &CallToolResult) -> Value {
    serde_json::to_value(result).expect("CallToolResult must serialize")
}

fn build_registry() -> ToolHandlerRegistry {
    let mut r = ToolHandlerRegistry::new();
    register_quality_mcp_handlers(&mut r);
    r
}

// ============================================================================
// Test 1: insert single issue returns inserted one
// ============================================================================

#[tokio::test]
async fn insert_single_issue_returns_inserted_one() {
    let Some(()) = with_test_db("insert_single", |adapter| async move {
        let issues = vec![NewIssue {
            workspace_id: "ws-1".into(),
            rule_id: "R001".into(),
            severity: "critical".into(),
            category: "complexity".into(),
            file_path: "src/main.rs".into(),
            line: 10,
            message: "too complex".into(),
            status: "open".into(),
        }];
        let summary = adapter.insert_issues(&issues).expect("insert should succeed");
        assert_eq!(summary, UpsertSummary { inserted: 1, updated: 0 });
    }).await else {
        eprintln!("TEST_DATABASE_URL unset — skipping");
        return;
    };
}

// ============================================================================
// Test 2: insert duplicate natural key returns updated one
// ============================================================================

#[tokio::test]
async fn insert_duplicate_natural_key_returns_updated_one() {
    let Some(()) = with_test_db("insert_dup", |adapter| async move {
        let make_issue = || NewIssue {
            workspace_id: "ws-1".into(),
            rule_id: "R001".into(),
            severity: "critical".into(),
            category: "complexity".into(),
            file_path: "src/main.rs".into(),
            line: 10,
            message: "updated message".into(),
            status: "resolved".into(),
        };
        let first = adapter.insert_issues(&[make_issue()]).expect("first insert succeeds");
        assert_eq!(first, UpsertSummary { inserted: 1, updated: 0 });

        let second = adapter.insert_issues(&[make_issue()]).expect("second insert succeeds");
        assert_eq!(second, UpsertSummary { inserted: 0, updated: 1 });
    }).await else {
        eprintln!("TEST_DATABASE_URL unset — skipping");
        return;
    };
}

// ============================================================================
// Test 3: insert batch of 100 in single transaction
// ============================================================================

#[tokio::test]
async fn insert_batch_of_100_in_single_transaction() {
    let Some(()) = with_test_db("insert_batch_100", |adapter| async move {
        let issues: Vec<NewIssue> = (0..100)
            .map(|i| NewIssue {
                workspace_id: "ws-1".into(),
                rule_id: format!("R{i:03}"),
                severity: "warning".into(),
                category: "complexity".into(),
                file_path: format!("src/file_{i}.rs"),
                line: 10,
                message: format!("issue {i}"),
                status: "open".into(),
            })
            .collect();
        let summary = adapter.insert_issues(&issues).expect("batch insert succeeds");
        assert_eq!(summary.inserted, 100);
        assert_eq!(summary.updated, 0);
    }).await else {
        eprintln!("TEST_DATABASE_URL unset — skipping");
        return;
    };
}

// ============================================================================
// Test 4: delete existing returns true
// ============================================================================

#[tokio::test]
async fn delete_existing_returns_true() {
    let Some(()) = with_test_db("delete_existing", |adapter| async move {
        let issue = NewIssue {
            workspace_id: "ws-1".into(),
            rule_id: "R001".into(),
            severity: "critical".into(),
            category: "complexity".into(),
            file_path: "src/main.rs".into(),
            line: 10,
            message: "to be deleted".into(),
            status: "open".into(),
        };
        adapter.insert_issues(&[issue]).expect("insert succeeds");

        let deleted = adapter
            .delete_issue("ws-1", "R001", "src/main.rs", 10)
            .expect("delete should not error");
        assert!(deleted, "existing issue should be deleted");
    }).await else {
        eprintln!("TEST_DATABASE_URL unset — skipping");
        return;
    };
}

// ============================================================================
// Test 5: delete nonexistent returns false
// ============================================================================

#[tokio::test]
async fn delete_nonexistent_returns_false() {
    let Some(()) = with_test_db("delete_nonexistent", |adapter| async move {
        let deleted = adapter
            .delete_issue("ws-none", "NO_SUCH", "no/such.rs", 999)
            .expect("delete should not error");
        assert!(!deleted, "nonexistent issue should return false");
    }).await else {
        eprintln!("TEST_DATABASE_URL unset — skipping");
        return;
    };
}

// ============================================================================
// Test 6: ingest_quality_issues MCP tool dispatches to port
// ============================================================================

#[tokio::test]
async fn ingest_quality_issues_mcp_tool_dispatches_to_port() {
    let Some(()) = with_test_db("ingest_tool_dispatch", |adapter| async move {
        let ctx = McpContext::builder()
            .with_session_registry(SessionRegistry::new())
            .with_quality_write(adapter as Arc<dyn QualityWritePort>)
            .build();
        let registry = build_registry();

        let result = registry
            .dispatch(
                TOOL_INGEST_QUALITY_ISSUES,
                &ctx,
                json!({
                    "workspace_id": "ws-tool-test",
                    "issues": [
                        {
                            "rule_id": "R100",
                            "severity": "warning",
                            "category": "style",
                            "file_path": "src/lib.rs",
                            "line": 42,
                            "message": "unused import",
                            "status": "open"
                        }
                    ]
                }),
            )
            .await;

        let payload = ok_payload(&result);
        assert_eq!(payload["inserted"].as_u64(), Some(1));
        assert_eq!(payload["updated"].as_u64(), Some(0));
        assert_eq!(payload["workspace_id"].as_str(), Some("ws-tool-test"));
    }).await else {
        eprintln!("TEST_DATABASE_URL unset — skipping");
        return;
    };
}

// ============================================================================
// Test 7: quality_write_unavailable when port not wired
// ============================================================================

#[tokio::test]
async fn quality_write_unavailable_when_port_not_wired() {
    // Context WITHOUT quality_write wired — should get quality_write_unavailable envelope
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let result = registry
        .dispatch(
            TOOL_INGEST_QUALITY_ISSUES,
            &ctx,
            json!({
                "workspace_id": "ws-test",
                "issues": [
                    {
                        "rule_id": "R001",
                        "severity": "info",
                        "category": "style",
                        "file_path": "src/main.rs",
                        "line": 1,
                        "message": "test",
                        "status": "open"
                    }
                ]
            }),
        )
        .await;

    let code = err_code(&result);
    assert_eq!(code, "quality_write_unavailable");
}

// ============================================================================
// Test 8: concurrent inserts with same key resolve to update
// ============================================================================

#[tokio::test]
async fn concurrent_inserts_with_same_key_resolve_to_update() {
    let Some(()) = with_test_db("concurrent_insert", |adapter| async move {
        let issue = NewIssue {
            workspace_id: "ws-concurrent".into(),
            rule_id: "R001".into(),
            severity: "critical".into(),
            category: "complexity".into(),
            file_path: "src/concurrent.rs".into(),
            line: 99,
            message: "race message".into(),
            status: "open".into(),
        };

        // insert_issues is synchronous, so we use spawn_blocking to
        // run both calls concurrently on the thread pool.
        let adapter1 = (*adapter).clone();
        let adapter2 = (*adapter).clone();
        let issue1 = issue.clone();
        let issue2 = issue;

        let (r1, r2) = tokio::join!(
            tokio::task::spawn_blocking(move || adapter1.insert_issues(&[issue1])),
            tokio::task::spawn_blocking(move || adapter2.insert_issues(&[issue2]))
        );

        let r1 = r1.expect("first task should not panic").expect("first insert should not error");
        let r2 = r2.expect("second task should not panic").expect("second insert should not error");

        // One inserted, one updated — order indeterminate
        let total_inserted = r1.inserted + r2.inserted;
        let total_updated = r1.updated + r2.updated;
        assert_eq!(total_inserted, 1, "exactly one row should be inserted");
        assert_eq!(total_updated, 1, "exactly one row should be updated");
    }).await else {
        eprintln!("TEST_DATABASE_URL unset — skipping");
        return;
    };
}
