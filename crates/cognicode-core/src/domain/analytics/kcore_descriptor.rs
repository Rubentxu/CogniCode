//! K-Core descriptor for the analytics registry.
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
// K-Core Params
// =============================================================================

/// Parameter names for k-core.
static KCORE_PARAM_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["k"]);

/// K-Core parameter schema.
///
/// Required: `k` — minimum degree threshold for the core.
pub struct KCoreParams;

impl AlgorithmParams for KCoreParams {
    fn param_names(&self) -> Vec<&'static str> {
        KCORE_PARAM_NAMES.to_vec()
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        if let Some(obj) = params.as_object() {
            if !obj.contains_key("k") {
                return Err("missing required parameter: k".into());
            }
            let k = obj.get("k")
                .ok_or("missing k")?;
            if !k.is_u64() {
                return Err("k must be a non-negative integer".into());
            }
            let k_val = k.as_u64().unwrap();
            if k_val > 1000 {
                return Err("k must be <= 1000".into());
            }
            Ok(())
        } else {
            Err("params must be a JSON object".into())
        }
    }
}

// =============================================================================
// K-Core output schema
// =============================================================================

static KCORE_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField { name: "nodes", type_: OutputType::NodeId },
        OutputField { name: "core_numbers", type_: OutputType::Json },
    ],
});

// =============================================================================
// K-Core limits
// =============================================================================

static KCORE_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
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
// K-Core complexity
// =============================================================================

static KCORE_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E)".into(),
    space: "O(V)".into(),
    notes: "Iterative peeling, degree-by-degree removal".into(),
});

// =============================================================================
// K-Core conformance fixtures
// =============================================================================

static KCORE_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        // Triangle A-B-C-A: all nodes have degree 2, survive k=2
        Fixture {
            name: "triangle k2 all nodes",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C"), ("C", "A")],
            },
            expected: serde_json::json!({
                "type": "triangle",
                "expectation": "all_survive_k2"
            }),
        },
        // Path A-B-C: k=2 returns empty (no node has degree >= 2)
        Fixture {
            name: "path k2 empty",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C")],
            },
            expected: serde_json::json!({
                "type": "path",
                "expectation": "empty_k2"
            }),
        },
        // Path A-B-C: k=1 returns all nodes
        Fixture {
            name: "path k1 all nodes",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C")],
            },
            expected: serde_json::json!({
                "type": "path",
                "expectation": "all_survive_k1"
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
// K-Core identity
// =============================================================================

static KCORE_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("k_core"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Stable,
    cohort: 2,
});

// =============================================================================
// K-Core descriptor
// =============================================================================

/// K-Core descriptor using iterative degree peeling.
///
/// Wraps `cognicode_graph_algos::k_core`:
/// - `k`: REQUIRED — minimum degree threshold
/// - Deterministic: sorted by node_id
/// - Directed: NO (treats graph as undirected)
/// - Weighted: no
/// - Heterogeneous: no
pub struct KCoreDescriptor;

impl AlgorithmDescriptor for KCoreDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &KCORE_IDENTITY
    }

    fn params(&self) -> &dyn AlgorithmParams {
        &KCoreParams
    }

    fn output_schema(&self) -> &OutputSchema {
        &KCORE_SCHEMA
    }

    fn supported_modes(&self) -> &[AnalyticsMode] {
        &[
            AnalyticsMode::Stream,
            AnalyticsMode::Stats,
            AnalyticsMode::Annotate,
        ]
    }

    fn complexity(&self) -> &ComplexityClass {
        &KCORE_COMPLEXITY
    }

    fn limits(&self) -> &PlanLimits {
        &KCORE_LIMITS
    }

    fn conformance_fixtures(&self) -> &[Fixture] {
        &KCORE_FIXTURES
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
impl AlgorithmExecute for KCoreDescriptor {
    async fn execute(
        &self,
        params: &serde_json::Value,
        graph: &CallGraph,
        _limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        let obj = params
            .as_object()
            .ok_or_else(|| AnalyticsError::Internal("KCore params must be a JSON object".into()))?;

        let k = obj
            .get("k")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(1);

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

        let raw = cognicode_graph_algos::k_core(&undirected, n, k);

        // Unpack into two parallel vectors
        let nodes: Vec<usize> = raw.iter().map(|(v, _)| *v).collect();
        let core_numbers: Vec<u32> = raw.iter().map(|(_, c)| *c).collect();

        Ok(RunOutput::KCore {
            nodes,
            core_numbers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kcore_descriptor_has_correct_identity() {
        let d = KCoreDescriptor;
        let id = d.identity();
        assert_eq!(id.id.as_str(), "k_core");
        assert_eq!(id.version.as_str(), "1.0.0");
        assert_eq!(id.maturity, Maturity::Stable);
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
    fn kcore_descriptor_is_undirected() {
        let d = KCoreDescriptor;
        assert!(!d.directed()); // undirected
        assert!(!d.weighted());
        assert!(!d.heterogeneous());
    }
}
