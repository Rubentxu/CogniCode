//! PageRank descriptor for the analytics registry.
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR2 Cohort-1 Core.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(unused_imports)]

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
// PageRank Params
// =============================================================================

static PAGERANK_PARAM_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["alpha", "max_iterations", "epsilon"]);

pub struct SimpleParams;

impl AlgorithmParams for SimpleParams {
    fn param_names(&self) -> Vec<&'static str> {
        PAGERANK_PARAM_NAMES.to_vec()
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        if let Some(obj) = params.as_object() {
            for name in PAGERANK_PARAM_NAMES.iter() {
                if !obj.contains_key(*name) {
                    return Err(format!("missing parameter: {}", name));
                }
            }
            Ok(())
        } else {
            Err("params must be a JSON object".into())
        }
    }
}

// =============================================================================
// PageRank output schema
// =============================================================================

static PAGERANK_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
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
// PageRank limits
// =============================================================================

static PAGERANK_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
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
// PageRank complexity
// =============================================================================

static PAGERANK_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E)",
    space: "O(V)",
    notes: "per iteration; typically converges in < 100 iterations",
});

// =============================================================================
// PageRank conformance fixtures
// =============================================================================

static PAGERANK_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        // 3-node cycle: A→B→C→A
        Fixture {
            name: "3-node cycle",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C"), ("C", "A")],
            },
            expected: serde_json::json!({
                "type": "uniform_cycle",
                "expectation": "equal_scores"
            }),
        },
        // Star: center C, leaves L1..L5 calling center
        Fixture {
            name: "star center highest",
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
                "expectation": "center_highest_score"
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
// PageRank identity
// =============================================================================

static PAGERANK_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("pagerank"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Stable,
    cohort: 1,
});

// =============================================================================
// PageRank descriptor
// =============================================================================

/// PageRank descriptor for the analytics registry.
///
/// Wraps `cognicode_graph_algos::page_rank` with the following contract:
/// - `alpha = 0.85` (damping factor)
/// - `epsilon = 1e-6` (convergence tolerance)
/// - `max_iterations = 100`
/// - Directed: yes (call graph is directed)
/// - Weighted: no
/// - Determinism: Seeded { required: false, default: Some(0) }
/// - Tolerance: 1e-6 for score comparison
pub struct PageRankDescriptor;

impl AlgorithmDescriptor for PageRankDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &PAGERANK_IDENTITY
    }

    fn params(&self) -> &dyn AlgorithmParams {
        &SimpleParams
    }

    fn output_schema(&self) -> &OutputSchema {
        &PAGERANK_SCHEMA
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
        &PAGERANK_COMPLEXITY
    }

    fn limits(&self) -> &PlanLimits {
        &PAGERANK_LIMITS
    }

    fn conformance_fixtures(&self) -> &[Fixture] {
        &PAGERANK_FIXTURES
    }

    fn determinism(&self) -> DeterminismKind {
        // PageRank is deterministic for fixed alpha, epsilon, max_iterations.
        // Seed only affects tie-breaking when scores are nearly equal.
        DeterminismKind::Seeded {
            required: false,
            default: Some(0),
        }
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
        // PageRank accumulates on callees (incoming edges)
        &ProjectionAssumption::CallGraphIncoming
    }
}

#[async_trait::async_trait]
impl AlgorithmExecute for PageRankDescriptor {
    async fn execute(
        &self,
        params: &serde_json::Value,
        graph: &CallGraph,
        _limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        let obj = params.as_object().ok_or_else(|| {
            AnalyticsError::Internal("PageRank params must be a JSON object".into())
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

        let raw_scores =
            cognicode_graph_algos::page_rank(&in_neighbors, &out_degree, n, alpha, max_iterations);

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
