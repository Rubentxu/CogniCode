//! Dominators descriptor for the analytics registry.
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
use crate::domain::ports::call_graph_projection::CallGraphProjectionPort;
use cognicode_graph_algos::GraphBuilder;

// =============================================================================
// Dominators Params
// =============================================================================

/// Parameter names for dominators.
static DOMINATORS_PARAM_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| vec!["root_symbol"]);

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
            if !obj
                .get("root_symbol")
                .map(|v| v.is_string())
                .unwrap_or(false)
            {
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
        OutputField {
            name: "nodes",
            type_: OutputType::NodeId,
        },
        OutputField {
            name: "immediate_dominators",
            type_: OutputType::Json,
        },
        OutputField {
            name: "depths",
            type_: OutputType::Json,
        },
    ],
});

// =============================================================================
// Dominators complexity
// =============================================================================

static DOMINATORS_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E)",
    space: "O(V)",
    notes: "CHK algorithm with Union-Find, two-phase pass",
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

impl_cohort2_descriptor!(
    DominatorsDescriptor,
    true,                                    // directed
    &DOMINATORS_IDENTITY,                    // identity
    &DominatorsParams,                       // params
    &DOMINATORS_SCHEMA,                      // output_schema
    &DOMINATORS_FIXTURES,                    // conformance_fixtures
    &DOMINATORS_COMPLEXITY,                  // complexity
    ProjectionAssumption::CallGraphOutgoing  // projection_assumption
);

#[async_trait::async_trait]
impl AlgorithmExecute for DominatorsDescriptor {
    async fn execute(
        &self,
        params: &serde_json::Value,
        limits: &PlanLimits,
        projection: &dyn CallGraphProjectionPort,
    ) -> Result<RunOutput, AnalyticsError> {
        let obj = params.as_object().ok_or_else(|| {
            AnalyticsError::Internal("Dominators params must be a JSON object".into())
        })?;

        let root_symbol = obj
            .get("root_symbol")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AnalyticsError::Internal("missing required param: root_symbol".into())
            })?;

        let out_neighbors = projection.build_out_neighbors();
        let n = projection.node_count();

        let root_id = crate::domain::aggregates::SymbolId::new(root_symbol);
        let root_idx = projection
            .symbol_index()
            .get(&root_id)
            .copied()
            .map(|ni| ni.index())
            .ok_or_else(|| {
                AnalyticsError::InvalidParameter(format!(
                    "root_symbol '{}' not found in call graph",
                    root_symbol
                ))
            })?;

        let raw = cognicode_graph_algos::dominators(&out_neighbors, n, root_idx);

        // Enforce max_result_rows limit
        let max_rows = limits.max_result_rows.unwrap_or(100_000) as usize;

        // Unpack into three parallel vectors
        let mut nodes: Vec<usize> = raw.iter().map(|(v, _, _)| *v).collect();
        let mut immediate_dominators: Vec<Option<usize>> =
            raw.iter().map(|(_, idom, _)| *idom).collect();
        let mut depths: Vec<u32> = raw.iter().map(|(_, _, d)| *d).collect();

        // Truncate to max_result_rows
        nodes.truncate(max_rows);
        immediate_dominators.truncate(max_rows);
        depths.truncate(max_rows);

        Ok(RunOutput::Dominators {
            nodes,
            immediate_dominators,
            depths,
        })
    }
}
