//! Dominators descriptor for the analytics registry.
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
// Dominators Params
// =============================================================================

/// Parameter names for dominators.
static DOMINATORS_PARAM_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["root_symbol"]);

/// Dominators parameter schema.
///
/// Required: `root_symbol` — the entry point symbol name.
pub struct DominatorsParams;

impl AlgorithmParams for DominatorsParams {
    fn param_names(&self) -> Vec<&'static str> {
        DOMINATORS_PARAM_NAMES.to_vec()
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        if let Some(obj) = params.as_object() {
            if !obj.contains_key("root_symbol") {
                return Err("missing required parameter: root_symbol".into());
            }
            if !obj.get("root_symbol").map(|v| v.is_string()).unwrap_or(false) {
                return Err("root_symbol must be a string".into());
            }
            Ok(())
        } else {
            Err("params must be a JSON object".into())
        }
    }
}

// =============================================================================
// Dominators output schema
// =============================================================================

static DOMINATORS_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField { name: "nodes", type_: OutputType::NodeId },
        OutputField { name: "immediate_dominators", type_: OutputType::Json },
        OutputField { name: "depths", type_: OutputType::Json },
    ],
});

// =============================================================================
// Dominators limits
// =============================================================================

static DOMINATORS_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
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
// Dominators complexity
// =============================================================================

static DOMINATORS_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E)".into(),
    space: "O(V)".into(),
    notes: "CHK algorithm with Union-Find, two-phase pass".into(),
});

// =============================================================================
// Dominators conformance fixtures
// =============================================================================

static DOMINATORS_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        // Chain A → B → C: B dominates C, A dominates B and C
        Fixture {
            name: "chain dominators",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C")],
            },
            expected: serde_json::json!({
                "type": "chain",
                "expectation": "b_dominates_c"
            }),
        },
        // Diamond: A → B, A → C, B → D, C → D
        // D's idom = A (no node strictly between A and D dominates D)
        Fixture {
            name: "diamond dominators",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C", "D"],
                edges: vec![("A", "B"), ("A", "C"), ("B", "D"), ("C", "D")],
            },
            expected: serde_json::json!({
                "type": "diamond",
                "expectation": "a_dominates_d"
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
// Dominators identity
// =============================================================================

static DOMINATORS_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("dominators"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Stable,
    cohort: 2,
});

// =============================================================================
// Dominators descriptor
// =============================================================================

/// Dominators descriptor using the CHK algorithm.
///
/// Wraps `cognicode_graph_algos::dominators`:
/// - `root_symbol`: REQUIRED — entry point symbol name
/// - Deterministic: DFS order with Union-Find
/// - Directed: yes
/// - Weighted: no
/// - Heterogeneous: no
pub struct DominatorsDescriptor;

impl AlgorithmDescriptor for DominatorsDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &DOMINATORS_IDENTITY
    }

    fn params(&self) -> &dyn AlgorithmParams {
        &DominatorsParams
    }

    fn output_schema(&self) -> &OutputSchema {
        &DOMINATORS_SCHEMA
    }

    fn supported_modes(&self) -> &[AnalyticsMode] {
        &[
            AnalyticsMode::Stream,
            AnalyticsMode::Stats,
            AnalyticsMode::Annotate,
        ]
    }

    fn complexity(&self) -> &ComplexityClass {
        &DOMINATORS_COMPLEXITY
    }

    fn limits(&self) -> &PlanLimits {
        &DOMINATORS_LIMITS
    }

    fn conformance_fixtures(&self) -> &[Fixture] {
        &DOMINATORS_FIXTURES
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
impl AlgorithmExecute for DominatorsDescriptor {
    async fn execute(
        &self,
        params: &serde_json::Value,
        graph: &CallGraph,
        _limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        let obj = params
            .as_object()
            .ok_or_else(|| AnalyticsError::Internal("Dominators params must be a JSON object".into()))?;

        let root_symbol = obj
            .get("root_symbol")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AnalyticsError::Internal("missing required param: root_symbol".into()))?;

        let projection = CallGraphProjection::from_call_graph(graph);
        let out_neighbors = projection.build_out_neighbors();
        let n = projection.node_count();

        let root_id = crate::domain::aggregates::SymbolId::new(root_symbol.to_string());
        let root_idx = projection
            .id_to_index()
            .get(&root_id)
            .copied()
            .map(|ni| ni.index())
            .unwrap_or(0);

        let raw = cognicode_graph_algos::dominators(&out_neighbors, n, root_idx);

        // Unpack into three parallel vectors
        let nodes: Vec<usize> = raw.iter().map(|(v, _, _)| *v).collect();
        let immediate_dominators: Vec<Option<usize>> = raw.iter().map(|(_, idom, _)| *idom).collect();
        let depths: Vec<u32> = raw.iter().map(|(_, _, d)| *d).collect();

        Ok(RunOutput::Dominators {
            nodes,
            immediate_dominators,
            depths,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominators_descriptor_has_correct_identity() {
        let d = DominatorsDescriptor;
        let id = d.identity();
        assert_eq!(id.id.as_str(), "dominators");
        assert_eq!(id.version.as_str(), "1.0.0");
        assert_eq!(id.maturity, Maturity::Stable);
        assert_eq!(id.cohort, 2);
    }

    #[test]
    fn dominators_descriptor_params_accepted() {
        let d = DominatorsDescriptor;
        let params = serde_json::json!({ "root_symbol": "main" });
        assert!(d.params().validate(&params).is_ok());
    }

    #[test]
    fn dominators_descriptor_rejects_missing_params() {
        let d = DominatorsDescriptor;
        let params = serde_json::json!({});
        assert!(d.params().validate(&params).is_err());
    }

    #[test]
    fn dominators_descriptor_is_directed() {
        let d = DominatorsDescriptor;
        assert!(d.directed());
        assert!(!d.weighted());
        assert!(!d.heterogeneous());
    }
}
