//! Per-function dominators descriptor (WU2) — minimal stub.

#![allow(dead_code, unused_imports)]

use std::sync::LazyLock;

use crate::domain::analytics::descriptor::{
    AlgorithmDescriptor, AlgorithmId, AlgorithmIdentity, AlgorithmParams, AlgorithmVersion,
    AnalyticsMode, ComplexityClass, DeterminismKind, Fixture, FixtureGraph, Maturity, OutputField,
    OutputSchema, OutputType, ProjectionAssumption,
};
use crate::domain::analytics::program_analysis::{DOMINATORS_CFG, FunctionId};
use crate::domain::plan::limits::PlanLimits;

static DOMCFG_PARAM_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["function_id", "cfg_digest"]);

pub struct DominatorsCfgParams;

impl AlgorithmParams for DominatorsCfgParams {
    fn param_names(&self) -> Vec<&'static str> {
        DOMCFG_PARAM_NAMES.to_vec()
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

static DOMCFG_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("dominators_cfg"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Experimental,
    cohort: 5,
});

static DOMCFG_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField {
            name: "function_id",
            type_: OutputType::NodeId,
        },
        OutputField {
            name: "immediate_dominators",
            type_: OutputType::Json,
        },
    ],
});

static DOMCFG_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O((V+E) * alpha(V)) — Cooper-Harvey-Kennedy",
    space: "O(V)",
    notes: "Per-function CHK; bounded by PlanLimits::max_visited_nodes",
});

static DOMCFG_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![Fixture {
        name: "entry dominates everything",
        graph: FixtureGraph {
            nodes: vec!["entry", "a", "b"],
            edges: vec![("entry", "a"), ("a", "b")],
        },
        expected: serde_json::json!({"entry_dominates": ["a", "b"]}),
    }]
});

static DOMCFG_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
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

static DOMCFG_MODES: LazyLock<Vec<AnalyticsMode>> = LazyLock::new(|| {
    vec![
        AnalyticsMode::Stream,
        AnalyticsMode::Stats,
        AnalyticsMode::Annotate,
    ]
});

pub struct DominatorsCfgDescriptor;

impl AlgorithmDescriptor for DominatorsCfgDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &DOMCFG_IDENTITY
    }
    fn params(&self) -> &dyn AlgorithmParams {
        &DominatorsCfgParams
    }
    fn output_schema(&self) -> &OutputSchema {
        &DOMCFG_SCHEMA
    }
    fn supported_modes(&self) -> &[AnalyticsMode] {
        DOMCFG_MODES.as_ref()
    }
    fn complexity(&self) -> &ComplexityClass {
        &DOMCFG_COMPLEXITY
    }
    fn limits(&self) -> &PlanLimits {
        &DOMCFG_LIMITS
    }
    fn conformance_fixtures(&self) -> &[Fixture] {
        DOMCFG_FIXTURES.as_ref()
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

pub fn dominators_cfg_id() -> AlgorithmId {
    DOMINATORS_CFG.clone()
}
