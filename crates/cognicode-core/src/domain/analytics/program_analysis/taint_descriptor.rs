//! Taint v1 descriptor (WU5) — minimal stub.
//!
//! Per design D6: sources, sinks, and untaint patterns are declared per language.
//! Per design D6 tier rules: each emitted `TaintPath` carries a `PrecisionTier`
//! label — `Extracted` for LSP-resolved identity, `Inferred` for local resolver
//! matches, `Ambiguous` for tree-sitter heuristic matches.

#![allow(dead_code, unused_imports)]

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::domain::analytics::descriptor::{
    AlgorithmDescriptor, AlgorithmId, AlgorithmIdentity, AlgorithmParams, AlgorithmVersion,
    AnalyticsMode, ComplexityClass, DeterminismKind, Fixture, FixtureGraph, Maturity, OutputField,
    OutputSchema, OutputType, ProjectionAssumption,
};
use crate::domain::analytics::program_analysis::{FunctionId, TAINT_FLOW};
use crate::domain::plan::limits::PlanLimits;
use crate::domain::traits::code_intelligence::PrecisionTier;

// ============================================================================
// TaintPatterns
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternKind {
    Source,
    Sink,
    Untaint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchShape {
    Identifier { name_regex: String },
    Call { callee_substring: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaintPattern {
    pub kind: PatternKind,
    pub match_shape: MatchShape,
}

/// Declared patterns per language. Per design D6.
/// Uses a string-keyed language identifier to keep `domain::analytics`
/// free of infrastructure imports.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaintPatterns {
    pub per_language: HashMap<String, Vec<TaintPattern>>,
}

impl TaintPatterns {
    /// Conservative v1 Rust pattern set.
    pub fn rust_v1() -> Self {
        let mut per_language = HashMap::new();
        per_language.insert(
            "rust".to_string(),
            vec![
                TaintPattern {
                    kind: PatternKind::Source,
                    match_shape: MatchShape::Call {
                        callee_substring: "std::fs::read".into(),
                    },
                },
                TaintPattern {
                    kind: PatternKind::Source,
                    match_shape: MatchShape::Call {
                        callee_substring: "std::env::var".into(),
                    },
                },
                TaintPattern {
                    kind: PatternKind::Source,
                    match_shape: MatchShape::Call {
                        callee_substring: "std::env::args".into(),
                    },
                },
                TaintPattern {
                    kind: PatternKind::Sink,
                    match_shape: MatchShape::Call {
                        callee_substring: "std::fs::write".into(),
                    },
                },
                TaintPattern {
                    kind: PatternKind::Sink,
                    match_shape: MatchShape::Call {
                        callee_substring: "println!".into(),
                    },
                },
                TaintPattern {
                    kind: PatternKind::Untaint,
                    match_shape: MatchShape::Call {
                        callee_substring: "sanitize".into(),
                    },
                },
            ],
        );
        Self { per_language }
    }
}

// ============================================================================
// TaintPath
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaintSite {
    pub function_id: FunctionId,
    pub node_id: u64,
    pub site_kind: SiteKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SiteKind {
    Source,
    Intermediate,
    Sink,
    Untaint,
}

/// Taint path — note `tier: PrecisionTier` is not serde-derived; serialization
/// for analytics outputs goes through the RunOutput value, not TaintPath directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaintPath {
    pub source: TaintSite,
    pub sink: TaintSite,
    pub intermediates: Vec<TaintSite>,
    pub tier: PrecisionTier,
}

// ============================================================================
// Taint descriptor
// ============================================================================

static TAINT_PARAM_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| vec!["function_id", "dfg_digest"]);

pub struct TaintParams;

impl AlgorithmParams for TaintParams {
    fn param_names(&self) -> Vec<&'static str> {
        TAINT_PARAM_NAMES.to_vec()
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        let obj = params.as_object().ok_or("params must be a JSON object")?;
        for key in &["function_id", "dfg_digest"] {
            if !obj.contains_key(*key) {
                return Err(format!("missing required parameter: {}", key));
            }
        }
        Ok(())
    }
}

static TAINT_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("taint_flow"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Experimental,
    cohort: 5,
});

static TAINT_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField {
            name: "function_id",
            type_: OutputType::NodeId,
        },
        OutputField {
            name: "paths",
            type_: OutputType::Json,
        },
    ],
});

static TAINT_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E) — flow-sensitive forward",
    space: "O(V)",
    notes: "Taint flag propagated along the DFG; declared patterns per language",
});

static TAINT_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        Fixture {
            name: "source to sink emits one path",
            graph: FixtureGraph {
                nodes: vec!["src", "sink"],
                edges: vec![("src", "sink")],
            },
            expected: serde_json::json!({"type": "taint_path"}),
        },
        Fixture {
            name: "untaint breaks path",
            graph: FixtureGraph {
                nodes: vec!["src", "untaint", "sink"],
                edges: vec![("src", "untaint"), ("untaint", "sink")],
            },
            expected: serde_json::json!({"type": "no_path"}),
        },
    ]
});

static TAINT_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
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

static TAINT_MODES: LazyLock<Vec<AnalyticsMode>> = LazyLock::new(|| {
    vec![
        AnalyticsMode::Stream,
        AnalyticsMode::Stats,
        AnalyticsMode::Annotate,
    ]
});

pub struct TaintDescriptor;

impl AlgorithmDescriptor for TaintDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &TAINT_IDENTITY
    }
    fn params(&self) -> &dyn AlgorithmParams {
        &TaintParams
    }
    fn output_schema(&self) -> &OutputSchema {
        &TAINT_SCHEMA
    }
    fn supported_modes(&self) -> &[AnalyticsMode] {
        TAINT_MODES.as_ref()
    }
    fn complexity(&self) -> &ComplexityClass {
        &TAINT_COMPLEXITY
    }
    fn limits(&self) -> &PlanLimits {
        &TAINT_LIMITS
    }
    fn conformance_fixtures(&self) -> &[Fixture] {
        TAINT_FIXTURES.as_ref()
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

pub fn taint_id() -> AlgorithmId {
    TAINT_FLOW.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_v1_has_expected_patterns() {
        let p = TaintPatterns::rust_v1();
        let rust = p.per_language.get("rust").expect("rust patterns");
        assert!(
            rust.iter().any(|p| p.kind == PatternKind::Source
                && matches!(p.match_shape, MatchShape::Call { .. }))
        );
        assert!(rust.iter().any(|p| p.kind == PatternKind::Sink));
        assert!(rust.iter().any(|p| p.kind == PatternKind::Untaint));
    }

    #[test]
    fn taint_descriptor_validates_required_params() {
        let d = TaintDescriptor;
        let good = serde_json::json!({"function_id": "f", "dfg_digest": "abc"});
        assert!(d.params().validate(&good).is_ok());
        let bad = serde_json::json!({});
        assert!(d.params().validate(&bad).is_err());
    }
}
