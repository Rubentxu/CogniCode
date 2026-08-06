//! Modularity descriptor for the analytics registry.
//!
//! Part of E28.6 Advanced Analytics Evidence Gate — PR2.
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
// Modularity Params
// =============================================================================

static MODULARITY_PARAM_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["community_assignment"]);

pub struct ModularityParams;

impl AlgorithmParams for ModularityParams {
    fn param_names(&self) -> Vec<&'static str> {
        MODULARITY_PARAM_NAMES.to_vec()
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
                        return Err(
                            "community_assignment must be [[node_id, community_id], ...]".into(),
                        );
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
// Modularity output schema
// =============================================================================

static MODULARITY_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField {
            name: "score",
            type_: OutputType::Score,
        },
        OutputField {
            name: "community_count",
            type_: OutputType::Count,
        },
    ],
});

// =============================================================================
// Modularity limits
// =============================================================================

static MODULARITY_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
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
// Modularity complexity
// =============================================================================

static MODULARITY_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E)",
    space: "O(V)",
    notes: "Linear scan of edges per community",
});

// =============================================================================
// Modularity conformance fixtures
// =============================================================================

static MODULARITY_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        // Two nodes connected: community {0,1} → Q = 0.5
        Fixture {
            name: "two nodes single community",
            graph: FixtureGraph {
                nodes: vec!["A", "B"],
                edges: vec![("A", "B")],
            },
            expected: serde_json::json!({
                "type": "pair",
                "expectation": "modularity_0.5"
            }),
        },
        // Triangle: single community, m=3, each node degree 2
        // Q = (1/6) * [3 * (1 - 2*2/6)] = (1/6) * [3 * (1 - 4/6)] = (1/6) * [3 * 2/6] = (1/6) * 1 = 1/6 ≈ 0.167
        Fixture {
            name: "triangle single community",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C"), ("C", "A")],
            },
            expected: serde_json::json!({
                "type": "triangle",
                "expectation": "positive_modularity"
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
                "expectation": "zero_modularity"
            }),
        },
    ]
});

// =============================================================================
// Modularity identity
// =============================================================================

static MODULARITY_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("modularity"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Experimental,
    cohort: 3,
});

// =============================================================================
// Modularity descriptor
// =============================================================================

/// Modularity descriptor for the analytics registry.
///
/// Computes the modularity score for a given community partition.
///
/// Param: `community_assignment` — array of [node_id, community_id] pairs.
///
/// - Modes: Stream, Stats, Annotate
/// - Directed: yes (uses directed adjacency)
/// - Weighted: no
/// - Determinism: Deterministic
/// - Maturity: Experimental (cohort 3)
pub struct ModularityDescriptor;

impl AlgorithmDescriptor for ModularityDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &MODULARITY_IDENTITY
    }

    fn params(&self) -> &dyn AlgorithmParams {
        &ModularityParams
    }

    fn output_schema(&self) -> &OutputSchema {
        &MODULARITY_SCHEMA
    }

    fn supported_modes(&self) -> &[AnalyticsMode] {
        &[
            AnalyticsMode::Stream,
            AnalyticsMode::Stats,
            AnalyticsMode::Annotate,
        ]
    }

    fn complexity(&self) -> &ComplexityClass {
        &MODULARITY_COMPLEXITY
    }

    fn limits(&self) -> &PlanLimits {
        &MODULARITY_LIMITS
    }

    fn conformance_fixtures(&self) -> &[Fixture] {
        &MODULARITY_FIXTURES
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
impl AlgorithmExecute for ModularityDescriptor {
    async fn execute(
        &self,
        params: &serde_json::Value,
        graph: &CallGraph,
        _limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        let obj = params.as_object().ok_or_else(|| {
            AnalyticsError::Internal("Modularity params must be a JSON object".into())
        })?;

        let assignment_arr = obj
            .get("community_assignment")
            .and_then(|v| v.as_array())
            .ok_or_else(|| AnalyticsError::Internal("missing community_assignment".into()))?;

        let projection: std::sync::Arc<dyn CallGraphProjectionPort> = project_call_graph(graph);
        let out_neighbors = projection.build_directed_adjacency();

        // Parse community_assignment from params
        // Format: [[node_id, community_id], ...]
        let mut community_assignment: Vec<(usize, usize)> = Vec::new();
        for item in assignment_arr {
            let arr = item.as_array().unwrap();
            let node = arr[0].as_u64().ok_or_else(|| {
                AnalyticsError::InvalidParameter("node_id must be a non-negative integer".into())
            })? as usize;
            let community = arr[1].as_u64().ok_or_else(|| {
                AnalyticsError::InvalidParameter(
                    "community_id must be a non-negative integer".into(),
                )
            })? as usize;
            community_assignment.push((node, community));
        }

        let (score, community_count) =
            cognicode_graph_algos::modularity(&community_assignment, &out_neighbors);

        Ok(RunOutput::Modularity {
            score,
            community_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modularity_descriptor_has_correct_identity() {
        let d = ModularityDescriptor;
        let id = d.identity();
        assert_eq!(id.id.as_str(), "modularity");
        assert_eq!(id.version.as_str(), "1.0.0");
        assert_eq!(id.maturity, Maturity::Experimental);
        assert_eq!(id.cohort, 3);
    }

    #[test]
    fn modularity_descriptor_params_accepted() {
        let d = ModularityDescriptor;
        let params = serde_json::json!({
            "community_assignment": [[0, 1], [1, 1], [2, 2]]
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
        let params = serde_json::json!({
            "community_assignment": []
        });
        assert!(d.params().validate(&params).is_err());
    }

    #[test]
    fn modularity_descriptor_rejects_non_array_community_assignment() {
        let d = ModularityDescriptor;
        let params = serde_json::json!({
            "community_assignment": "not an array"
        });
        assert!(d.params().validate(&params).is_err());
    }
}
