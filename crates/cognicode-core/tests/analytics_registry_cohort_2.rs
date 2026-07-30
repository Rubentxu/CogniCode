//! Tests for E28.5 Cohort-2 structural algorithm descriptors (Dominators, ArticulationPoints, Bridges, KCore).
//!
//! Part of E28.5 Structural Analytics Cohort 2 — PR2 Descriptors.

use std::sync::Arc;

use cognicode_core::application::services::graph_analytics::AlgorithmRegistry;
use cognicode_core::domain::analytics::{
    AdmissionError, AlgorithmDescriptor, AlgorithmId, AnalyticsError, RunLineage, RunLineageFilter,
    RunLineageStore, Uuid, articulation_descriptor::ArticulationPointsDescriptor,
    bridges_descriptor::BridgesDescriptor, dominators_descriptor::DominatorsDescriptor,
    kcore_descriptor::KCoreDescriptor,
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
// Dominators descriptor tests
// =============================================================================

#[test]
fn dominators_descriptor_has_correct_identity() {
    let d = DominatorsDescriptor;
    let id = d.identity();
    assert_eq!(id.id.as_str(), "dominators");
    assert_eq!(id.version.as_str(), "1.0.0");
    assert_eq!(
        id.maturity,
        cognicode_core::domain::analytics::Maturity::Stable
    );
    assert_eq!(id.cohort, 2);
}

#[test]
fn dominators_descriptor_params_accepted() {
    let d = DominatorsDescriptor;
    let params = serde_json::json!({ "root_symbol": "main" });
    assert!(d.params().validate(&params).is_ok());
}

#[test]
fn dominators_descriptor_rejects_missing_root_symbol() {
    let d = DominatorsDescriptor;
    let params = serde_json::json!({});
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn dominators_descriptor_rejects_non_string_root_symbol() {
    let d = DominatorsDescriptor;
    let params = serde_json::json!({ "root_symbol": 123 });
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn dominators_descriptor_output_schema_has_three_fields() {
    let d = DominatorsDescriptor;
    let schema = d.output_schema();
    assert_eq!(schema.fields.len(), 3);
    assert_eq!(schema.fields[0].name, "nodes");
    assert_eq!(schema.fields[1].name, "immediate_dominators");
    assert_eq!(schema.fields[2].name, "depths");
}

#[test]
fn dominators_descriptor_is_directed() {
    let d = DominatorsDescriptor;
    assert!(d.directed());
    assert!(!d.weighted());
    assert!(!d.heterogeneous());
}

#[test]
fn dominators_descriptor_has_conformance_fixtures() {
    let d = DominatorsDescriptor;
    let fixtures = d.conformance_fixtures();
    assert!(!fixtures.is_empty());
    // Verify chain fixture
    let chain = fixtures
        .iter()
        .find(|f| f.name == "chain dominators")
        .unwrap();
    assert_eq!(chain.graph.nodes, vec!["A", "B", "C"]);
    assert_eq!(chain.graph.edges.len(), 2);
}

// =============================================================================
// ArticulationPoints descriptor tests
// =============================================================================

#[test]
fn articulation_descriptor_has_correct_identity() {
    let d = ArticulationPointsDescriptor;
    let id = d.identity();
    assert_eq!(id.id.as_str(), "articulation_points");
    assert_eq!(id.version.as_str(), "1.0.0");
    assert_eq!(id.cohort, 2);
}

#[test]
fn articulation_descriptor_accepts_null_params() {
    let d = ArticulationPointsDescriptor;
    assert!(d.params().validate(&serde_json::Value::Null).is_ok());
    assert!(d.params().validate(&serde_json::json!({})).is_ok());
}

#[test]
fn articulation_descriptor_rejects_non_empty_params() {
    let d = ArticulationPointsDescriptor;
    let params = serde_json::json!({"foo": "bar"});
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn articulation_descriptor_output_schema_has_two_fields() {
    let d = ArticulationPointsDescriptor;
    let schema = d.output_schema();
    assert_eq!(schema.fields.len(), 2);
    assert_eq!(schema.fields[0].name, "nodes");
    assert_eq!(schema.fields[1].name, "cut_vertices_counts");
}

#[test]
fn articulation_descriptor_is_undirected() {
    let d = ArticulationPointsDescriptor;
    assert!(!d.directed()); // undirected
    assert!(!d.weighted());
}

#[test]
fn articulation_descriptor_has_conformance_fixtures() {
    let d = ArticulationPointsDescriptor;
    let fixtures = d.conformance_fixtures();
    assert!(!fixtures.is_empty());
    // Verify path fixture
    let path = fixtures
        .iter()
        .find(|f| f.name == "path articulation")
        .unwrap();
    assert_eq!(path.graph.nodes, vec!["A", "B", "C"]);
}

// =============================================================================
// Bridges descriptor tests
// =============================================================================

#[test]
fn bridges_descriptor_has_correct_identity() {
    let d = BridgesDescriptor;
    let id = d.identity();
    assert_eq!(id.id.as_str(), "bridges");
    assert_eq!(id.version.as_str(), "1.0.0");
    assert_eq!(id.cohort, 2);
}

#[test]
fn bridges_descriptor_accepts_null_params() {
    let d = BridgesDescriptor;
    assert!(d.params().validate(&serde_json::Value::Null).is_ok());
    assert!(d.params().validate(&serde_json::json!({})).is_ok());
}

#[test]
fn bridges_descriptor_rejects_non_empty_params() {
    let d = BridgesDescriptor;
    let params = serde_json::json!({"foo": "bar"});
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn bridges_descriptor_output_schema_has_one_field() {
    let d = BridgesDescriptor;
    let schema = d.output_schema();
    assert_eq!(schema.fields.len(), 1);
    assert_eq!(schema.fields[0].name, "edges");
}

#[test]
fn bridges_descriptor_is_undirected() {
    let d = BridgesDescriptor;
    assert!(!d.directed()); // undirected
    assert!(!d.weighted());
}

#[test]
fn bridges_descriptor_has_conformance_fixtures() {
    let d = BridgesDescriptor;
    let fixtures = d.conformance_fixtures();
    assert!(!fixtures.is_empty());
    // Verify path fixture
    let path = fixtures
        .iter()
        .find(|f| f.name == "path all bridges")
        .unwrap();
    assert_eq!(path.graph.nodes, vec!["A", "B", "C"]);
}

// =============================================================================
// K-Core descriptor tests
// =============================================================================

#[test]
fn kcore_descriptor_has_correct_identity() {
    let d = KCoreDescriptor;
    let id = d.identity();
    assert_eq!(id.id.as_str(), "k_core");
    assert_eq!(id.version.as_str(), "1.0.0");
    assert_eq!(id.cohort, 2);
}

#[test]
fn kcore_descriptor_params_accepted() {
    let d = KCoreDescriptor;
    let params = serde_json::json!({ "k": 2 });
    assert!(d.params().validate(&params).is_ok());
}

#[test]
fn kcore_descriptor_rejects_missing_k() {
    let d = KCoreDescriptor;
    let params = serde_json::json!({});
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn kcore_descriptor_rejects_non_u64_k() {
    let d = KCoreDescriptor;
    let params = serde_json::json!({ "k": "2" });
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn kcore_descriptor_rejects_k_too_large() {
    let d = KCoreDescriptor;
    let params = serde_json::json!({ "k": 1001 });
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn kcore_descriptor_output_schema_has_two_fields() {
    let d = KCoreDescriptor;
    let schema = d.output_schema();
    assert_eq!(schema.fields.len(), 2);
    assert_eq!(schema.fields[0].name, "nodes");
    assert_eq!(schema.fields[1].name, "core_numbers");
}

#[test]
fn kcore_descriptor_is_undirected() {
    let d = KCoreDescriptor;
    assert!(!d.directed()); // undirected
    assert!(!d.weighted());
}

#[test]
fn kcore_descriptor_has_conformance_fixtures() {
    let d = KCoreDescriptor;
    let fixtures = d.conformance_fixtures();
    assert!(!fixtures.is_empty());
    // Verify triangle fixture
    let triangle = fixtures
        .iter()
        .find(|f| f.name == "triangle k2 all nodes")
        .unwrap();
    assert_eq!(triangle.graph.nodes, vec!["A", "B", "C"]);
}

// =============================================================================
// AlgorithmRegistry admission tests for cohort 2
// =============================================================================

#[test]
fn registry_admits_all_cohort_2_algorithms() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    assert!(registry.admit(Box::new(DominatorsDescriptor)).is_ok());
    assert!(
        registry
            .admit(Box::new(ArticulationPointsDescriptor))
            .is_ok()
    );
    assert!(registry.admit(Box::new(BridgesDescriptor)).is_ok());
    assert!(registry.admit(Box::new(KCoreDescriptor)).is_ok());
}

#[test]
fn registry_returns_admitted_cohort_2_descriptors() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    registry.admit(Box::new(DominatorsDescriptor)).unwrap();
    registry
        .admit(Box::new(ArticulationPointsDescriptor))
        .unwrap();
    registry.admit(Box::new(BridgesDescriptor)).unwrap();
    registry.admit(Box::new(KCoreDescriptor)).unwrap();

    let ids: Vec<_> = registry
        .admitted()
        .map(|d| d.identity().id.as_str())
        .collect();
    assert!(ids.contains(&"dominators"));
    assert!(ids.contains(&"articulation_points"));
    assert!(ids.contains(&"bridges"));
    assert!(ids.contains(&"k_core"));
}

#[test]
fn registry_is_admitted_cohort_2_check() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    registry.admit(Box::new(DominatorsDescriptor)).unwrap();

    let dominators_id = cognicode_core::domain::analytics::AlgorithmId::from_static("dominators");
    let bridges_id = cognicode_core::domain::analytics::AlgorithmId::from_static("bridges");

    assert!(registry.is_admitted(&dominators_id));
    assert!(!registry.is_admitted(&bridges_id)); // not admitted yet
}

#[test]
fn registry_get_returns_cohort_2_descriptor() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    registry.admit(Box::new(KCoreDescriptor)).unwrap();

    let kcore_id = cognicode_core::domain::analytics::AlgorithmId::from_static("k_core");
    let d = registry.get(&kcore_id);
    assert!(d.is_some());
    assert_eq!(d.unwrap().identity().id.as_str(), "k_core");
}

#[test]
fn registry_rejects_duplicate_cohort_2_same_version() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    assert!(
        registry
            .admit(Box::new(ArticulationPointsDescriptor))
            .is_ok()
    );
    let result = registry.admit(Box::new(ArticulationPointsDescriptor));
    assert!(matches!(result, Err(AdmissionError::AlreadyAdmitted(_, _))));
}
