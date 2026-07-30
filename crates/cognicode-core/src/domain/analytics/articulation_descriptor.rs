//! Articulation Points descriptor for the analytics registry.
//!
//! Part of E28.5 Structural Analytics Cohort 2 — PR2 Descriptors.

use std::sync::LazyLock;

use crate::domain::aggregates::CallGraph;
use crate::domain::analytics::{
    AlgorithmDescriptor, AlgorithmExecute, AlgorithmIdentity, AlgorithmId, AlgorithmParams,
    AlgorithmVersion, AnalyticsError, AnalyticsMode, ComplexityClass, DeterminismKind,
    Fixture, FixtureGraph, Maturity, OutputField, OutputSchema, OutputType,
    ProjectionAssumption, RunOutput,
};
use crate::domain::plan::limits::PlanLimits;
use crate::infrastructure::graph::CallGraphProjection;
use cognicode_graph_algos::GraphBuilder;

// =============================================================================
// Articulation Points output schema
// =============================================================================

static ARTICULATION_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField { name: "nodes", type_: OutputType::NodeId },
        OutputField { name: "cut_vertices_counts", type_: OutputType::Json },
    ],
});

// =============================================================================
// Articulation Points limits
// =============================================================================

static ARTICULATION_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
    time_ms: Some(30000),
    cancellation: None,
    max_depth: None,
    max_hops: None,
    max_visited_nodes: Some(1_000_000),
    max_visited_edges: None,
    max_result_rows: Some(100_000),
    max_path_count: None,
    max_memory_bytes: Some(512 * 1024 * 1024),
});

// =============================================================================
// Articulation Points complexity
// =============================================================================

static ARTICULATION_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E)".into(),
    space: "O(V)".into(),
    notes: "Tarjan's algorithm, single DFS pass with component counting".into(),
});

// =============================================================================
// Articulation Points conformance fixtures
// =============================================================================

static ARTICULATION_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        // Path A-B-C: B is the articulation point
        Fixture {
            name: "path articulation",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C")],
            },
            expected: serde_json::json!({
                "type": "path",
                "expectation": "b_is_articulation"
            }),
        },
        // Cycle A-B-C-A: no articulation points
        Fixture {
            name: "cycle no articulation",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C"), ("C", "A")],
            },
            expected: serde_json::json!({
                "type": "cycle",
                "expectation": "no_articulation"
            }),
        },
        // Star: center A connected to B, C, D. A is articulation point.
        Fixture {
            name: "star articulation",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C", "D"],
                edges: vec![("A", "B"), ("A", "C"), ("A", "D")],
            },
            expected: serde_json::json!({
                "type": "star",
                "expectation": "center_is_articulation"
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
// Articulation Points identity
// =============================================================================

static ARTICULATION_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("articulation_points"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Stable,
    cohort: 2,
});

// =============================================================================
// Articulation Points params (no params)
// =============================================================================

pub struct ArticulationPointsParams;

impl AlgorithmParams for ArticulationPointsParams {
    fn param_names(&self) -> Vec<&'static str> {
        vec![]
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        if params.is_null() || params.as_object().map_or(false, |o| o.is_empty()) {
            Ok(())
        } else {
            Err("ArticulationPoints algorithm accepts no parameters".into())
        }
    }
}

// =============================================================================
// Articulation Points descriptor
// =============================================================================

/// Articulation Points descriptor using Tarjan's algorithm.
///
/// Wraps `cognicode_graph_algos::articulation_points`:
/// - Deterministic: sorted by node_id
/// - Directed: NO (treats graph as undirected for connectivity)
/// - Weighted: no
/// - Heterogeneous: no
pub struct ArticulationPointsDescriptor;

impl AlgorithmDescriptor for ArticulationPointsDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &ARTICULATION_IDENTITY
    }

    fn params(&self) -> &dyn AlgorithmParams {
        &ArticulationPointsParams
    }

    fn output_schema(&self) -> &OutputSchema {
        &ARTICULATION_SCHEMA
    }

    fn supported_modes(&self) -> &[AnalyticsMode] {
        &[
            AnalyticsMode::Stream,
            AnalyticsMode::Stats,
            AnalyticsMode::Annotate,
        ]
    }

    fn complexity(&self) -> &ComplexityClass {
        &ARTICULATION_COMPLEXITY
    }

    fn limits(&self) -> &PlanLimits {
        &ARTICULATION_LIMITS
    }

    fn conformance_fixtures(&self) -> &[Fixture] {
        &ARTICULATION_FIXTURES
    }

    fn determinism(&self) -> DeterminismKind {
        DeterminismKind::Deterministic
    }

    fn directed(&self) -> bool {
        false
    }

    fn weighted(&self) -> bool {
        false
    }

    fn heterogeneous(&self) -> bool {
        false
    }

    fn projection_assumption(&self) -> &ProjectionAssumption {
        &ProjectionAssumption::Undirected
    }
}

#[async_trait::async_trait]
impl AlgorithmExecute for ArticulationPointsDescriptor {
    async fn execute(
        &self,
        _params: &serde_json::Value,
        graph: &CallGraph,
        _limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        let projection = CallGraphProjection::from_call_graph(graph);
        let (in_neighbors, _) = projection.build_adjacency();
        let n = projection.node_count();

        // Build undirected adjacency: union of in and out neighbors
        let mut undirected: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (i, neighbors) in in_neighbors.iter().enumerate() {
            for &j in neighbors {
                if i < n && j < n {
                    undirected[i].push(j);
                }
            }
        }
        // Also add out neighbors to make it undirected
        let out_neighbors = projection.build_out_neighbors();
        for (i, neighbors) in out_neighbors.iter().enumerate() {
            for &j in neighbors {
                if i < n && j < n && !undirected[i].contains(&j) {
                    undirected[i].push(j);
                }
            }
        }

        let raw = cognicode_graph_algos::articulation_points(&undirected, n);

        // Unpack into two parallel vectors
        let nodes: Vec<usize> = raw.iter().map(|(v, _)| *v).collect();
        let cut_vertices_counts: Vec<usize> = raw.iter().map(|(_, c)| *c).collect();

        Ok(RunOutput::ArticulationPoints {
            nodes,
            cut_vertices_counts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn articulation_descriptor_has_correct_identity() {
        let d = ArticulationPointsDescriptor;
        let id = d.identity();
        assert_eq!(id.id.as_str(), "articulation_points");
        assert_eq!(id.version.as_str(), "1.0.0");
        assert_eq!(id.maturity, Maturity::Stable);
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
    fn articulation_descriptor_is_undirected() {
        let d = ArticulationPointsDescriptor;
        assert!(!d.directed()); // undirected
        assert!(!d.weighted());
        assert!(!d.heterogeneous());
    }
}
