//! Run lineage types and storage port for analytics run records.
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR1 Foundation.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::analytics::descriptor::{AlgorithmId, AnalyticsError, AnalyticsMode};
use crate::domain::value_objects::{RevisionId, WorkspaceId};

// ============================================================================
// RunStatus
// ============================================================================

/// Final status of an analytics run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
    /// Run was accepted and is in progress.
    Pending,
    /// Run is currently executing.
    Running,
    /// Run completed successfully with full results.
    Succeeded,
    /// Run completed but results were truncated (soft limit hit).
    Truncated,
    /// Run failed (hard limit or execution error).
    Failed,
}

impl fmt::Display for RunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunStatus::Pending => write!(f, "pending"),
            RunStatus::Running => write!(f, "running"),
            RunStatus::Succeeded => write!(f, "succeeded"),
            RunStatus::Truncated => write!(f, "truncated"),
            RunStatus::Failed => write!(f, "failed"),
        }
    }
}

// ============================================================================
// TruncationMarker
// ============================================================================

/// Indicates which soft limit caused truncation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TruncationMarker {
    ResultRowsLimit,
    PathCountLimit,
    VisitedNodesLimit,
    VisitedEdgesLimit,
}

impl fmt::Display for TruncationMarker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TruncationMarker::ResultRowsLimit => write!(f, "ResultRowsLimit"),
            TruncationMarker::PathCountLimit => write!(f, "PathCountLimit"),
            TruncationMarker::VisitedNodesLimit => write!(f, "VisitedNodesLimit"),
            TruncationMarker::VisitedEdgesLimit => write!(f, "VisitedEdgesLimit"),
        }
    }
}

// ============================================================================
// RunLineage
// ============================================================================

/// Immutable record of an analytics run — the "who, what, when, why" log.
///
/// Every admitted execution produces exactly one `RunLineage` record.
/// Records are queryable and immutable once finalized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunLineage {
    /// Unique identifier for this run.
    pub run_id: Uuid,
    /// Workspace in which the run executed.
    pub workspace_id: WorkspaceId,
    /// Revision pin at execution time.
    pub revision_id: RevisionId,
    /// Algorithm identifier (e.g., "pagerank").
    pub algorithm_id: AlgorithmId,
    /// Algorithm version string (e.g., "v1.0.0").
    pub algorithm_version: String,
    /// SHA-256 hash of the canonicalized plan.
    pub plan_hash: Vec<u8>,
    /// Runtime parameters as JSON.
    pub params: serde_json::Value,
    /// Effective seed used (null if deterministic).
    pub seed: Option<u64>,
    /// Execution mode.
    pub mode: AnalyticsMode,
    /// Final status.
    pub status: RunStatus,
    /// When the run started.
    pub started_at: DateTime<Utc>,
    /// When the run completed (null if still running).
    pub finished_at: Option<DateTime<Utc>>,
    /// Number of result rows emitted (if applicable).
    pub row_count: Option<i64>,
    /// Which soft limit caused truncation (if truncated).
    pub truncation_marker: Option<TruncationMarker>,
    /// Idempotency key for persist mode (ensures deduplication).
    pub idempotency_key: Option<String>,
    /// Error kind if failed (e.g., "LimitExceeded(Memory)").
    pub error_kind: Option<String>,
    /// Error message if failed.
    pub error_message: Option<String>,
}

/// Run identifier — a UUID v4 string representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Uuid(String);

impl Uuid {
    /// Construct a Uuid from a string.
    ///
    /// Used when reconstructing from database storage.
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Generate a new random UUID v4 using CSPRNG.
    pub fn new_v4() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl RunLineage {
    /// Create a new pending run lineage record.
    pub fn new(
        workspace_id: WorkspaceId,
        revision_id: RevisionId,
        algorithm_id: AlgorithmId,
        algorithm_version: String,
        plan_hash: Vec<u8>,
        params: serde_json::Value,
        seed: Option<u64>,
        mode: AnalyticsMode,
    ) -> Self {
        Self {
            run_id: Uuid::new_v4(),
            workspace_id,
            revision_id,
            algorithm_id,
            algorithm_version,
            plan_hash,
            params,
            seed,
            mode,
            status: RunStatus::Pending,
            started_at: Utc::now(),
            finished_at: None,
            row_count: None,
            truncation_marker: None,
            idempotency_key: None,
            error_kind: None,
            error_message: None,
        }
    }

    /// Mark the run as succeeded.
    pub fn succeed(&mut self, row_count: i64) {
        self.status = RunStatus::Succeeded;
        self.finished_at = Some(Utc::now());
        self.row_count = Some(row_count);
    }

    /// Mark the run as truncated with a truncation marker.
    pub fn truncate(&mut self, marker: TruncationMarker, row_count: i64) {
        self.status = RunStatus::Truncated;
        self.finished_at = Some(Utc::now());
        self.row_count = Some(row_count);
        self.truncation_marker = Some(marker);
    }

    /// Mark the run as failed with error details.
    pub fn fail(&mut self, error_kind: impl Into<String>, error_message: impl Into<String>) {
        self.status = RunStatus::Failed;
        self.finished_at = Some(Utc::now());
        self.error_kind = Some(error_kind.into());
        self.error_message = Some(error_message.into());
    }

    /// Set the idempotency key for persist mode.
    pub fn set_idempotency_key(&mut self, key: String) {
        self.idempotency_key = Some(key);
    }
}

// ============================================================================
// RunLineageStore (port)
// ============================================================================

/// Port for persisting and querying run lineage records.
///
/// Implementations must provide durable storage (e.g., PostgreSQL).
#[async_trait::async_trait]
pub trait RunLineageStore: Send + Sync + 'static {
    /// Insert a new run lineage record.
    ///
    /// Returns `Err(AnalyticsError::IdempotencyConflict)` if an existing
    /// record has the same idempotency_key but different parameters.
    async fn insert(&self, lineage: &RunLineage) -> Result<(), AnalyticsError>;

    /// Get a run lineage record by run ID.
    async fn get(&self, run_id: Uuid) -> Result<RunLineage, AnalyticsError>;

    /// Query lineage records by filter.
    async fn query(
        &self,
        filter: RunLineageFilter,
        limit: Option<u64>,
    ) -> Result<Vec<RunLineage>, AnalyticsError>;

    /// Upsert descriptor limits for an algorithm version.
    async fn upsert_descriptor_limits(
        &self,
        algorithm_id: &AlgorithmId,
        version: &str,
        limits: &crate::domain::plan::limits::PlanLimits,
    ) -> Result<(), AnalyticsError>;

    /// Get descriptor limits for an algorithm version.
    async fn get_descriptor_limits(
        &self,
        algorithm_id: &AlgorithmId,
        version: &str,
    ) -> Result<Option<crate::domain::plan::limits::PlanLimits>, AnalyticsError>;
}

/// Filter criteria for lineage queries.
#[derive(Debug, Clone, Default)]
pub struct RunLineageFilter {
    pub workspace_id: Option<WorkspaceId>,
    pub revision_id: Option<RevisionId>,
    pub algorithm_id: Option<AlgorithmId>,
    pub status: Option<RunStatus>,
}

// ============================================================================
// InMemoryLineageStore
// ============================================================================

/// A `Vec`-backed, in-process implementation of [`RunLineageStore`].
///
/// **This store is ephemeral — it is NOT durable and data is lost on process
/// exit.** Use it for single-node deployments, local development, or when no
/// persistent backend (e.g., PostgreSQL/LadybugDB) is configured.
///
/// Queries against an empty store return empty results; `insert` appends to the
/// internal `Vec` in memory.
#[derive(Debug, Default)]
pub struct InMemoryLineageStore {
    records: std::sync::RwLock<Vec<RunLineage>>,
    limits: std::sync::RwLock<
        std::collections::HashMap<(AlgorithmId, String), crate::domain::plan::limits::PlanLimits>,
    >,
}

impl InMemoryLineageStore {
    /// Construct a new empty `InMemoryLineageStore`.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl RunLineageStore for InMemoryLineageStore {
    async fn insert(&self, lineage: &RunLineage) -> Result<(), AnalyticsError> {
        // Clone and store to simulate durable insert
        let record = lineage.clone();
        self.records.write().unwrap().push(record);
        Ok(())
    }

    async fn get(&self, run_id: Uuid) -> Result<RunLineage, AnalyticsError> {
        let records = self.records.read().unwrap();
        records
            .iter()
            .find(|r| r.run_id == run_id)
            .cloned()
            .ok_or_else(|| AnalyticsError::RunNotFound(format!("run {} not found", run_id)))
    }

    async fn query(
        &self,
        filter: RunLineageFilter,
        limit: Option<u64>,
    ) -> Result<Vec<RunLineage>, AnalyticsError> {
        let records = self.records.read().unwrap();
        let mut results: Vec<RunLineage> = records
            .iter()
            .filter(|r| {
                filter
                    .workspace_id
                    .as_ref()
                    .map_or(true, |wid| &r.workspace_id == wid)
                    && filter
                        .revision_id
                        .as_ref()
                        .map_or(true, |rid| &r.revision_id == rid)
                    && filter
                        .algorithm_id
                        .as_ref()
                        .map_or(true, |aid| &r.algorithm_id == aid)
                    && filter.status.as_ref().map_or(true, |s| &r.status == s)
            })
            .cloned()
            .collect();

        results.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        if let Some(limit) = limit {
            results.truncate(limit as usize);
        }

        Ok(results)
    }

    async fn upsert_descriptor_limits(
        &self,
        algorithm_id: &AlgorithmId,
        version: &str,
        limits: &crate::domain::plan::limits::PlanLimits,
    ) -> Result<(), AnalyticsError> {
        let key = (algorithm_id.clone(), version.to_string());
        self.limits.write().unwrap().insert(key, limits.clone());
        Ok(())
    }

    async fn get_descriptor_limits(
        &self,
        algorithm_id: &AlgorithmId,
        version: &str,
    ) -> Result<Option<crate::domain::plan::limits::PlanLimits>, AnalyticsError> {
        let key = (algorithm_id.clone(), version.to_string());
        Ok(self.limits.read().unwrap().get(&key).cloned())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::WorkspaceId;

    fn dummy_workspace() -> WorkspaceId {
        WorkspaceId::try_new("test-workspace").expect("valid workspace id")
    }

    fn dummy_revision() -> RevisionId {
        RevisionId::new(1)
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
        lineage.truncate(TruncationMarker::ResultRowsLimit, 10);
        assert_eq!(lineage.status, RunStatus::Truncated);
        assert_eq!(
            lineage.truncation_marker,
            Some(TruncationMarker::ResultRowsLimit)
        );
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

    // ========================================================================
    // InMemoryLineageStore tests
    // ========================================================================

    use super::{InMemoryLineageStore, RunLineageFilter, RunLineageStore, TruncationMarker};

    #[tokio::test]
    async fn in_memory_lineage_store_insert_and_get() {
        let store = InMemoryLineageStore::new();
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
        let run_id = lineage.run_id.clone();
        store.insert(&lineage).await.unwrap();

        let retrieved = store.get(run_id).await.unwrap();
        assert_eq!(retrieved.algorithm_id.as_str(), "pagerank");
    }

    #[tokio::test]
    async fn in_memory_lineage_store_query_empty() {
        let store = InMemoryLineageStore::new();
        let results = store
            .query(RunLineageFilter::default(), None)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn in_memory_lineage_store_query_by_workspace() {
        let store = InMemoryLineageStore::new();
        let ws = dummy_workspace();
        let lineage = RunLineage::new(
            ws.clone(),
            dummy_revision(),
            AlgorithmId::from_static("pagerank"),
            "v1.0.0".into(),
            vec![],
            serde_json::json!({}),
            None,
            AnalyticsMode::Stream,
        );
        store.insert(&lineage).await.unwrap();

        let filter = RunLineageFilter {
            workspace_id: Some(ws),
            ..Default::default()
        };
        let results = store.query(filter, None).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn in_memory_lineage_store_query_with_limit() {
        let store = InMemoryLineageStore::new();
        for i in 0..5 {
            let mut lineage = RunLineage::new(
                dummy_workspace(),
                dummy_revision(),
                AlgorithmId::from_static("pagerank"),
                "v1.0.0".into(),
                vec![],
                serde_json::json!({"i": i}),
                None,
                AnalyticsMode::Stream,
            );
            lineage.succeed(i);
            store.insert(&lineage).await.unwrap();
        }

        let results = store
            .query(RunLineageFilter::default(), Some(3))
            .await
            .unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn in_memory_lineage_store_get_not_found() {
        let store = InMemoryLineageStore::new();
        let result = store.get(Uuid::new_v4()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn in_memory_lineage_store_ephemeral_nature() {
        // Verify that data is in-memory only by checking that a new store instance
        // does not retain data from a previous one.
        let store1 = InMemoryLineageStore::new();
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
        store1.insert(&lineage).await.unwrap();

        let store2 = InMemoryLineageStore::new();
        let results = store2
            .query(RunLineageFilter::default(), None)
            .await
            .unwrap();
        assert!(results.is_empty());
    }
}
