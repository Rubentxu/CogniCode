//! DFG per-function descriptor (WU3) — minimal stub.

#![allow(dead_code, unused_imports)]

use std::sync::LazyLock;

use crate::domain::analytics::descriptor::{
    AlgorithmDescriptor, AlgorithmId, AlgorithmIdentity, AlgorithmParams, AlgorithmVersion,
    AnalyticsMode, ComplexityClass, DeterminismKind, Fixture, FixtureGraph, Maturity, OutputField,
    OutputSchema, OutputType, ProjectionAssumption,
};
use crate::domain::analytics::program_analysis::{DFG, FunctionId};
use crate::domain::plan::limits::PlanLimits;

static DFG_PARAM_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["function_id", "cfg_digest"]);

pub struct DfgParams;

impl AlgorithmParams for DfgParams {
    fn param_names(&self) -> Vec<&'static str> {
        DFG_PARAM_NAMES.to_vec()
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        let obj = params.as_object().ok_or("params must be a JSON object")?;
        for key in &["function_id", "cfg_digest"] {
            if !obj.contains_key(*key) {
                return Err(format!("missing required parameter: {}", key));
            }
        }
        Ok(())
    }
}

static DFG_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("dfg"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Experimental,
    cohort: 5,
});

static DFG_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField {
            name: "function_id",
            type_: OutputType::NodeId,
        },
        OutputField {
            name: "edges",
            type_: OutputType::Json,
        },
    ],
});

static DFG_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E) over the supplied CFG",
    space: "O(V)",
    notes: "DFG derived from CFG without re-parsing (per spec)",
});

static DFG_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![Fixture {
        name: "definition reaches use",
        graph: FixtureGraph {
            nodes: vec!["def", "use"],
            edges: vec![("def", "use")],
        },
        expected: serde_json::json!({"type": "def_use"}),
    }]
});

static DFG_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
    time_ms: Some(60000),
    cancellation: None,
    max_depth: None,
    max_hops: None,
    max_visited_nodes: Some(100_000),
    max_visited_edges: None,
    max_result_rows: Some(200_000),
    max_path_count: None,
    max_memory_bytes: Some(512 * 1024 * 1024),
});

static DFG_MODES: LazyLock<Vec<AnalyticsMode>> = LazyLock::new(|| {
    vec![
        AnalyticsMode::Stream,
        AnalyticsMode::Stats,
        AnalyticsMode::Annotate,
    ]
});

pub struct DfgDescriptor;

impl AlgorithmDescriptor for DfgDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &DFG_IDENTITY
    }
    fn params(&self) -> &dyn AlgorithmParams {
        &DfgParams
    }
    fn output_schema(&self) -> &OutputSchema {
        &DFG_SCHEMA
    }
    fn supported_modes(&self) -> &[AnalyticsMode] {
        DFG_MODES.as_ref()
    }
    fn complexity(&self) -> &ComplexityClass {
        &DFG_COMPLEXITY
    }
    fn limits(&self) -> &PlanLimits {
        &DFG_LIMITS
    }
    fn conformance_fixtures(&self) -> &[Fixture] {
        DFG_FIXTURES.as_ref()
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

pub fn dfg_id() -> AlgorithmId {
    DFG.clone()
}
