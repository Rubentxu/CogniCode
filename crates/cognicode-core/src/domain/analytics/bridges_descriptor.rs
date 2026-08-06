//! Bridges descriptor for the analytics registry.
//!
//! Part of E28.5 Structural Analytics Cohort 2 — PR2 Descriptors.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(unused_imports)]

use std::sync::LazyLock;

use crate::domain::aggregates::CallGraph;
use crate::domain::analytics::{
    AlgorithmDescriptor, AlgorithmExecute, AlgorithmId, AlgorithmIdentity, AlgorithmParams,
    AlgorithmVersion, AnalyticsError, AnalyticsMode, ComplexityClass, DeterminismKind, Fixture,
    FixtureGraph, Maturity, OutputField, OutputSchema, OutputType, ProjectionAssumption, RunOutput,
};
use crate::domain::plan::limits::PlanLimits;
use crate::domain::ports::call_graph_projection::{CallGraphProjectionPort, project_call_graph};
use cognicode_graph_algos::GraphBuilder;

// =============================================================================
// Bridges output schema
// =============================================================================

static BRIDGES_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![OutputField {
        name: "edges",
        type_: OutputType::Json,
    }],
});

// =============================================================================
// Bridges complexity
// =============================================================================

static BRIDGES_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E)",
    space: "O(V)",
    notes: "Tarjan's algorithm, single DFS pass",
});

// =============================================================================
// Bridges conformance fixtures
// =============================================================================

static BRIDGES_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        // Path A-B-C: all edges are bridges
        Fixture {
            name: "path all bridges",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C")],
            },
            expected: serde_json::json!({
                "type": "path",
                "expectation": "all_edges_are_bridges"
            }),
        },
        // Cycle A-B-C-A: no bridges
        Fixture {
            name: "cycle no bridges",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C"), ("C", "A")],
            },
            expected: serde_json::json!({
                "type": "cycle",
                "expectation": "no_bridges"
            }),
        },
        // Diamond (A-B, A-C, B-D, C-D): no bridges
        Fixture {
            name: "diamond no bridges",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C", "D"],
                edges: vec![("A", "B"), ("A", "C"), ("B", "D"), ("C", "D")],
            },
            expected: serde_json::json!({
                "type": "diamond",
                "expectation": "no_bridges"
            }),
        },
        // Empty graph
        Fixture {
            name: "empty graph",
            graph: FixtureGraph {
                nodes: vec![],
                edges: vec![],
            },
            expected: serde_json::json!({
                "type": "empty",
                "expectation": "empty_result"
            }),
        },
    ]
});

// =============================================================================
// Bridges identity
// =============================================================================

static BRIDGES_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("bridges"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Stable,
    cohort: 2,
});

// =============================================================================
// Bridges params (no params)
// =============================================================================

pub struct BridgesParams;

impl AlgorithmParams for BridgesParams {
    fn param_names(&self) -> Vec<&'static str> {
        vec![]
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        if params.is_null() || params.as_object().is_some_and(|o| o.is_empty()) {
            Ok(())
        } else {
            Err("Bridges algorithm accepts no parameters".into())
        }
    }
}

// =============================================================================
// Bridges descriptor
// =============================================================================

/// Bridges descriptor using Tarjan's algorithm.
///
/// Wraps `cognicode_graph_algos::bridges`:
/// - Deterministic: sorted lexicographically by edge pair
/// - Directed: NO (treats graph as undirected for connectivity)
/// - Weighted: no
/// - Heterogeneous: no
pub struct BridgesDescriptor;

impl_cohort2_descriptor!(
    BridgesDescriptor,
    false,                            // directed
    &BRIDGES_IDENTITY,                // identity
    &BridgesParams,                   // params
    &BRIDGES_SCHEMA,                  // output_schema
    &BRIDGES_FIXTURES,                // conformance_fixtures
    &BRIDGES_COMPLEXITY,              // complexity
    ProjectionAssumption::Undirected  // projection_assumption
);

#[async_trait::async_trait]
impl AlgorithmExecute for BridgesDescriptor {
    async fn execute(
        &self,
        _params: &serde_json::Value,
        graph: &CallGraph,
        limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        let projection: std::sync::Arc<dyn CallGraphProjectionPort> = project_call_graph(graph);
        let undirected = projection.build_undirected_neighbors();
        let n = projection.node_count();

        let raw = cognicode_graph_algos::bridges(&undirected, n);

        // Enforce max_result_rows limit
        let max_rows = limits.max_result_rows.unwrap_or(100_000) as usize;
        let edges = raw.into_iter().take(max_rows).collect();

        Ok(RunOutput::Bridges { edges })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridges_descriptor_has_correct_identity() {
        let d = BridgesDescriptor;
        let id = d.identity();
        assert_eq!(id.id.as_str(), "bridges");
        assert_eq!(id.version.as_str(), "1.0.0");
        assert_eq!(id.maturity, Maturity::Stable);
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
    fn bridges_descriptor_is_undirected() {
        let d = BridgesDescriptor;
        assert!(!d.directed()); // undirected
        assert!(!d.weighted());
        assert!(!d.heterogeneous());
    }
}
