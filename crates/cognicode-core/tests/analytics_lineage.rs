//! Tests for E28.4 Analytics Lineage Persistence (m0020).
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR4 Lineage Persistence.

use std::sync::Arc;

use cognicode_core::domain::analytics::{
    AnalyticsError, AnalyticsMode, AlgorithmId, DeterminismKind, RunLineage,
    RunLineageFilter, RunLineageStore, RunStatus, Uuid,
};
use cognicode_core::domain::value_objects::{RevisionId, WorkspaceId};
use cognicode_core::infrastructure::persistence::PostgresLineageStore;

/// A no-op lineage store for testing — in-memory fallback.
struct NoOpLineageStore;

impl RunLineageStore for NoOpLineageStore {
    fn insert(&self, _lineage: &RunLineage) -> Result<(), AnalyticsError> {
        Ok(())
    }

    fn get(&self, _run_id: Uuid) -> Result<RunLineage, AnalyticsError> {
        Err(AnalyticsError::RunNotFound("not found".into()))
    }

    fn query(
        &self,
        _filter: RunLineageFilter,
        _limit: Option<u64>,
    ) -> Result<Vec<RunLineage>, AnalyticsError> {
        Ok(vec![])
    }
}

fn dummy_workspace() -> WorkspaceId {
    WorkspaceId::try_new("test-workspace").expect("valid workspace id")
}

fn dummy_revision() -> RevisionId {
    RevisionId::new(1)
}

#[test]
fn noop_lineage_store_insert_returns_ok() {
    let store = NoOpLineageStore;
    let lineage = RunLineage::new(
        dummy_workspace(),
        dummy_revision(),
        AlgorithmId::from_static("pagerank"),
        "v1.0.0".into(),
        vec![],
        serde_json::json!({}),
        None,
        AnalyticsMode::Stream,
    );
    assert!(store.insert(&lineage).is_ok());
}

#[test]
fn noop_lineage_store_get_returns_not_found() {
    let store = NoOpLineageStore;
    let uuid = Uuid::new_v4();
    let result = store.get(uuid);
    assert!(matches!(result, Err(AnalyticsError::RunNotFound(_))));
}

#[test]
fn noop_lineage_store_query_returns_empty() {
    let store = NoOpLineageStore;
    let filter = RunLineageFilter::default();
    let result = store.query(filter, None);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn run_lineage_new_is_pending() {
    let lineage = RunLineage::new(
        dummy_workspace(),
        dummy_revision(),
        AlgorithmId::from_static("pagerank"),
        "v1.0.0".into(),
        vec![],
        serde_json::json!({}),
        None,
        AnalyticsMode::Stream,
    );
    assert_eq!(lineage.status, RunStatus::Pending);
    assert!(lineage.finished_at.is_none());
    assert!(lineage.row_count.is_none());
    assert!(lineage.truncation_marker.is_none());
    assert!(lineage.error_kind.is_none());
}

#[test]
fn run_lineage_succeed() {
    let mut lineage = RunLineage::new(
        dummy_workspace(),
        dummy_revision(),
        AlgorithmId::from_static("pagerank"),
        "v1.0.0".into(),
        vec![],
        serde_json::json!({}),
        None,
        AnalyticsMode::Stream,
    );
    lineage.succeed(42);
    assert_eq!(lineage.status, RunStatus::Succeeded);
    assert!(lineage.finished_at.is_some());
    assert_eq!(lineage.row_count, Some(42));
}

#[test]
fn run_lineage_truncate() {
    let mut lineage = RunLineage::new(
        dummy_workspace(),
        dummy_revision(),
        AlgorithmId::from_static("pagerank"),
        "v1.0.0".into(),
        vec![],
        serde_json::json!({}),
        None,
        AnalyticsMode::Stream,
    );
    lineage.truncate(cognicode_core::domain::analytics::TruncationMarker::ResultRowsLimit, 10);
    assert_eq!(lineage.status, RunStatus::Truncated);
    assert_eq!(lineage.truncation_marker, Some(cognicode_core::domain::analytics::TruncationMarker::ResultRowsLimit));
    assert_eq!(lineage.row_count, Some(10));
}

#[test]
fn run_lineage_fail() {
    let mut lineage = RunLineage::new(
        dummy_workspace(),
        dummy_revision(),
        AlgorithmId::from_static("pagerank"),
        "v1.0.0".into(),
        vec![],
        serde_json::json!({}),
        None,
        AnalyticsMode::Stream,
    );
    lineage.fail("LimitExceeded(Memory)", "out of memory");
    assert_eq!(lineage.status, RunStatus::Failed);
    assert_eq!(lineage.error_kind, Some("LimitExceeded(Memory)".into()));
    assert_eq!(lineage.error_message, Some("out of memory".into()));
}

#[test]
fn run_lineage_with_idempotency_key() {
    let mut lineage = RunLineage::new(
        dummy_workspace(),
        dummy_revision(),
        AlgorithmId::from_static("pagerank"),
        "v1.0.0".into(),
        vec![],
        serde_json::json!({"alpha": 0.85}),
        None,
        AnalyticsMode::Persist,
    );
    lineage.set_idempotency_key("test-key-123".to_string());
    assert_eq!(lineage.idempotency_key, Some("test-key-123".to_string()));
}

#[test]
fn algorithm_id_from_static_valid() {
    let id = AlgorithmId::from_static("pagerank");
    assert_eq!(id.as_str(), "pagerank");
    assert_eq!(id.to_string(), "pagerank");
}

#[test]
fn algorithm_id_from_string_valid() {
    let id = AlgorithmId::from_string("bounded_shortest_paths".to_string());
    assert_eq!(id.as_str(), "bounded_shortest_paths");
}

#[test]
fn uuid_is_not_empty() {
    let uuid = Uuid::new_v4();
    let s = uuid.to_string();
    assert!(!s.is_empty(), "UUID should not be empty");
}

#[test]
fn uuid_from_string_roundtrip() {
    let original = Uuid::new_v4();
    let s = original.to_string();
    let recovered = Uuid::from_string(&s);
    assert_eq!(original, recovered);
}
