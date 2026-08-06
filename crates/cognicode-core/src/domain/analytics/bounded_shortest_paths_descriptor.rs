//! Bounded Shortest Paths descriptor for the analytics registry.
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR3 Bounded Paths.

use std::sync::LazyLock;

use crate::domain::aggregates::{CallGraph, SymbolId};
use crate::domain::analytics::{
    AlgorithmDescriptor, AlgorithmExecute, AlgorithmId, AlgorithmIdentity, AlgorithmParams,
    AlgorithmVersion, AnalyticsError, AnalyticsMode, ComplexityClass, DeterminismKind, Fixture,
    FixtureGraph, Maturity, OutputField, OutputSchema, OutputType, ProjectionAssumption, RunOutput,
};
use crate::domain::plan::limits::PlanLimits;
use crate::domain::ports::call_graph_projection::{CallGraphProjectionPort, project_call_graph};

// =============================================================================
// Bounded Shortest Paths params
// =============================================================================

/// Parameter names for bounded shortest paths.
static BSP_PARAM_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["from_symbol", "to_symbol", "max_hops", "max_paths"]);

/// Bounded Shortest Paths parameter schema.
///
/// Required: `from_symbol`, `to_symbol` (symbol names), `max_hops` (positive integer).
/// Optional: `max_paths` (positive integer, max paths to return).
pub struct BoundedShortestPathsParams;

impl AlgorithmParams for BoundedShortestPathsParams {
    fn param_names(&self) -> Vec<&'static str> {
        BSP_PARAM_NAMES.to_vec()
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        if let Some(obj) = params.as_object() {
            // from_symbol is required
            if !obj.contains_key("from_symbol") {
                return Err("missing required parameter: from_symbol".into());
            }
            if !obj
                .get("from_symbol")
                .map(|v| v.is_string())
                .unwrap_or(false)
            {
                return Err("from_symbol must be a string".into());
            }
            // to_symbol is required
            if !obj.contains_key("to_symbol") {
                return Err("missing required parameter: to_symbol".into());
            }
            if !obj.get("to_symbol").map(|v| v.is_string()).unwrap_or(false) {
                return Err("to_symbol must be a string".into());
            }
            // max_hops is required
            if !obj.contains_key("max_hops") {
                return Err("missing required parameter: max_hops".into());
            }
            let max_hops = obj.get("max_hops").ok_or("missing max_hops")?;
            if !max_hops.is_u64() {
                return Err("max_hops must be a positive integer".into());
            }
            let hops_val = max_hops.as_u64().unwrap();
            if hops_val == 0 {
                return Err("max_hops must be > 0".into());
            }
            // max_paths is optional but must be positive if provided
            if let Some(max_paths) = obj.get("max_paths") {
                if !max_paths.is_u64() {
                    return Err("max_paths must be a positive integer".into());
                }
                let paths_val = max_paths.as_u64().unwrap();
                if paths_val == 0 {
                    return Err("max_paths must be > 0".into());
                }
            }
            Ok(())
        } else {
            Err("params must be a JSON object".into())
        }
    }
}

// =============================================================================
// Bounded Shortest Paths output schema
// =============================================================================

static BSP_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField {
            name: "path_id",
            type_: OutputType::Count,
        },
        OutputField {
            name: "nodes",
            type_: OutputType::Json,
        },
        OutputField {
            name: "cost",
            type_: OutputType::Cost,
        },
    ],
});

// =============================================================================
// Bounded Shortest Paths limits
// =============================================================================

static BSP_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
    time_ms: Some(30000),
    cancellation: None,
    max_depth: None,
    max_hops: Some(100), // Required param, defaults to 100
    max_visited_nodes: Some(1_000_000),
    max_visited_edges: None,
    max_result_rows: Some(100_000),
    max_path_count: Some(10_000),
    max_memory_bytes: Some(512 * 1024 * 1024),
});

// =============================================================================
// Bounded Shortest Paths complexity
// =============================================================================

static BSP_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E + k·d)",
    space: "O(V)",
    notes: "k = max_hops, d = graph diameter; bounded DFS",
});

// =============================================================================
// Bounded Shortest Paths conformance fixtures
// =============================================================================

static BSP_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        // Diamond DAG: A → B → D, A → C → D, A → D (direct)
        // Paths from A to D with max_hops=2: [A,D], [A,B,D], [A,C,D]
        Fixture {
            name: "diamond three paths",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C", "D"],
                edges: vec![("A", "B"), ("A", "C"), ("A", "D"), ("B", "D"), ("C", "D")],
            },
            expected: serde_json::json!({
                "type": "diamond_dag",
                "expectation": "three_paths_a_to_d"
            }),
        },
        // Single direct edge: A → B
        Fixture {
            name: "single direct path",
            graph: FixtureGraph {
                nodes: vec!["A", "B"],
                edges: vec![("A", "B")],
            },
            expected: serde_json::json!({
                "type": "direct_edge",
                "expectation": "one_path"
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
// Bounded Shortest Paths identity
// =============================================================================

static BSP_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("bounded_shortest_paths"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Stable,
    cohort: 1,
});

// =============================================================================
// Bounded Shortest Paths descriptor
// =============================================================================

/// Bounded Shortest Paths descriptor.
///
/// Wraps `cognicode_graph_algos::all_simple_paths`:
/// - `max_hops`: REQUIRED — max intermediate nodes (hop limit)
/// - `max_paths`: optional — max paths to return
/// - Deterministic: DFS with alphabetic tie-breaking
/// - Directed: yes
/// - Weighted: no
/// - Tolerance: 1e-9 for cost, 0 for node/edge sequence
pub struct BoundedShortestPathsDescriptor;

impl AlgorithmDescriptor for BoundedShortestPathsDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &BSP_IDENTITY
    }

    fn params(&self) -> &dyn AlgorithmParams {
        &BoundedShortestPathsParams
    }

    fn output_schema(&self) -> &OutputSchema {
        &BSP_SCHEMA
    }

    fn supported_modes(&self) -> &[AnalyticsMode] {
        &[AnalyticsMode::Stream, AnalyticsMode::Persist]
    }

    fn complexity(&self) -> &ComplexityClass {
        &BSP_COMPLEXITY
    }

    fn limits(&self) -> &PlanLimits {
        &BSP_LIMITS
    }

    fn conformance_fixtures(&self) -> &[Fixture] {
        &BSP_FIXTURES
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
        // all_simple_paths uses out_neighbors (CallGraphOutgoing)
        &ProjectionAssumption::CallGraphOutgoing
    }
}

#[async_trait::async_trait]
impl AlgorithmExecute for BoundedShortestPathsDescriptor {
    async fn execute(
        &self,
        params: &serde_json::Value,
        graph: &CallGraph,
        limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError> {
        let obj = params
            .as_object()
            .ok_or_else(|| AnalyticsError::Internal("BSP params must be a JSON object".into()))?;

        let from_symbol = obj
            .get("from_symbol")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AnalyticsError::Internal("missing required param: from_symbol".into())
            })?;
        let to_symbol = obj
            .get("to_symbol")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AnalyticsError::Internal("missing required param: to_symbol".into()))?;
        let max_hops = obj
            .get("max_hops")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(100);
        let max_paths = obj
            .get("max_paths")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        let projection: std::sync::Arc<dyn CallGraphProjectionPort> = project_call_graph(graph);
        let out_neighbors = projection.build_out_neighbors();
        let _n = projection.node_count();

        let from_id = SymbolId::new(from_symbol);
        let to_id = SymbolId::new(to_symbol);

        let (Some(&from_idx), Some(&to_idx)) = (
            projection.symbol_index().get(&from_id),
            projection.symbol_index().get(&to_id),
        ) else {
            // Unknown symbols - return empty result
            let empty: Vec<serde_json::Value> = vec![];
            return Ok(RunOutput::BoundedShortestPaths(
                serde_json::to_value(empty).unwrap(),
            ));
        };

        let mut paths = cognicode_graph_algos::all_simple_paths(
            &out_neighbors,
            from_idx.index(),
            to_idx.index(),
            max_hops,
        );

        // Apply max_paths limit
        if let Some(max_p) = max_paths {
            paths.truncate(max_p);
        }

        // Enforce result_rows limit from limits
        let max_result_rows = limits.max_result_rows.unwrap_or(100_000) as usize;
        paths.truncate(max_result_rows);

        // Map paths to string representation
        let result_paths: Vec<serde_json::Value> = paths
            .into_iter()
            .take(max_result_rows)
            .map(|path| {
                let nodes: Vec<String> = path
                    .into_iter()
                    .filter_map(|idx| {
                        projection
                            .symbol_index()
                            .iter()
                            .find(|(_, ni)| ni.index() == idx)
                            .map(|(sid, _)| sid.as_str().to_string())
                    })
                    .collect();
                serde_json::json!({
                    "nodes": nodes,
                    "cost": 1.0  // unweighted, cost = 1 per edge
                })
            })
            .collect();

        Ok(RunOutput::BoundedShortestPaths(
            serde_json::to_value(result_paths).unwrap(),
        ))
    }
}
