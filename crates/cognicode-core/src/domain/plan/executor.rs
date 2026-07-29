//! GraphExecutor — backend-neutral port trait for graph plan execution.
//!
//! Part of e28-2-differential-graph-executors: PR1 Port Phase 1.

use std::fmt;

use super::{
    GraphPlan, PlanLimits, PlanLimitKind, ResultSet,
    TypedValue,
};
use crate::domain::value_objects::{RevisionId, WorkspaceId};
use super::ExecutorError;

/// A provenance envelope carrying source-side provenance information per result row.
///
/// The envelope wraps backend-specific provenance (e.g., SQL query text, execution
/// time, snapshot version) in a normalized form so that conformance tests can
/// compare results from different backends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceEnvelope {
    /// Backend identifier (e.g., "Postgres", "Snapshot").
    pub backend: String,
    /// Execution time in milliseconds.
    pub execution_time_ms: Option<u64>,
    /// Snapshot revision if applicable.
    pub revision: Option<(WorkspaceId, RevisionId)>,
    /// Additional key-value metadata.
    pub metadata: Vec<(String, TypedValue)>,
}

impl ProvenanceEnvelope {
    /// Construct an empty provenance envelope for a given backend.
    pub fn new(backend: impl Into<String>) -> Self {
        Self {
            backend: backend.into(),
            execution_time_ms: None,
            revision: None,
            metadata: Vec::new(),
        }
    }

    /// Set the execution time.
    pub fn with_execution_time(mut self, ms: u64) -> Self {
        self.execution_time_ms = Some(ms);
        self
    }

    /// Set the workspace and revision.
    pub fn with_revision(mut self, ws: WorkspaceId, rev: RevisionId) -> Self {
        self.revision = Some((ws, rev));
        self
    }
}

/// Backend-neutral trait for executing a pinned `GraphPlan`.
///
/// Implementors must be `Send + Sync + 'static` so the executor can be
/// shared across async tasks and thread pools without synchronization issues.
pub trait GraphExecutor: Send + Sync + fmt::Debug {
    /// Execute a graph plan against the specified workspace revision.
    ///
    /// The executor MUST NOT read graph state for any revision other than `pin.1`.
    /// If the revision does not exist, the executor MUST return
    /// `Err(ExecutorError::RevisionUnknown(_))`.
    fn execute(
        &self,
        plan: &GraphPlan,
        pin: (WorkspaceId, RevisionId),
    ) -> Result<ResultSet, ExecutorError>;

    /// Execute with explicit limits override.
    ///
    /// If `limits` is `Some`, those limits override `plan.limits()`.
    /// If `limits` is `None`, `plan.limits()` is used.
    fn execute_with_limits(
        &self,
        plan: &GraphPlan,
        pin: (WorkspaceId, RevisionId),
        limits: Option<PlanLimits>,
    ) -> Result<ResultSet, ExecutorError> {
        let limits = limits.unwrap_or_else(|| plan.limits().clone());
        // Default implementation using execute — override for performance
        let mut result = self.execute(plan, pin)?;
        // Apply soft limit truncation if needed
        if let Some(max_rows) = limits.max_result_rows {
            if result.rows.len() as u64 > max_rows {
                result = result.with_truncation(super::TruncationMarker::ResultRowsLimit);
            }
        }
        Ok(result)
    }
}

/// Stub executor used for trait verification and testing.
#[derive(Debug, Clone, Default)]
pub struct StubExecutor;

impl StubExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl GraphExecutor for StubExecutor {
    fn execute(
        &self,
        _plan: &GraphPlan,
        _pin: (WorkspaceId, RevisionId),
    ) -> Result<ResultSet, ExecutorError> {
        Ok(ResultSet::empty())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 1.1a RED — Trait object-safety assertion
    // Scenario: graph-executor-port::Trait is object-safe
    // Assert: `fn _executor(_: &dyn GraphExecutor) {}` compiles
    // -------------------------------------------------------------------------

    /// `GraphExecutor` must be object-safe — `&dyn GraphExecutor` must compile.
    #[test]
    fn trait_is_object_safe() {
        fn _executor(_: &dyn GraphExecutor) {}
    }

    // -------------------------------------------------------------------------
    // Task 1.2 RED/GREEN — StubExecutor implements GraphExecutor returning empty
    // Scenario: graph-executor-port::Trait is implementable
    // Assert: `StubExecutor` compiles and returns `Ok(ResultSet::empty())`
    // -------------------------------------------------------------------------

    /// `StubExecutor` implements `GraphExecutor` and returns `Ok(ResultSet::empty())`.
    #[test]
    fn stub_executor_returns_empty() {
        use super::super::{GraphPlan, PlanMetadata, PlanVersion, PlanHash};
        let executor = StubExecutor::new();
        let plan = GraphPlan::Neighbors {
            src: "A".into(),
            kind: super::super::NeighborKind::Both,
            depth: 1,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let ws = WorkspaceId::try_new("ws1").unwrap();
        let rev = RevisionId::new(3);
        let result = executor.execute(&plan, (ws, rev));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    /// `GraphExecutor` must be `Send + Sync + 'static`.
    #[test]
    fn graph_executor_send_sync_static() {
        fn assert_send<T: Send + ?Sized>() {}
        fn assert_sync<T: Sync + ?Sized>() {}
        fn assert_static<T: 'static + ?Sized>() {}
        assert_send::<dyn GraphExecutor>();
        assert_sync::<dyn GraphExecutor>();
        assert_static::<dyn GraphExecutor>();
    }

    /// `StubExecutor` is `Send + Sync + 'static`.
    #[test]
    fn stub_executor_send_sync_static() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        fn assert_static<T: 'static>() {}
        assert_send::<StubExecutor>();
        assert_sync::<StubExecutor>();
        assert_static::<StubExecutor>();
    }

    /// `ProvenanceEnvelope::new` creates an empty envelope.
    #[test]
    fn provenance_envelope_new() {
        let env = ProvenanceEnvelope::new("Postgres");
        assert_eq!(env.backend, "Postgres");
        assert!(env.execution_time_ms.is_none());
        assert!(env.revision.is_none());
        assert!(env.metadata.is_empty());
    }

    /// `ProvenanceEnvelope::with_execution_time` sets the time.
    #[test]
    fn provenance_envelope_with_time() {
        let env = ProvenanceEnvelope::new("Postgres").with_execution_time(42);
        assert_eq!(env.execution_time_ms, Some(42));
    }

    /// `ProvenanceEnvelope::with_revision` sets the revision.
    #[test]
    fn provenance_envelope_with_revision() {
        let ws = WorkspaceId::try_new("ws1").unwrap();
        let rev = RevisionId::new(3);
        let env = ProvenanceEnvelope::new("Snapshot").with_revision(ws, rev);
        assert_eq!(
            env.revision,
            Some((WorkspaceId::try_new("ws1").unwrap(), RevisionId::new(3)))
        );
    }

    /// `ProvenanceEnvelope` is `Send + Sync + 'static`.
    #[test]
    fn provenance_envelope_send_sync_static() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        fn assert_static<T: 'static>() {}
        assert_send::<ProvenanceEnvelope>();
        assert_sync::<ProvenanceEnvelope>();
        assert_static::<ProvenanceEnvelope>();
    }

    /// `ExecutorError::RevisionUnknown` format matches spec: `"ws:rev"`.
    #[test]
    fn executor_error_revision_unknown_format() {
        let err = ExecutorError::RevisionUnknown("ws1:99".into());
        let display = err.to_string();
        assert!(display.contains("ws1"));
        assert!(display.contains("99") || display.contains("revision unknown"));
    }
}
