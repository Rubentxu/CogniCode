//! Interprocedural summary descriptor (WU4) — minimal stub.

#![allow(dead_code, unused_imports)]

use std::sync::LazyLock;

use crate::domain::analytics::descriptor::{
    AlgorithmDescriptor, AlgorithmId, AlgorithmIdentity, AlgorithmParams, AlgorithmVersion,
    AnalyticsMode, ComplexityClass, DeterminismKind, Fixture, FixtureGraph, Maturity, OutputField,
    OutputSchema, OutputType, ProjectionAssumption,
};
use crate::domain::analytics::program_analysis::{FunctionId, INTERPROC_SUMMARY};
use crate::domain::plan::limits::PlanLimits;

static INTERPROC_PARAM_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| vec!["function_id"]);

pub struct InterprocSummaryParams;

impl AlgorithmParams for InterprocSummaryParams {
    fn param_names(&self) -> Vec<&'static str> {
        INTERPROC_PARAM_NAMES.to_vec()
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        let obj = params.as_object().ok_or("params must be a JSON object")?;
        if !obj.contains_key("function_id") {
            return Err("missing required parameter: function_id".into());
        }
        Ok(())
    }
}

static INTERPROC_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("interproc_summary"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Experimental,
    cohort: 5,
});

static INTERPROC_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField {
            name: "function_id",
            type_: OutputType::NodeId,
        },
        OutputField {
            name: "summary_id",
            type_: OutputType::Json,
        },
        OutputField {
            name: "reads",
            type_: OutputType::Json,
        },
        OutputField {
            name: "writes",
            type_: OutputType::Json,
        },
        OutputField {
            name: "calls",
            type_: OutputType::Json,
        },
        OutputField {
            name: "kind",
            type_: OutputType::Json,
        },
    ],
});

static INTERPROC_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(N * SCC) — bottom-up memoized",
    space: "O(N)",
    notes: "Recursion via cognicode-graph-algos::condensation",
});

static INTERPROC_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        Fixture {
            name: "caller references callee",
            graph: FixtureGraph {
                nodes: vec!["caller", "callee"],
                edges: vec![("caller", "callee")],
            },
            expected: serde_json::json!({"type": "chain"}),
        },
        Fixture {
            name: "recursive call emits fixed-point marker",
            graph: FixtureGraph {
                nodes: vec!["recur"],
                edges: vec![("recur", "recur")],
            },
            expected: serde_json::json!({"type": "fixed_point"}),
        },
    ]
});

static INTERPROC_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
    time_ms: Some(60000),
    cancellation: None,
    max_depth: Some(1000),
    max_hops: None,
    max_visited_nodes: Some(500_000),
    max_visited_edges: None,
    max_result_rows: Some(100_000),
    max_path_count: None,
    max_memory_bytes: Some(512 * 1024 * 1024),
});

static INTERPROC_MODES: LazyLock<Vec<AnalyticsMode>> = LazyLock::new(|| {
    vec![
        AnalyticsMode::Stream,
        AnalyticsMode::Stats,
        AnalyticsMode::Annotate,
    ]
});

pub struct InterprocSummaryDescriptor;

impl AlgorithmDescriptor for InterprocSummaryDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &INTERPROC_IDENTITY
    }
    fn params(&self) -> &dyn AlgorithmParams {
        &InterprocSummaryParams
    }
    fn output_schema(&self) -> &OutputSchema {
        &INTERPROC_SCHEMA
    }
    fn supported_modes(&self) -> &[AnalyticsMode] {
        INTERPROC_MODES.as_ref()
    }
    fn complexity(&self) -> &ComplexityClass {
        &INTERPROC_COMPLEXITY
    }
    fn limits(&self) -> &PlanLimits {
        &INTERPROC_LIMITS
    }
    fn conformance_fixtures(&self) -> &[Fixture] {
        INTERPROC_FIXTURES.as_ref()
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

#[allow(dead_code)]
fn _suppress(_: FunctionId) {}

pub fn interproc_summary_id() -> AlgorithmId {
    INTERPROC_SUMMARY.clone()
}
