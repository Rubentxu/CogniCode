//! CFG per-function descriptor (WU2) — minimal stub.
//!
//! Per design D1, the algorithm lives in `cognicode-graph-algos::algorithms::cfg`
//! as free functions. WU1 establishes the descriptor skeleton; WU2 wires the
//! actual CFG extraction algorithm and conformance fixtures.

#![allow(dead_code, unused_imports)]

use std::sync::LazyLock;

use crate::domain::analytics::descriptor::{
    AlgorithmDescriptor, AlgorithmId, AlgorithmIdentity, AlgorithmParams, AlgorithmVersion,
    AnalyticsMode, ComplexityClass, DeterminismKind, Fixture, FixtureGraph, Maturity, OutputField,
    OutputSchema, OutputType, ProjectionAssumption,
};
use crate::domain::analytics::program_analysis::{CFG_PER_FUNCTION, FunctionId};
use crate::domain::plan::limits::PlanLimits;

static CFG_PARAM_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| vec!["function_id"]);

pub struct CfgParams;

impl AlgorithmParams for CfgParams {
    fn param_names(&self) -> Vec<&'static str> {
        CFG_PARAM_NAMES.to_vec()
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        let obj = params.as_object().ok_or("params must be a JSON object")?;
        if !obj.contains_key("function_id") {
            return Err("missing required parameter: function_id".into());
        }
        Ok(())
    }
}

static CFG_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("cfg_per_function"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Experimental,
    cohort: 5,
});

static CFG_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField {
            name: "function_id",
            type_: OutputType::NodeId,
        },
        OutputField {
            name: "entry",
            type_: OutputType::Json,
        },
        OutputField {
            name: "nodes",
            type_: OutputType::Json,
        },
        OutputField {
            name: "edges",
            type_: OutputType::Json,
        },
    ],
});

static CFG_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(N) where N = AST node count",
    space: "O(N)",
    notes: "Single tree-sitter pass per function body",
});

static CFG_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        Fixture {
            name: "linear function",
            graph: FixtureGraph {
                nodes: vec!["entry", "stmt1", "stmt2", "exit"],
                edges: vec![("entry", "stmt1"), ("stmt1", "stmt2"), ("stmt2", "exit")],
            },
            expected: serde_json::json!({"type": "linear"}),
        },
        Fixture {
            name: "empty function",
            graph: FixtureGraph {
                nodes: vec!["entry"],
                edges: vec![],
            },
            expected: serde_json::json!({"type": "empty", "nodes": 1, "edges": 0}),
        },
    ]
});

static CFG_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
    time_ms: Some(30000),
    cancellation: None,
    max_depth: None,
    max_hops: None,
    max_visited_nodes: Some(100_000),
    max_visited_edges: None,
    max_result_rows: Some(100_000),
    max_path_count: None,
    max_memory_bytes: Some(256 * 1024 * 1024),
});

static CFG_MODES: LazyLock<Vec<AnalyticsMode>> = LazyLock::new(|| {
    vec![
        AnalyticsMode::Stream,
        AnalyticsMode::Stats,
        AnalyticsMode::Annotate,
    ]
});

pub struct CfgDescriptor;

impl AlgorithmDescriptor for CfgDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &CFG_IDENTITY
    }
    fn params(&self) -> &dyn AlgorithmParams {
        &CfgParams
    }
    fn output_schema(&self) -> &OutputSchema {
        &CFG_SCHEMA
    }
    fn supported_modes(&self) -> &[AnalyticsMode] {
        CFG_MODES.as_ref()
    }
    fn complexity(&self) -> &ComplexityClass {
        &CFG_COMPLEXITY
    }
    fn limits(&self) -> &PlanLimits {
        &CFG_LIMITS
    }
    fn conformance_fixtures(&self) -> &[Fixture] {
        CFG_FIXTURES.as_ref()
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
        &ProjectionAssumption::Any
    }
}

#[allow(dead_code)]
fn _suppress(_: FunctionId) {}

pub fn cfg_id() -> AlgorithmId {
    CFG_PER_FUNCTION.clone()
}
