//! K-Core descriptor for the analytics registry.
//!
//! Part of E28.5 Structural Analytics Cohort 2 — PR2 Descriptors.

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
// K-Core Params
// =============================================================================

/// Parameter names for k-core.
static KCORE_PARAM_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| vec!["k"]);

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
            let k = obj.get("k").ok_or("missing k")?;
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
        OutputField {
            name: "nodes",
            type_: OutputType::NodeId,
        },
        OutputField {
            name: "core_numbers",
            type_: OutputType::Json,
        },
    ],
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

impl_cohort2_descriptor!(
    KCoreDescriptor,
    false,                            // directed
    &KCORE_IDENTITY,                  // identity
    &KCoreParams,                     // params
    &KCORE_SCHEMA,                    // output_schema
    &KCORE_FIXTURES,                  // conformance_fixtures
    &KCORE_COMPLEXITY,                // complexity
    ProjectionAssumption::Undirected  // projection_assumption
);

#[async_trait::async_trait]
impl AlgorithmExecute for KCoreDescriptor {
    async fn execute(
        &self,
        params: &serde_json::Value,
        graph: &CallGraph,
        limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        let obj = params
            .as_object()
            .ok_or_else(|| AnalyticsError::Internal("KCore params must be a JSON object".into()))?;

        let k = obj
            .get("k")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(1);

        let projection: std::sync::Arc<dyn CallGraphProjectionPort> = project_call_graph(graph);
        let undirected = projection.build_undirected_neighbors();
        let n = projection.node_count();

        let raw = cognicode_graph_algos::k_core(&undirected, n, k);

        // Enforce max_result_rows limit
        let max_rows = limits.max_result_rows.unwrap_or(100_000) as usize;

        // Unpack into two parallel vectors
        let mut nodes: Vec<usize> = raw.iter().map(|(v, _)| *v).collect();
        let mut core_numbers: Vec<u32> = raw.iter().map(|(_, c)| *c).collect();

        // Truncate to max_result_rows
        nodes.truncate(max_rows);
        core_numbers.truncate(max_rows);

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
