//! Personalized PageRank descriptor for the analytics registry.
//!
//! Part of E28.6 Advanced Analytics Evidence Gate — PR1.

use std::collections::HashMap;
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
// Personalized PageRank Params
// =============================================================================

static PERSONALIZED_PAGERANK_PARAM_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["alpha", "max_iterations", "personalization_vector"]);

pub struct PersonalizedParams;

impl AlgorithmParams for PersonalizedParams {
    fn param_names(&self) -> Vec<&'static str> {
        PERSONALIZED_PAGERANK_PARAM_NAMES.to_vec()
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        if let Some(obj) = params.as_object() {
            if !obj.contains_key("alpha") {
                return Err("missing parameter: alpha".into());
            }
            if !obj.contains_key("max_iterations") {
                return Err("missing parameter: max_iterations".into());
            }
            // personalization_vector is optional (None = standard PageRank)
            Ok(())
        } else {
            Err("params must be a JSON object".into())
        }
    }
}

// =============================================================================
// Personalized PageRank output schema
// =============================================================================

static PERSONALIZED_PAGERANK_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField {
            name: "node_id",
            type_: OutputType::NodeId,
        },
        OutputField {
            name: "score",
            type_: OutputType::Score,
        },
    ],
});

// =============================================================================
// Personalized PageRank limits
// =============================================================================

static PERSONALIZED_PAGERANK_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
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
// Personalized PageRank complexity
// =============================================================================

static PERSONALIZED_PAGERANK_COMPLEXITY: LazyLock<ComplexityClass> =
    LazyLock::new(|| ComplexityClass {
        time: "O(V + E)".into(),
        space: "O(V)".into(),
        notes: "per iteration; same as PageRank, personalization vector adds O(V)".into(),
    });

// =============================================================================
// Personalized PageRank conformance fixtures
// =============================================================================

static PERSONALIZED_PAGERANK_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        // 3-node cycle: A→B→C→A — personalize toward A
        Fixture {
            name: "3-node cycle personalize A",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C"), ("C", "A")],
            },
            expected: serde_json::json!({
                "type": "uniform_cycle",
                "expectation": "personalized_A_highest"
            }),
        },
        // Star: center=0, leaves=1..5. Personalize toward leaf 1.
        Fixture {
            name: "star personalize leaf1",
            graph: FixtureGraph {
                nodes: vec!["center", "leaf1", "leaf2", "leaf3", "leaf4", "leaf5"],
                edges: vec![
                    ("leaf1", "center"),
                    ("leaf2", "center"),
                    ("leaf3", "center"),
                    ("leaf4", "center"),
                    ("leaf5", "center"),
                ],
            },
            expected: serde_json::json!({
                "type": "star",
                "expectation": "personalized_leaf1_highest"
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
                "expectation": "empty_scores"
            }),
        },
    ]
});

// =============================================================================
// Personalized PageRank identity
// =============================================================================

static PERSONALIZED_PAGERANK_IDENTITY: LazyLock<AlgorithmIdentity> =
    LazyLock::new(|| AlgorithmIdentity {
        id: AlgorithmId::from_static("personalized_pagerank"),
        version: AlgorithmVersion::v1(),
        maturity: Maturity::Experimental,
        cohort: 3,
    });

// =============================================================================
// Personalized PageRank descriptor
// =============================================================================

/// Personalized PageRank descriptor for the analytics registry.
///
/// Wraps `cognicode_graph_algos::personalized_pagerank` with the following contract:
/// - `alpha = 0.85` (damping factor)
/// - `max_iterations = 100`
/// - `personalization_vector` (optional): list of node IDs to bias toward.
///   When absent or empty, falls back to standard PageRank.
/// - Directed: yes (call graph is directed)
/// - Weighted: no
/// - Determinism: Deterministic
/// - Maturity: Experimental (cohort 3)
pub struct PersonalizedPageRankDescriptor;

impl AlgorithmDescriptor for PersonalizedPageRankDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &PERSONALIZED_PAGERANK_IDENTITY
    }

    fn params(&self) -> &dyn AlgorithmParams {
        &PersonalizedParams
    }

    fn output_schema(&self) -> &OutputSchema {
        &PERSONALIZED_PAGERANK_SCHEMA
    }

    fn supported_modes(&self) -> &[AnalyticsMode] {
        &[
            AnalyticsMode::Stream,
            AnalyticsMode::Stats,
            AnalyticsMode::Annotate,
            AnalyticsMode::Persist,
        ]
    }

    fn complexity(&self) -> &ComplexityClass {
        &PERSONALIZED_PAGERANK_COMPLEXITY
    }

    fn limits(&self) -> &PlanLimits {
        &PERSONALIZED_PAGERANK_LIMITS
    }

    fn conformance_fixtures(&self) -> &[Fixture] {
        &PERSONALIZED_PAGERANK_FIXTURES
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

#[async_trait::async_trait]
impl AlgorithmExecute for PersonalizedPageRankDescriptor {
    async fn execute(
        &self,
        params: &serde_json::Value,
        graph: &CallGraph,
        _limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        let obj = params.as_object().ok_or_else(|| {
            AnalyticsError::Internal("PersonalizedPageRank params must be a JSON object".into())
        })?;

        let alpha = obj.get("alpha").and_then(|v| v.as_f64()).unwrap_or(0.85);
        let max_iterations = obj
            .get("max_iterations")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(100);

        let projection: std::sync::Arc<dyn CallGraphProjectionPort> = project_call_graph(graph);
        let (in_neighbors, out_degree) = projection.build_adjacency();
        let n = projection.node_count();

        // Build personalization vector from personalization_vector param
        let personalization: Option<Vec<f64>> = obj
            .get("personalization_vector")
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                // Count how many times each node index appears in the array
                let mut counts = vec![0usize; n];
                let id_to_idx = projection.symbol_index();
                for item in arr {
                    // NodeId could be a string (symbol ID) or number (index)
                    if let Some(sid_str) = item.as_str() {
                        let sid = crate::domain::aggregates::SymbolId::new(sid_str);
                        // Map symbol ID to node index
                        if let Some(&ni) = id_to_idx.get(&sid) {
                            let idx = ni.index();
                            if idx < n {
                                counts[idx] += 1;
                            }
                        }
                    } else if let Some(idx) = item.as_u64() {
                        let idx = idx as usize;
                        if idx < n {
                            counts[idx] += 1;
                        }
                    }
                }
                // Convert to probability distribution
                let total: usize = counts.iter().sum();
                if total > 0 {
                    Some(counts.iter().map(|&c| c as f64 / total as f64).collect())
                } else {
                    None
                }
            });

        let raw_scores = cognicode_graph_algos::personalized_pagerank(
            &in_neighbors,
            &out_degree,
            n,
            alpha,
            max_iterations,
            personalization.as_deref(),
        );

        // Map back to SymbolId and serialize
        let mut scores: HashMap<String, f64> = HashMap::new();
        for (sid, ni) in projection.symbol_index() {
            if let Some(&score) = raw_scores.get(&ni.index()) {
                scores.insert(sid.as_str().to_string(), score);
            }
        }

        Ok(RunOutput::PageRank(serde_json::to_value(scores).unwrap()))
    }
}
