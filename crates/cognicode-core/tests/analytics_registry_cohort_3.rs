//! Tests for E28.6 Cohort-3 algorithm descriptors (Personalized PageRank, Conductance, Modularity).
//!
//! Part of E28.6 Advanced Analytics Evidence Gate — cohort-3 admission.

use std::sync::Arc;

use cognicode_core::application::services::graph_analytics::AlgorithmRegistry;
use cognicode_core::domain::analytics::{
    AdmissionError, AlgorithmDescriptor, AlgorithmId, AnalyticsError, RunLineage, RunLineageFilter,
    RunLineageStore, Uuid, conductance_descriptor::ConductanceDescriptor,
    modularity_descriptor::ModularityDescriptor,
    personalized_pagerank_descriptor::PersonalizedPageRankDescriptor,
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
// PersonalizedPageRank descriptor tests
// =============================================================================

#[test]
fn personalized_pagerank_descriptor_has_correct_identity() {
    let d = PersonalizedPageRankDescriptor;
    let id = d.identity();
    assert_eq!(id.id.as_str(), "personalized_pagerank");
    assert_eq!(id.version.as_str(), "1.0.0");
    assert_eq!(
        id.maturity,
        cognicode_core::domain::analytics::Maturity::Experimental
    );
    assert_eq!(id.cohort, 3);
}

#[test]
fn personalized_pagerank_descriptor_params_accepted() {
    let d = PersonalizedPageRankDescriptor;
    // alpha + max_iterations required; personalization_vector optional
    let params = serde_json::json!({
        "alpha": 0.85,
        "max_iterations": 100
    });
    assert!(d.params().validate(&params).is_ok());
}

#[test]
fn personalized_pagerank_descriptor_accepts_personalization_vector() {
    let d = PersonalizedPageRankDescriptor;
    let params = serde_json::json!({
        "alpha": 0.85,
        "max_iterations": 100,
        "personalization_vector": ["main", "lib_a"]
    });
    assert!(d.params().validate(&params).is_ok());
}

#[test]
fn personalized_pagerank_descriptor_rejects_missing_alpha() {
    let d = PersonalizedPageRankDescriptor;
    let params = serde_json::json!({ "max_iterations": 100 });
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn personalized_pagerank_descriptor_rejects_missing_max_iterations() {
    let d = PersonalizedPageRankDescriptor;
    let params = serde_json::json!({ "alpha": 0.85 });
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn personalized_pagerank_descriptor_rejects_non_object_params() {
    let d = PersonalizedPageRankDescriptor;
    assert!(d.params().validate(&serde_json::Value::Null).is_err());
    assert!(d.params().validate(&serde_json::json!([])).is_err());
}

#[test]
fn personalized_pagerank_descriptor_output_schema_has_two_fields() {
    let d = PersonalizedPageRankDescriptor;
    let schema = d.output_schema();
    assert_eq!(schema.fields.len(), 2);
    assert_eq!(schema.fields[0].name, "node_id");
    assert_eq!(schema.fields[1].name, "score");
}

#[test]
fn personalized_pagerank_descriptor_is_directed() {
    let d = PersonalizedPageRankDescriptor;
    assert!(d.directed());
    assert!(!d.weighted());
    assert!(!d.heterogeneous());
}

#[test]
fn personalized_pagerank_descriptor_has_conformance_fixtures() {
    let d = PersonalizedPageRankDescriptor;
    let fixtures = d.conformance_fixtures();
    assert!(!fixtures.is_empty());
    // Verify the first fixture (3-node cycle)
    let cycle = fixtures
        .iter()
        .find(|f| f.name.contains("3-node cycle"))
        .unwrap();
    assert_eq!(cycle.graph.nodes, vec!["A", "B", "C"]);
    assert_eq!(cycle.graph.edges.len(), 3);
}

#[test]
fn personalized_pagerank_descriptor_supported_modes() {
    let d = PersonalizedPageRankDescriptor;
    let modes = d.supported_modes();
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Stream));
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Stats));
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Annotate));
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Persist));
}

// =============================================================================
// Conductance descriptor tests
// =============================================================================

#[test]
fn conductance_descriptor_has_correct_identity() {
    let d = ConductanceDescriptor;
    let id = d.identity();
    assert_eq!(id.id.as_str(), "conductance");
    assert_eq!(id.version.as_str(), "1.0.0");
    assert_eq!(
        id.maturity,
        cognicode_core::domain::analytics::Maturity::Experimental
    );
    assert_eq!(id.cohort, 3);
}

#[test]
fn conductance_descriptor_params_accepted() {
    let d = ConductanceDescriptor;
    let params = serde_json::json!({
        "community_assignment": [["A", 0], ["B", 0], ["C", 1]]
    });
    assert!(d.params().validate(&params).is_ok());
}

#[test]
fn conductance_descriptor_rejects_missing_community_assignment() {
    let d = ConductanceDescriptor;
    let params = serde_json::json!({});
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn conductance_descriptor_rejects_empty_community_assignment() {
    let d = ConductanceDescriptor;
    let params = serde_json::json!({ "community_assignment": [] });
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn conductance_descriptor_rejects_non_array_community_assignment() {
    let d = ConductanceDescriptor;
    let params = serde_json::json!({ "community_assignment": "not-an-array" });
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn conductance_descriptor_rejects_malformed_pair() {
    let d = ConductanceDescriptor;
    // Pair must be [node_id, community_id] — this has 3 elements
    let params = serde_json::json!({
        "community_assignment": [["A", 0, "extra"]]
    });
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn conductance_descriptor_output_schema_has_two_fields() {
    let d = ConductanceDescriptor;
    let schema = d.output_schema();
    assert_eq!(schema.fields.len(), 2);
    assert_eq!(schema.fields[0].name, "community_id");
    assert_eq!(schema.fields[1].name, "conductance");
}

#[test]
fn conductance_descriptor_is_directed() {
    let d = ConductanceDescriptor;
    assert!(d.directed());
    assert!(!d.weighted());
    assert!(!d.heterogeneous());
}

#[test]
fn conductance_descriptor_has_conformance_fixtures() {
    let d = ConductanceDescriptor;
    let fixtures = d.conformance_fixtures();
    assert!(!fixtures.is_empty());
    // Verify triangle fixture
    let triangle = fixtures
        .iter()
        .find(|f| f.name.contains("triangle"))
        .unwrap();
    assert_eq!(triangle.graph.nodes.len(), 3);
}

#[test]
fn conductance_descriptor_supported_modes() {
    let d = ConductanceDescriptor;
    let modes = d.supported_modes();
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Stream));
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Stats));
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Annotate));
    // Conductance does NOT support Persist
    assert!(!modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Persist));
}

// =============================================================================
// Modularity descriptor tests
// =============================================================================

#[test]
fn modularity_descriptor_has_correct_identity() {
    let d = ModularityDescriptor;
    let id = d.identity();
    assert_eq!(id.id.as_str(), "modularity");
    assert_eq!(id.version.as_str(), "1.0.0");
    assert_eq!(
        id.maturity,
        cognicode_core::domain::analytics::Maturity::Experimental
    );
    assert_eq!(id.cohort, 3);
}

#[test]
fn modularity_descriptor_params_accepted() {
    let d = ModularityDescriptor;
    let params = serde_json::json!({
        "community_assignment": [["A", 0], ["B", 0], ["C", 1]]
    });
    assert!(d.params().validate(&params).is_ok());
}

#[test]
fn modularity_descriptor_rejects_missing_community_assignment() {
    let d = ModularityDescriptor;
    let params = serde_json::json!({});
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn modularity_descriptor_rejects_empty_community_assignment() {
    let d = ModularityDescriptor;
    let params = serde_json::json!({ "community_assignment": [] });
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn modularity_descriptor_rejects_non_array_community_assignment() {
    let d = ModularityDescriptor;
    let params = serde_json::json!({ "community_assignment": 42 });
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn modularity_descriptor_rejects_non_pair_elements() {
    let d = ModularityDescriptor;
    // Pair must be [node_id, community_id] — this has wrong length
    let params = serde_json::json!({
        "community_assignment": [["A", "B", "C"]]
    });
    assert!(d.params().validate(&params).is_err());
}

#[test]
fn modularity_descriptor_output_schema_has_two_fields() {
    let d = ModularityDescriptor;
    let schema = d.output_schema();
    assert_eq!(schema.fields.len(), 2);
    assert_eq!(schema.fields[0].name, "score");
    assert_eq!(schema.fields[1].name, "community_count");
}

#[test]
fn modularity_descriptor_is_directed() {
    let d = ModularityDescriptor;
    assert!(d.directed());
    assert!(!d.weighted());
    assert!(!d.heterogeneous());
}

#[test]
fn modularity_descriptor_has_conformance_fixtures() {
    let d = ModularityDescriptor;
    let fixtures = d.conformance_fixtures();
    assert!(!fixtures.is_empty());
    // Verify two-nodes fixture
    let pair = fixtures
        .iter()
        .find(|f| f.name.contains("two nodes"))
        .unwrap();
    assert_eq!(pair.graph.nodes, vec!["A", "B"]);
    assert_eq!(pair.graph.edges.len(), 1);
}

#[test]
fn modularity_descriptor_supported_modes() {
    let d = ModularityDescriptor;
    let modes = d.supported_modes();
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Stream));
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Stats));
    assert!(modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Annotate));
    // Modularity does NOT support Persist
    assert!(!modes.contains(&cognicode_core::domain::analytics::AnalyticsMode::Persist));
}

// =============================================================================
// Registry integration tests
// =============================================================================

#[test]
fn registry_admits_all_three_cohort3_algorithms() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);

    let r1 = registry.admit(Box::new(PersonalizedPageRankDescriptor));
    assert!(
        r1.is_ok(),
        "PersonalizedPageRank should be admitted: {:?}",
        r1
    );

    let r2 = registry.admit(Box::new(ConductanceDescriptor));
    assert!(r2.is_ok(), "Conductance should be admitted: {:?}", r2);

    let r3 = registry.admit(Box::new(ModularityDescriptor));
    assert!(r3.is_ok(), "Modularity should be admitted: {:?}", r3);
}

#[test]
fn registry_detects_all_three_cohort3_algorithms() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    registry
        .admit(Box::new(PersonalizedPageRankDescriptor))
        .unwrap();
    registry.admit(Box::new(ConductanceDescriptor)).unwrap();
    registry.admit(Box::new(ModularityDescriptor)).unwrap();

    let ppr_id = AlgorithmId::from_static("personalized_pagerank");
    let cond_id = AlgorithmId::from_static("conductance");
    let mod_id = AlgorithmId::from_static("modularity");

    assert!(registry.is_admitted(&ppr_id));
    assert!(registry.is_admitted(&cond_id));
    assert!(registry.is_admitted(&mod_id));
}

#[test]
fn registry_get_returns_cohort3_descriptors() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    registry
        .admit(Box::new(PersonalizedPageRankDescriptor))
        .unwrap();
    registry.admit(Box::new(ConductanceDescriptor)).unwrap();
    registry.admit(Box::new(ModularityDescriptor)).unwrap();

    let ppr_id = AlgorithmId::from_static("personalized_pagerank");
    let cond_id = AlgorithmId::from_static("conductance");
    let mod_id = AlgorithmId::from_static("modularity");

    let ppr = registry.get(&ppr_id);
    assert!(ppr.is_some());
    assert_eq!(ppr.unwrap().identity().id.as_str(), "personalized_pagerank");

    let cond = registry.get(&cond_id);
    assert!(cond.is_some());
    assert_eq!(cond.unwrap().identity().id.as_str(), "conductance");

    let m = registry.get(&mod_id);
    assert!(m.is_some());
    assert_eq!(m.unwrap().identity().id.as_str(), "modularity");
}

#[test]
fn registry_rejects_duplicate_cohort3_admission() {
    let mut registry = AlgorithmRegistry::new(Arc::new(NoOpLineageStore), None);
    registry
        .admit(Box::new(PersonalizedPageRankDescriptor))
        .unwrap();

    let result = registry.admit(Box::new(PersonalizedPageRankDescriptor));
    assert!(matches!(result, Err(AdmissionError::AlreadyAdmitted(_, _))));
}
