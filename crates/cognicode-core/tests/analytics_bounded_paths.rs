//! Tests for E28.4 Bounded Shortest Paths descriptor.
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR3 Bounded Paths.

use cognicode_core::domain::analytics::bounded_shortest_paths_descriptor::BoundedShortestPathsDescriptor;
use cognicode_core::domain::analytics::{AlgorithmDescriptor, AlgorithmId, AnalyticsMode, RunLineageStore};
use cognicode_core::domain::plan::limits::PlanLimits;

/// A no-op lineage store for testing.
struct NoOpLineageStore;

#[async_trait::async_trait]
impl RunLineageStore for NoOpLineageStore {
    async fn insert(&self, _lineage: &cognicode_core::domain::analytics::RunLineage) -> Result<(), cognicode_core::domain::analytics::AnalyticsError> {
        Ok(())
    }

    async fn get(&self, _run_id: cognicode_core::domain::analytics::Uuid) -> Result<cognicode_core::domain::analytics::RunLineage, cognicode_core::domain::analytics::AnalyticsError> {
        Err(cognicode_core::domain::analytics::AnalyticsError::RunNotFound("not found".into()))
    }

    async fn query(
        &self,
        _filter: cognicode_core::domain::analytics::RunLineageFilter,
        _limit: Option<u64>,
    ) -> Result<Vec<cognicode_core::domain::analytics::RunLineage>, cognicode_core::domain::analytics::AnalyticsError> {
        Ok(vec![])
    }

    async fn upsert_descriptor_limits(
        &self,
        _algorithm_id: &AlgorithmId,
        _version: &str,
        _limits: &PlanLimits,
    ) -> Result<(), cognicode_core::domain::analytics::AnalyticsError> {
        Ok(())
    }

    async fn get_descriptor_limits(
        &self,
        _algorithm_id: &AlgorithmId,
        _version: &str,
    ) -> Result<Option<PlanLimits>, cognicode_core::domain::analytics::AnalyticsError> {
        Ok(None)
    }
}

// =============================================================================
// Bounded Shortest Paths descriptor tests
// =============================================================================

#[test]
fn bsp_descriptor_has_correct_identity() {
    let d = BoundedShortestPathsDescriptor;
    let id = d.identity();
    assert_eq!(id.id.as_str(), "bounded_shortest_paths");
    assert_eq!(id.version.as_str(), "1.0.0");
    assert_eq!(id.maturity, cognicode_core::domain::analytics::Maturity::Stable);
    assert_eq!(id.cohort, 1);
}

#[test]
fn bsp_descriptor_accepts_valid_params() {
    let d = BoundedShortestPathsDescriptor;
    let params = serde_json::json!({
        "from_symbol": "test.rs:A:1",
        "to_symbol": "test.rs:D:1",
        "max_hops": 5,
        "max_paths": 100
    });
    assert!(d.params().validate(&params).is_ok());
}

#[test]
fn bsp_descriptor_accepts_params_without_max_paths() {
    let d = BoundedShortestPathsDescriptor;
    let params = serde_json::json!({
        "from_symbol": "test.rs:A:1",
        "to_symbol": "test.rs:D:1",
        "max_hops": 5
    });
    assert!(d.params().validate(&params).is_ok());
}

#[test]
fn bsp_descriptor_rejects_missing_max_hops() {
    let d = BoundedShortestPathsDescriptor;
    let params = serde_json::json!({
        "from_symbol": "test.rs:A:1",
        "to_symbol": "test.rs:D:1",
        "max_paths": 100
    });
    assert!(d.params().validate(&params).is_err());
    let err = d.params().validate(&params).unwrap_err();
    assert!(err.contains("max_hops"));
}

#[test]
fn bsp_descriptor_rejects_zero_max_hops() {
    let d = BoundedShortestPathsDescriptor;
    let params = serde_json::json!({
        "from_symbol": "test.rs:A:1",
        "to_symbol": "test.rs:D:1",
        "max_hops": 0
    });
    assert!(d.params().validate(&params).is_err());
    let err = d.params().validate(&params).unwrap_err();
    assert!(err.contains("> 0"));
}

#[test]
fn bsp_descriptor_rejects_zero_max_paths() {
    let d = BoundedShortestPathsDescriptor;
    let params = serde_json::json!({
        "from_symbol": "test.rs:A:1",
        "to_symbol": "test.rs:D:1",
        "max_hops": 5,
        "max_paths": 0
    });
    assert!(d.params().validate(&params).is_err());
    let err = d.params().validate(&params).unwrap_err();
    assert!(err.contains("max_paths"));
}

#[test]
fn bsp_descriptor_output_schema_has_three_fields() {
    let d = BoundedShortestPathsDescriptor;
    let schema = d.output_schema();
    assert_eq!(schema.fields.len(), 3);
    assert_eq!(schema.fields[0].name, "path_id");
    assert_eq!(schema.fields[1].name, "nodes");
    assert_eq!(schema.fields[2].name, "cost");
}

#[test]
fn bsp_descriptor_supports_only_stream_and_persist() {
    let d = BoundedShortestPathsDescriptor;
    let modes = d.supported_modes();
    assert!(modes.contains(&AnalyticsMode::Stream));
    assert!(modes.contains(&AnalyticsMode::Persist));
    assert!(!modes.contains(&AnalyticsMode::Stats));
    assert!(!modes.contains(&AnalyticsMode::Annotate));
}

#[test]
fn bsp_descriptor_is_directed() {
    let d = BoundedShortestPathsDescriptor;
    assert!(d.directed());
    assert!(!d.weighted());
    assert!(!d.heterogeneous());
}

#[test]
fn bsp_descriptor_has_conformance_fixtures() {
    let d = BoundedShortestPathsDescriptor;
    let fixtures = d.conformance_fixtures();
    assert!(!fixtures.is_empty());
    // Verify diamond fixture
    let diamond = fixtures.iter().find(|f| f.name == "diamond three paths").unwrap();
    assert_eq!(diamond.graph.nodes, vec!["A", "B", "C", "D"]);
    assert_eq!(diamond.graph.edges.len(), 5);
}

#[test]
fn bsp_descriptor_has_correct_complexity() {
    let d = BoundedShortestPathsDescriptor;
    let complexity = d.complexity();
    assert!(complexity.time.contains("k·d"));
}

#[test]
fn bsp_descriptor_projection_is_outgoing() {
    let d = BoundedShortestPathsDescriptor;
    use cognicode_core::domain::analytics::ProjectionAssumption;
    assert!(matches!(
        d.projection_assumption(),
        &ProjectionAssumption::CallGraphOutgoing
    ));
}

// =============================================================================
// AlgorithmRegistry admission tests for BSP
// =============================================================================

#[test]
fn registry_admits_bounded_shortest_paths() {
    use cognicode_core::application::services::graph_analytics::AlgorithmRegistry;
    use std::sync::Arc;

    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    let result = registry.admit(Box::new(BoundedShortestPathsDescriptor));
    assert!(result.is_ok());
}

#[test]
fn registry_returns_bounded_shortest_paths() {
    use cognicode_core::application::services::graph_analytics::AlgorithmRegistry;
    use std::sync::Arc;

    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    registry.admit(Box::new(BoundedShortestPathsDescriptor)).unwrap();

    let ids: Vec<_> = registry.admitted().map(|d| d.identity().id.as_str()).collect();
    assert!(ids.contains(&"bounded_shortest_paths"));
}

#[test]
fn registry_get_returns_bounded_shortest_paths_descriptor() {
    use cognicode_core::application::services::graph_analytics::AlgorithmRegistry;
    use std::sync::Arc;

    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    registry.admit(Box::new(BoundedShortestPathsDescriptor)).unwrap();

    let bsp_id = cognicode_core::domain::analytics::AlgorithmId::from_static("bounded_shortest_paths");
    let d = registry.get(&bsp_id);
    assert!(d.is_some());
    assert_eq!(d.unwrap().identity().id.as_str(), "bounded_shortest_paths");
}

// =============================================================================
// all_simple_paths algorithm tests (graph-algos integration)
// =============================================================================

fn sym(name: &str) -> cognicode_core::domain::aggregates::Symbol {
    use cognicode_core::domain::value_objects::{Location, SymbolKind};
    cognicode_core::domain::aggregates::Symbol::new(name, SymbolKind::Function, Location::new("test.rs", 1, 1))
}

fn id(name: &str) -> cognicode_core::domain::aggregates::SymbolId {
    cognicode_core::domain::aggregates::SymbolId::new(format!("test.rs:{name}:1"))
}

fn add_edge(g: &mut cognicode_core::domain::aggregates::CallGraph, a: &str, b: &str) {
    use cognicode_core::domain::services::ExtractionContext;
    use cognicode_core::domain::value_objects::DependencyType;
    g.add_symbol(sym(a));
    g.add_symbol(sym(b));
    let _ = g.add_dependency_with_provenance(
        &id(a),
        &id(b),
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    );
}

#[test]
fn all_simple_paths_single_hop() {
    // A → B: one path with max_hops=1
    use cognicode_core::application::services::graph_analytics::GraphAnalyticsService;
    use cognicode_core::domain::aggregates::CallGraph;

    let mut g = CallGraph::new();
    add_edge(&mut g, "A", "B");

    let paths = GraphAnalyticsService::all_simple_paths(&g, &id("A"), &id("B"), 1);
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0], vec![id("A"), id("B")]);
}

#[test]
fn all_simple_paths_multi_hop() {
    // A → B → C → D: chain with max_hops=5
    use cognicode_core::application::services::graph_analytics::GraphAnalyticsService;
    use cognicode_core::domain::aggregates::CallGraph;

    let mut g = CallGraph::new();
    add_edge(&mut g, "A", "B");
    add_edge(&mut g, "B", "C");
    add_edge(&mut g, "C", "D");

    let paths = GraphAnalyticsService::all_simple_paths(&g, &id("A"), &id("D"), 5);
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].len(), 4); // A, B, C, D
}

#[test]
fn all_simple_paths_max_hops_respected() {
    // A → B → C → D: chain but max_hops=1 should return empty
    use cognicode_core::application::services::graph_analytics::GraphAnalyticsService;
    use cognicode_core::domain::aggregates::CallGraph;

    let mut g = CallGraph::new();
    add_edge(&mut g, "A", "B");
    add_edge(&mut g, "B", "C");
    add_edge(&mut g, "C", "D");

    let paths = GraphAnalyticsService::all_simple_paths(&g, &id("A"), &id("D"), 1);
    assert!(paths.is_empty()); // No direct edge A→D
}

#[test]
fn all_simple_paths_empty_when_no_path_within_bound() {
    // A → B, but asking for path from A to C (unreachable)
    use cognicode_core::application::services::graph_analytics::GraphAnalyticsService;
    use cognicode_core::domain::aggregates::CallGraph;

    let mut g = CallGraph::new();
    g.add_symbol(sym("A"));
    g.add_symbol(sym("B"));
    g.add_symbol(sym("C"));
    // Note: C is never connected
    add_edge(&mut g, "A", "B");

    let paths = GraphAnalyticsService::all_simple_paths(&g, &id("A"), &id("C"), 5);
    assert!(paths.is_empty());
}

#[test]
fn all_simple_paths_diamond_dag_three_paths() {
    // Diamond: A → B → D, A → C → D, A → D (direct)
    // Three paths from A to D with max_hops=5
    use cognicode_core::application::services::graph_analytics::GraphAnalyticsService;
    use cognicode_core::domain::aggregates::CallGraph;

    let mut g = CallGraph::new();
    add_edge(&mut g, "A", "B");
    add_edge(&mut g, "A", "C");
    add_edge(&mut g, "A", "D");
    add_edge(&mut g, "B", "D");
    add_edge(&mut g, "C", "D");

    let paths = GraphAnalyticsService::all_simple_paths(&g, &id("A"), &id("D"), 5);
    assert_eq!(paths.len(), 3); // Direct, via B, via C
}
