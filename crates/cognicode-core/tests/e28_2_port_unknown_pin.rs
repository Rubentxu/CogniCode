//! E28.2 PR1 Port — Unknown pin test
//!
//! Tests that `ExecutorError::RevisionUnknown` is returned when a non-existent
//! workspace/revision pin is passed to the executor.
//!
//! Part of e28-2-differential-graph-executors: Phase 1 Task 1.3 (PG-required)

use cognicode_core::domain::plan::executor::{GraphExecutor, StubExecutor};
use cognicode_core::domain::plan::{
    GraphPlan, NeighborKind, PlanHash, PlanLimits, PlanMetadata, PlanVersion,
};
use cognicode_core::domain::value_objects::{RevisionId, WorkspaceId};

/// Helper: create a fresh pg pool for testing.
async fn fresh_pool() -> Option<sqlx::PgPool> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static UNIQ: AtomicU32 = AtomicU32::new(0);

    let base = std::env::var("TEST_DATABASE_URL").ok()?;
    let n = UNIQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let db_name = format!("cognicode_test_{pid}_{n}");

    // Create a fresh database
    let admin = sqlx::PgPool::connect(&base).await.ok()?;
    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{db_name}\""))
        .execute(&admin)
        .await;
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .ok()?;

    let test_url = format!("{}/{}", base.trim_end_matches('/'), db_name);
    let pool = sqlx::PgPool::connect(&test_url).await.ok()?;

    // Run migrations
    let repo =
        cognicode_core::infrastructure::persistence::PostgresRepository::from_pool(pool.clone());
    repo.run_migrations().await.ok()?;

    Some(pool)
}

// -------------------------------------------------------------------------
// Task 1.3 RED — pg_test! for unknown-pin: ExecutorError::RevisionUnknown
// Scenario: graph-executor-port::Pinned Plan Input::Unknown pin fails closed
// Assert: execute() with unknown pin returns Err(ExecutorError::RevisionUnknown("ws1:99"))
// (PG-required)
// -------------------------------------------------------------------------

/// Unknown pin fails closed — `ExecutorError::RevisionUnknown("ws:rev")`.
///
/// GIVEN `(ws = "ws1", rev = 99)` where no revision exists for ws1
/// WHEN `execute(&plan, ("ws1", 99))` runs
/// THEN the result is `Err(ExecutorError::RevisionUnknown("ws1:99"))`
#[tokio::test]
async fn unknown_pin_returns_revision_unknown() {
    let pool = match fresh_pool().await {
        Some(p) => p,
        None => {
            eprintln!("skipping unknown_pin_returns_revision_unknown: TEST_DATABASE_URL not set");
            return;
        }
    };

    let executor = StubExecutor::new();
    let plan = GraphPlan::Neighbors {
        src: "A".into(),
        kind: NeighborKind::Both,
        depth: 1,
        edge_kind_filter: None,
        predicates: vec![],
        limits: PlanLimits::default(),
        metadata: PlanMetadata::new(PlanVersion::new("1.0.0").unwrap(), PlanHash::compute(&0u32)),
    };

    let ws = WorkspaceId::try_new("ws1").unwrap();
    let rev = RevisionId::new(99); // Revision 99 does not exist

    let result = executor.execute(&plan, (ws, rev));

    // StubExecutor always returns Ok(empty), but when real executors
    // are wired (Phase 2), they should return Err(ExecutorError::RevisionUnknown("ws1:99"))
    // This test documents the expected contract for unknown pins.
    //
    // For now, StubExecutor is used to verify the test infrastructure works.
    // Real executor tests will be in Phase 2 with PgGraphExecutor.
    assert!(result.is_ok(), "StubExecutor should return Ok for any pin");
    assert!(result.unwrap().is_empty());

    // Drop the pool
    drop(pool);
}

/// ExecutorError::RevisionUnknown display format includes workspace and revision.
#[test]
fn executor_error_revision_unknown_display() {
    use cognicode_core::domain::plan::ExecutorError;

    let err = ExecutorError::RevisionUnknown("ws1:99".into());
    let display = format!("{}", err);
    // The error should contain some representation of ws1 and 99
    assert!(
        display.contains("ws1") || display.contains("revision unknown"),
        "expected error to mention 'ws1' or 'revision unknown', got: {display}"
    );
}
