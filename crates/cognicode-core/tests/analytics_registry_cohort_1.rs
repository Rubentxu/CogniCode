//! Tests for E28.4 Cohort-1 core algorithm descriptors (PageRank, SCC, WCC).
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR2 Cohort-1 Core.

use std::sync::Arc;

use cognicode_core::application::services::graph_analytics::AlgorithmRegistry;
use cognicode_core::domain::analytics::{
    AdmissionError, AlgorithmDescriptor, AlgorithmId, AnalyticsError, RunLineage, RunLineageFilter,
    RunLineageStore, Uuid, pagerank_descriptor::PageRankDescriptor, scc_descriptor::SccDescriptor,
    wcc_descriptor::WccDescriptor,
};
use cognicode_core::domain::plan::limits::PlanLimits;

/// A no-op lineage store for testing.
struct NoOpLineageStore;

#[async_trait::async_trait]
impl RunLineageStore for NoOpLineageStore {
    async fn insert(&self, _lineage: &RunLineage) -> Result<(), AnalyticsError> {
        Ok(())
    }

    async fn get(&self, _run_id: Uuid) -> Result<RunLineage, AnalyticsError> {
        Err(AnalyticsError::RunNotFound("not found".into()))
    }

    async fn query(
        &self,
        _filter: RunLineageFilter,
        _limit: Option<u64>,
    ) -> Result<Vec<RunLineage>, AnalyticsError> {
        Ok(vec![])
    }

    async fn upsert_descriptor_limits(
        &self,
        _algorithm_id: &AlgorithmId,
        _version: &str,
        _limits: &PlanLimits,
    ) -> Result<(), AnalyticsError> {
        Ok(())
    }

    async fn get_descriptor_limits(
        &self,
        _algorithm_id: &AlgorithmId,
        _version: &str,
    ) -> Result<Option<PlanLimits>, AnalyticsError> {
        Ok(None)
    }
}

// =============================================================================
// PageRank descriptor tests
// =============================================================================

#[test]
fn pagerank_descriptor_has_correct_identity() {
    let d = PageRankDescriptor;
    let id = d.identity();
    assert_eq!(id.id.as_str(), "pagerank");
    assert_eq!(id.version.as_str(), "1.0.0");
    assert_eq!(
        id.maturity,
        cognicode_core::domain::analytics::Maturity::Stable
    );
    assert_eq!(id.cohort, 1);
}

#[test]
fn pagerank_descriptor_params_accepted() {
    let d = PageRankDescriptor;
    let params = serde_json::json!({
        "alpha": 0.85,
        "max_iterations": 100,
        "epsilon": 1e-6
    });
    assert!(d.params().validate(&params).is_ok());
}

#[test]
fn pagerank_descriptor_rejects_missing_params() {
    let d = PageRankDescriptor;
    let params = serde_json::json!({
        "alpha": 0.85
        // missing max_iterations and epsilon
    });
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn pagerank_descriptor_output_schema_has_two_fields() {
    let d = PageRankDescriptor;
    let schema = d.output_schema();
    assert_eq!(schema.fields.len(), 2);
    assert_eq!(schema.fields[0].name, "node_id");
    assert_eq!(schema.fields[1].name, "score");
}

#[test]
fn pagerank_descriptor_supports_all_modes() {
    let d = PageRankDescriptor;
    let modes = d.supported_modes();
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Stream));
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Stats));
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Annotate));
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Persist));
}

#[test]
fn pagerank_descriptor_is_directed() {
    let d = PageRankDescriptor;
    assert!(d.directed());
    assert!(!d.weighted());
    assert!(!d.heterogeneous());
}

#[test]
fn pagerank_descriptor_has_conformance_fixtures() {
    let d = PageRankDescriptor;
    let fixtures = d.conformance_fixtures();
    assert!(!fixtures.is_empty());
    // Verify 3-node cycle fixture
    let cycle = fixtures.iter().find(|f| f.name == "3-node cycle").unwrap();
    assert_eq!(cycle.graph.nodes, vec!["A", "B", "C"]);
    assert_eq!(cycle.graph.edges.len(), 3);
}

// =============================================================================
// SCC descriptor tests
// =============================================================================

#[test]
fn scc_descriptor_has_correct_identity() {
    let d = SccDescriptor;
    let id = d.identity();
    assert_eq!(id.id.as_str(), "scc");
    assert_eq!(id.version.as_str(), "1.0.0");
}

#[test]
fn scc_descriptor_accepts_null_params() {
    let d = SccDescriptor;
    assert!(d.params().validate(&serde_json::Value::Null).is_ok());
    assert!(d.params().validate(&serde_json::json!({})).is_ok());
}

#[test]
fn scc_descriptor_rejects_non_empty_params() {
    let d = SccDescriptor;
    let params = serde_json::json!({"foo": "bar"});
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn scc_descriptor_output_schema_has_three_fields() {
    let d = SccDescriptor;
    let schema = d.output_schema();
    assert_eq!(schema.fields.len(), 3);
    assert_eq!(schema.fields[0].name, "node_id");
    assert_eq!(schema.fields[1].name, "scc_id");
    assert_eq!(schema.fields[2].name, "total_sccs");
}

#[test]
fn scc_descriptor_is_directed() {
    let d = SccDescriptor;
    assert!(d.directed());
    assert!(!d.weighted());
}

#[test]
fn scc_descriptor_has_conformance_fixtures() {
    let d = SccDescriptor;
    let fixtures = d.conformance_fixtures();
    assert!(!fixtures.is_empty());
    // Verify 3-node cycle single SCC
    let cycle = fixtures
        .iter()
        .find(|f| f.name == "3-node cycle single SCC")
        .unwrap();
    assert_eq!(cycle.graph.nodes, vec!["A", "B", "C"]);
    assert_eq!(cycle.graph.edges.len(), 3);
}

// =============================================================================
// WCC descriptor tests
// =============================================================================

#[test]
fn wcc_descriptor_has_correct_identity() {
    let d = WccDescriptor;
    let id = d.identity();
    assert_eq!(id.id.as_str(), "wcc");
    assert_eq!(id.version.as_str(), "1.0.0");
}

#[test]
fn wcc_descriptor_accepts_null_params() {
    let d = WccDescriptor;
    assert!(d.params().validate(&serde_json::Value::Null).is_ok());
    assert!(d.params().validate(&serde_json::json!({})).is_ok());
}

#[test]
fn wcc_descriptor_rejects_non_empty_params() {
    let d = WccDescriptor;
    let params = serde_json::json!({"foo": "bar"});
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn wcc_descriptor_output_schema_has_three_fields() {
    let d = WccDescriptor;
    let schema = d.output_schema();
    assert_eq!(schema.fields.len(), 3);
    assert_eq!(schema.fields[0].name, "node_id");
    assert_eq!(schema.fields[1].name, "component_id");
    assert_eq!(schema.fields[2].name, "total_components");
}

#[test]
fn wcc_descriptor_is_undirected() {
    let d = WccDescriptor;
    assert!(!d.directed()); // WCC treats graph as undirected
    assert!(!d.weighted());
}

#[test]
fn wcc_descriptor_has_conformance_fixtures() {
    let d = WccDescriptor;
    let fixtures = d.conformance_fixtures();
    assert!(!fixtures.is_empty());
    // Verify chain fixture
    let chain = fixtures
        .iter()
        .find(|f| f.name == "chain one component")
        .unwrap();
    assert_eq!(chain.graph.nodes, vec!["A", "B", "C"]);
    assert_eq!(chain.graph.edges.len(), 2);
}

// =============================================================================
// AlgorithmRegistry admission tests
// =============================================================================

#[test]
fn registry_admits_pagerank_scc_wcc() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    assert!(registry.admit(Box::new(PageRankDescriptor)).is_ok());
    assert!(registry.admit(Box::new(SccDescriptor)).is_ok());
    assert!(registry.admit(Box::new(WccDescriptor)).is_ok());
}

#[test]
fn registry_returns_admitted_descriptors() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    registry.admit(Box::new(PageRankDescriptor)).unwrap();
    registry.admit(Box::new(SccDescriptor)).unwrap();
    registry.admit(Box::new(WccDescriptor)).unwrap();

    let ids: Vec<_> = registry
        .admitted()
        .map(|d| d.identity().id.as_str())
        .collect();
    assert!(ids.contains(&"pagerank"));
    assert!(ids.contains(&"scc"));
    assert!(ids.contains(&"wcc"));
}

#[test]
fn registry_is_admitted_check() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    registry.admit(Box::new(PageRankDescriptor)).unwrap();

    let pagerank_id = cognicode_core::domain::analytics::AlgorithmId::from_static("pagerank");
    let scc_id = cognicode_core::domain::analytics::AlgorithmId::from_static("scc");

    assert!(registry.is_admitted(&pagerank_id));
    assert!(!registry.is_admitted(&scc_id)); // not admitted yet
}

#[test]
fn registry_get_returns_descriptor() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    registry.admit(Box::new(PageRankDescriptor)).unwrap();

    let pagerank_id = cognicode_core::domain::analytics::AlgorithmId::from_static("pagerank");
    let d = registry.get(&pagerank_id);
    assert!(d.is_some());
    assert_eq!(d.unwrap().identity().id.as_str(), "pagerank");
}

#[test]
fn registry_rejects_duplicate_same_version() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    assert!(registry.admit(Box::new(PageRankDescriptor)).is_ok());
    let result = registry.admit(Box::new(PageRankDescriptor));
    assert!(matches!(result, Err(AdmissionError::AlreadyAdmitted(_, _))));
}
