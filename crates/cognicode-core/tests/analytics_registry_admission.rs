//! Tests for AlgorithmRegistry admission and rejection cases.
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR1 Foundation Phase.

use std::sync::Arc;
use std::sync::LazyLock;

use cognicode_core::domain::analytics::{
    AdmissionError, AlgorithmDescriptor, AlgorithmId, AlgorithmVersion, AnalyticsError,
    AnalyticsMode, ComplexityClass, DeterminismKind, Fixture, FixtureGraph, Maturity,
    OutputField, OutputSchema, OutputType, ProjectionAssumption, RunLineage, RunLineageFilter,
    RunLineageStore,
};
use cognicode_core::domain::plan::limits::PlanLimits;
use cognicode_core::application::services::graph_analytics::AlgorithmRegistry;

// =============================================================================
// Test fixtures
// =============================================================================

static ID_PAGERANK: LazyLock<cognicode_core::domain::analytics::AlgorithmIdentity> =
    LazyLock::new(|| cognicode_core::domain::analytics::AlgorithmIdentity {
        id: AlgorithmId::from_static("pagerank"),
        version: AlgorithmVersion::v1(),
        maturity: Maturity::Stable,
        cohort: 1,
    });

static ID_INCOMPLETE2: LazyLock<cognicode_core::domain::analytics::AlgorithmIdentity> =
    LazyLock::new(|| cognicode_core::domain::analytics::AlgorithmIdentity {
        id: AlgorithmId::from_static("incomplete2"),
        version: AlgorithmVersion::v1(),
        maturity: Maturity::Stable,
        cohort: 1,
    });

static PAGERANK_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField { name: "node_id", type_: OutputType::NodeId },
        OutputField { name: "score", type_: OutputType::Score },
    ],
});

static PAGERANK_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
    time_ms: Some(10000),
    cancellation: None,
    max_depth: None,
    max_hops: None,
    max_visited_nodes: Some(1_000_000),
    max_visited_edges: None,
    max_result_rows: Some(10_000),
    max_path_count: None,
    max_memory_bytes: Some(512 * 1024 * 1024),
});

static EMPTY_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
    time_ms: None,
    cancellation: None,
    max_depth: None,
    max_hops: None,
    max_visited_nodes: None,
    max_visited_edges: None,
    max_result_rows: None,
    max_path_count: None,
    max_memory_bytes: None,
});

static EMPTY_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema { fields: vec![] });

static COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E)",
    space: "O(V)",
    notes: "",
});

/// Simple params implementation for testing.
struct SimpleParams {
    names: &'static [&'static str],
}

impl cognicode_core::domain::analytics::AlgorithmParams for SimpleParams {
    fn param_names(&self) -> Vec<&'static str> {
        self.names.to_vec()
    }

    fn validate(&self, _params: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
}

static PAGERANK_PARAMS: LazyLock<SimpleParams> = LazyLock::new(|| SimpleParams {
    names: &["alpha", "max_iterations"],
});

static INCOMPLETE_PARAMS: LazyLock<SimpleParams> = LazyLock::new(|| SimpleParams {
    names: &["alpha"],
});

static PAGERANK_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![Fixture {
        name: "3-node cycle",
        graph: FixtureGraph {
            nodes: vec!["A", "B", "C"],
            edges: vec![("A", "B"), ("B", "C"), ("C", "A")],
        },
        expected: serde_json::json!({"type": "cycle"}),
    }]
});

/// A minimal complete descriptor that passes admission.
struct CompletePagerankDescriptor;

impl AlgorithmDescriptor for CompletePagerankDescriptor {
    fn identity(&self) -> &cognicode_core::domain::analytics::AlgorithmIdentity {
        &ID_PAGERANK
    }

    fn params(&self) -> &dyn cognicode_core::domain::analytics::AlgorithmParams {
        &*PAGERANK_PARAMS
    }

    fn output_schema(&self) -> &OutputSchema {
        &PAGERANK_SCHEMA
    }

    fn supported_modes(&self) -> &[AnalyticsMode] {
        &[AnalyticsMode::Stream, AnalyticsMode::Stats]
    }

    fn complexity(&self) -> &ComplexityClass {
        &COMPLEXITY
    }

    fn limits(&self) -> &PlanLimits {
        &PAGERANK_LIMITS
    }

    fn conformance_fixtures(&self) -> &[Fixture] {
        &PAGERANK_FIXTURES
    }

    fn determinism(&self) -> DeterminismKind {
        DeterminismKind::Seeded { required: false, default: Some(0) }
    }

    fn directed(&self) -> bool {
        true
    }

    fn weighted(&self) -> bool {
        false
    }

    fn heterogeneous(&self) -> bool {
        false
    }

    fn projection_assumption(&self) -> &ProjectionAssumption {
        &ProjectionAssumption::CallGraphIncoming
    }
}

/// Descriptor missing output_schema.
struct IncompleteSchemaDescriptor;

impl AlgorithmDescriptor for IncompleteSchemaDescriptor {
    fn identity(&self) -> &cognicode_core::domain::analytics::AlgorithmIdentity {
        &ID_PAGERANK
    }

    fn params(&self) -> &dyn cognicode_core::domain::analytics::AlgorithmParams {
        &*INCOMPLETE_PARAMS
    }

    fn output_schema(&self) -> &OutputSchema {
        &EMPTY_SCHEMA // EMPTY
    }

    fn supported_modes(&self) -> &[AnalyticsMode] {
        &[AnalyticsMode::Stream]
    }

    fn complexity(&self) -> &ComplexityClass {
        &COMPLEXITY
    }

    fn limits(&self) -> &PlanLimits {
        &PAGERANK_LIMITS
    }

    fn conformance_fixtures(&self) -> &[Fixture] {
        &[]
    }

    fn determinism(&self) -> DeterminismKind {
        DeterminismKind::Deterministic
    }

    fn directed(&self) -> bool {
        true
    }

    fn weighted(&self) -> bool {
        false
    }

    fn heterogeneous(&self) -> bool {
        false
    }

    fn projection_assumption(&self) -> &ProjectionAssumption {
        &ProjectionAssumption::CallGraphIncoming
    }
}

/// Descriptor missing limits (unbounded).
struct IncompleteLimitsDescriptor;

impl AlgorithmDescriptor for IncompleteLimitsDescriptor {
    fn identity(&self) -> &cognicode_core::domain::analytics::AlgorithmIdentity {
        &ID_INCOMPLETE2
    }

    fn params(&self) -> &dyn cognicode_core::domain::analytics::AlgorithmParams {
        &*INCOMPLETE_PARAMS
    }

    fn output_schema(&self) -> &OutputSchema {
        &PAGERANK_SCHEMA
    }

    fn supported_modes(&self) -> &[AnalyticsMode] {
        &[AnalyticsMode::Stream]
    }

    fn complexity(&self) -> &ComplexityClass {
        &COMPLEXITY
    }

    fn limits(&self) -> &PlanLimits {
        &EMPTY_LIMITS // EMPTY - unbounded
    }

    fn conformance_fixtures(&self) -> &[Fixture] {
        &[]
    }

    fn determinism(&self) -> DeterminismKind {
        DeterminismKind::Deterministic
    }

    fn directed(&self) -> bool {
        true
    }

    fn weighted(&self) -> bool {
        false
    }

    fn heterogeneous(&self) -> bool {
        false
    }

    fn projection_assumption(&self) -> &ProjectionAssumption {
        &ProjectionAssumption::CallGraphIncoming
    }
}

/// A noop lineage store for testing.
struct NoopLineageStore;

impl RunLineageStore for NoopLineageStore {
    fn insert(&self, _lineage: &RunLineage) -> Result<(), AnalyticsError> {
        Ok(())
    }

    fn get(&self, _run_id: cognicode_core::domain::analytics::Uuid) -> Result<RunLineage, AnalyticsError> {
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

// =============================================================================
// Tests
// =============================================================================

#[test]
fn admits_complete_descriptor() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoopLineageStore));
    let result = registry.admit(Box::new(CompletePagerankDescriptor));
    assert!(result.is_ok(), "complete descriptor should be admitted: {:?}", result);
    assert!(registry.is_admitted(&AlgorithmId::from_static("pagerank")));
}

#[test]
fn rejects_incomplete_descriptor_missing_output_schema() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoopLineageStore));
    let result = registry.admit(Box::new(IncompleteSchemaDescriptor));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, AdmissionError::Incomplete(_)));
    if let AdmissionError::Incomplete(msg) = err {
        assert!(
            msg.contains("output_schema"),
            "error should mention missing output_schema: {}",
            msg
        );
    }
}

#[test]
fn rejects_incomplete_descriptor_missing_limits() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoopLineageStore));
    let result = registry.admit(Box::new(IncompleteLimitsDescriptor));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, AdmissionError::Incomplete(_)));
    if let AdmissionError::Incomplete(msg) = err {
        assert!(
            msg.contains("limits"),
            "error should mention missing limits: {}",
            msg
        );
    }
}

#[test]
fn rejects_duplicate_admission_same_version() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoopLineageStore));
    registry
        .admit(Box::new(CompletePagerankDescriptor))
        .expect("first admission should succeed");

    // Try to admit the same algorithm again
    let result = registry.admit(Box::new(CompletePagerankDescriptor));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, AdmissionError::AlreadyAdmitted(_, _)));
}

#[test]
fn is_admitted_returns_false_for_non_admitted() {
    let registry = AlgorithmRegistry::new(Arc::new(NoopLineageStore));
    assert!(!registry.is_admitted(&AlgorithmId::from_static("nonexistent")));
    assert!(registry.get(&AlgorithmId::from_static("nonexistent")).is_none());
}
