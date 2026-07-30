//! Conductance descriptor for the analytics registry.
//!
//! Part of E28.6 Advanced Analytics Evidence Gate — PR2.

use std::sync::LazyLock;

use crate::domain::aggregates::CallGraph;
use crate::domain::analytics::{
    AlgorithmDescriptor, AlgorithmExecute, AlgorithmId, AlgorithmIdentity, AlgorithmParams,
    AlgorithmVersion, AnalyticsError, AnalyticsMode, ComplexityClass, DeterminismKind, Fixture,
    FixtureGraph, Maturity, OutputField, OutputSchema, OutputType, ProjectionAssumption, RunOutput,
};
use crate::domain::plan::limits::PlanLimits;
use crate::infrastructure::graph::CallGraphProjection;
use cognicode_graph_algos::GraphBuilder;

// =============================================================================
// Conductance Params
// =============================================================================

static CONDUCTANCE_PARAM_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["community_assignment"]);

pub struct ConductanceParams;

impl AlgorithmParams for ConductanceParams {
    fn param_names(&self) -> Vec<&'static str> {
        CONDUCTANCE_PARAM_NAMES.to_vec()
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        if let Some(obj) = params.as_object() {
            if !obj.contains_key("community_assignment") {
                return Err("missing required parameter: community_assignment".into());
            }
            let assignment = obj.get("community_assignment");
            if assignment.is_none() {
                return Err("community_assignment must be provided".into());
            }
            // Must be an array of [node_id, community_id] pairs
            if let Some(arr) = assignment.and_then(|v| v.as_array()) {
                if arr.is_empty() {
                    return Err("community_assignment must not be empty".into());
                }
                for item in arr {
                    if !item.is_array() || item.as_array().unwrap().len() != 2 {
                        return Err("community_assignment must be [[node_id, community_id], ...]".into());
                    }
                }
            } else {
                return Err("community_assignment must be an array".into());
            }
            Ok(())
        } else {
            Err("params must be a JSON object".into())
        }
    }
}

// =============================================================================
// Conductance output schema
// =============================================================================

static CONDUCTANCE_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField {
            name: "community_id",
            type_: OutputType::Count,
        },
        OutputField {
            name: "conductance",
            type_: OutputType::Score,
        },
    ],
});

// =============================================================================
// Conductance limits
// =============================================================================

static CONDUCTANCE_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
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
// Conductance complexity
// =============================================================================

static CONDUCTANCE_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E)".into(),
    space: "O(V)".into(),
    notes: "Linear scan of edges per community".into(),
});

// =============================================================================
// Conductance conformance fixtures
// =============================================================================

static CONDUCTANCE_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        // Triangle: single community, no cut edges → conductance = 1.0
        Fixture {
            name: "triangle single community",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C"), ("C", "A")],
            },
            expected: serde_json::json!({
                "type": "triangle",
                "expectation": "single_community_conductance_1"
            }),
        },
        // Two communities with crossing edge
        Fixture {
            name: "two communities with cut",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("A", "C")],
            },
            expected: serde_json::json!({
                "type": "two_communities",
                "expectation": "cut_edges_present"
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
// Conductance identity
// =============================================================================

static CONDUCTANCE_IDENTITY: LazyLock<AlgorithmIdentity> =
    LazyLock::new(|| AlgorithmIdentity {
        id: AlgorithmId::from_static("conductance"),
        version: AlgorithmVersion::v1(),
        maturity: Maturity::Experimental,
        cohort: 3,
    });

// =============================================================================
// Conductance descriptor
// =============================================================================

/// Conductance descriptor for the analytics registry.
///
/// Computes the conductance score for each community in a given partition.
///
/// Param: `community_assignment` — array of [node_id, community_id] pairs.
///
/// - Modes: Stream, Stats, Annotate
/// - Directed: yes (uses directed adjacency)
/// - Weighted: no
/// - Determinism: Deterministic
/// - Maturity: Experimental (cohort 3)
pub struct ConductanceDescriptor;

impl AlgorithmDescriptor for ConductanceDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &CONDUCTANCE_IDENTITY
    }

    fn params(&self) -> &dyn AlgorithmParams {
        &ConductanceParams
    }

    fn output_schema(&self) -> &OutputSchema {
        &CONDUCTANCE_SCHEMA
    }

    fn supported_modes(&self) -> &[AnalyticsMode] {
        &[
            AnalyticsMode::Stream,
            AnalyticsMode::Stats,
            AnalyticsMode::Annotate,
        ]
    }

    fn complexity(&self) -> &ComplexityClass {
        &CONDUCTANCE_COMPLEXITY
    }

    fn limits(&self) -> &PlanLimits {
        &CONDUCTANCE_LIMITS
    }

    fn conformance_fixtures(&self) -> &[Fixture] {
        &CONDUCTANCE_FIXTURES
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
        &ProjectionAssumption::CallGraphOutgoing
    }
}

#[async_trait::async_trait]
impl AlgorithmExecute for ConductanceDescriptor {
    async fn execute(
        &self,
        params: &serde_json::Value,
        graph: &CallGraph,
        limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        let obj = params.as_object().ok_or_else(|| {
            AnalyticsError::Internal("Conductance params must be a JSON object".into())
        })?;

        let assignment_arr = obj
            .get("community_assignment")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                AnalyticsError::Internal("missing community_assignment".into())
            })?;

        let projection = CallGraphProjection::from_call_graph(graph);
        let out_neighbors = projection.build_directed_adjacency();
        let n = projection.node_count();

        // Parse community_assignment from params
        // Format: [[node_id, community_id], ...]
        let mut community_assignment: Vec<(usize, usize)> = Vec::new();
        for item in assignment_arr {
            let arr = item.as_array().unwrap();
            let node = arr[0].as_u64().ok_or_else(|| {
                AnalyticsError::InvalidParameter("node_id must be a non-negative integer".into())
            })? as usize;
            let community = arr[1].as_u64().ok_or_else(|| {
                AnalyticsError::InvalidParameter("community_id must be a non-negative integer".into())
            })? as usize;
            community_assignment.push((node, community));
        }

        // Enforce max_result_rows limit
        let max_rows = limits.max_result_rows.unwrap_or(100_000) as usize;

        let raw = cognicode_graph_algos::conductance(&community_assignment, &out_neighbors);

        // Truncate to max_result_rows
        let mut community_ids: Vec<usize> = raw.iter().map(|&(c, _)| c).collect();
        let mut scores: Vec<f64> = raw.iter().map(|&(_, s)| s).collect();

        community_ids.truncate(max_rows);
        scores.truncate(max_rows);

        Ok(RunOutput::Conductance {
            community_ids,
            scores,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conductance_descriptor_has_correct_identity() {
        let d = ConductanceDescriptor;
        let id = d.identity();
        assert_eq!(id.id.as_str(), "conductance");
        assert_eq!(id.version.as_str(), "1.0.0");
        assert_eq!(id.maturity, Maturity::Experimental);
        assert_eq!(id.cohort, 3);
    }

    #[test]
    fn conductance_descriptor_params_accepted() {
        let d = ConductanceDescriptor;
        let params = serde_json::json!({
            "community_assignment": [[0, 1], [1, 1], [2, 2]]
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
        let params = serde_json::json!({
            "community_assignment": []
        });
        assert!(d.params().validate(&params).is_err());
    }

    #[test]
    fn conductance_descriptor_rejects_non_array_community_assignment() {
        let d = ConductanceDescriptor;
        let params = serde_json::json!({
            "community_assignment": "not an array"
        });
        assert!(d.params().validate(&params).is_err());
    }

    #[test]
    fn conductance_descriptor_rejects_malformed_pair() {
        let d = ConductanceDescriptor;
        let params = serde_json::json!({
            "community_assignment": [[0]]
        });
        assert!(d.params().validate(&params).is_err());
    }
}
