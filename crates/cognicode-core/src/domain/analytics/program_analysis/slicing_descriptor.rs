//! Forward + backward slicer descriptor (WU3) — minimal stub.

#![allow(dead_code, unused_imports)]

use std::sync::LazyLock;

use crate::domain::analytics::descriptor::{
    AlgorithmDescriptor, AlgorithmId, AlgorithmIdentity, AlgorithmParams, AlgorithmVersion,
    AnalyticsMode, ComplexityClass, DeterminismKind, Fixture, FixtureGraph, Maturity, OutputField,
    OutputSchema, OutputType, ProjectionAssumption,
};
use crate::domain::analytics::program_analysis::{FunctionId, SLICE_BACKWARD, SLICE_FORWARD};
use crate::domain::plan::limits::PlanLimits;

static SLICE_PARAM_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["function_id", "variable", "definition_site"]);

pub struct SlicingParams;

impl AlgorithmParams for SlicingParams {
    fn param_names(&self) -> Vec<&'static str> {
        SLICE_PARAM_NAMES.to_vec()
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        let obj = params.as_object().ok_or("params must be a JSON object")?;
        for key in &["function_id", "variable", "definition_site"] {
            if !obj.contains_key(*key) {
                return Err(format!("missing required parameter: {}", key));
            }
        }
        if !obj.get("variable").map(|v| v.is_string()).unwrap_or(false) {
            return Err("variable must be a string".into());
        }
        Ok(())
    }
}

static SLICING_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("slice_forward"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Experimental,
    cohort: 5,
});

static SLICING_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField {
            name: "function_id",
            type_: OutputType::NodeId,
        },
        OutputField {
            name: "variable",
            type_: OutputType::Json,
        },
        OutputField {
            name: "nodes",
            type_: OutputType::Json,
        },
    ],
});

static SLICING_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E) per slice",
    space: "O(V)",
    notes: "BFS over the CFG from the criterion",
});

static SLICING_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![Fixture {
        name: "backward from use reaches def",
        graph: FixtureGraph {
            nodes: vec!["def", "middle", "use"],
            edges: vec![("def", "middle"), ("middle", "use")],
        },
        expected: serde_json::json!({"type": "backward"}),
    }]
});

static SLICING_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
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

static SLICING_MODES: LazyLock<Vec<AnalyticsMode>> = LazyLock::new(|| {
    vec![
        AnalyticsMode::Stream,
        AnalyticsMode::Stats,
        AnalyticsMode::Annotate,
    ]
});

pub struct SlicingDescriptor;

impl AlgorithmDescriptor for SlicingDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &SLICING_IDENTITY
    }
    fn params(&self) -> &dyn AlgorithmParams {
        &SlicingParams
    }
    fn output_schema(&self) -> &OutputSchema {
        &SLICING_SCHEMA
    }
    fn supported_modes(&self) -> &[AnalyticsMode] {
        SLICING_MODES.as_ref()
    }
    fn complexity(&self) -> &ComplexityClass {
        &SLICING_COMPLEXITY
    }
    fn limits(&self) -> &PlanLimits {
        &SLICING_LIMITS
    }
    fn conformance_fixtures(&self) -> &[Fixture] {
        SLICING_FIXTURES.as_ref()
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

// Slicing is exposed under two algorithm ids — forward and backward. The
// canonical descriptor id is `slice_forward`; the backward id routes to the
// same descriptor. The service treats both ids as supported.
impl SlicingDescriptor {
    pub fn aliases() -> Vec<AlgorithmId> {
        vec![SLICE_FORWARD.clone(), SLICE_BACKWARD.clone()]
    }
}

#[allow(dead_code)]
fn _suppress(_: FunctionId) {}

pub fn slice_forward_id() -> AlgorithmId {
    SLICE_FORWARD.clone()
}
pub fn slice_backward_id() -> AlgorithmId {
    SLICE_BACKWARD.clone()
}
