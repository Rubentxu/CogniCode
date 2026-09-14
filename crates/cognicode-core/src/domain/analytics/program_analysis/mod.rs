//! Program Analysis Core — types, IDs, and shared definitions (M5).
//!
//! Re-exported by individual descriptors. Algorithms live in `cognicode-graph-algos`
//! as free functions; descriptors in `cognicode-core::domain::analytics` carry them
//! through the analytics registry; the service facade is in
//! `cognicode-core::application::program_analysis::ProgramAnalysisService`.
//!
//! Per design D4, every output is canonical-serialized (sorted, stable ids, no
//! timestamps) and pinned to a digest. Per design D5, every operation is bounded
//! by `PlanLimits`. Per design D6, taint v1 sources/sinks are declared per
//! language in `TaintPatterns`. Per design D8, DFG / summaries / taint are
//! gated behind the `program-analysis-server` feature flag.

#![allow(missing_docs)]
#![forbid(unsafe_code)]

use std::sync::LazyLock;

use crate::domain::analytics::descriptor::{AlgorithmId, AnalyticsError, RunOutput};

// ============================================================================
// M5 Algorithm IDs (LazyLock because AlgorithmId::from_static is non-const)
// ============================================================================

/// Per-function control-flow graph.
pub static CFG_PER_FUNCTION: LazyLock<AlgorithmId> =
    LazyLock::new(|| AlgorithmId::from_static("cfg_per_function"));

/// Per-function data-flow graph (definitions to uses within a function).
pub static DFG: LazyLock<AlgorithmId> = LazyLock::new(|| AlgorithmId::from_static("dfg"));

/// Forward slicing from a definition site.
pub static SLICE_FORWARD: LazyLock<AlgorithmId> =
    LazyLock::new(|| AlgorithmId::from_static("slice_forward"));

/// Backward slicing to a use site.
pub static SLICE_BACKWARD: LazyLock<AlgorithmId> =
    LazyLock::new(|| AlgorithmId::from_static("slice_backward"));

/// Per-function dominators (immediate dominator per node).
pub static DOMINATORS_CFG: LazyLock<AlgorithmId> =
    LazyLock::new(|| AlgorithmId::from_static("dominators_cfg"));

/// Interprocedural summaries with recursion fixed-point marker.
pub static INTERPROC_SUMMARY: LazyLock<AlgorithmId> =
    LazyLock::new(|| AlgorithmId::from_static("interproc_summary"));

/// Taint v1 — flow-sensitive forward propagation with declared sources/sinks.
pub static TAINT_FLOW: LazyLock<AlgorithmId> =
    LazyLock::new(|| AlgorithmId::from_static("taint_flow"));

/// Returns the canonical, ordered list of M5 algorithm ids.
pub fn m5_algorithm_ids() -> Vec<AlgorithmId> {
    vec![
        CFG_PER_FUNCTION.clone(),
        DFG.clone(),
        SLICE_FORWARD.clone(),
        SLICE_BACKWARD.clone(),
        DOMINATORS_CFG.clone(),
        INTERPROC_SUMMARY.clone(),
        TAINT_FLOW.clone(),
    ]
}

// ============================================================================
// Per-function scope (D3) — reuses existing Symbol id
// ============================================================================

/// Stable scope identifier for a function-shaped body.
pub type FunctionId = crate::domain::aggregates::SymbolId;

// ============================================================================
// Generic algorithm-result error
// ============================================================================

/// Error returned by M5 algorithms when a resource limit is exceeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitViolation {
    pub kind: &'static str,
    pub limit: u64,
    pub observed: Option<u64>,
}

impl std::fmt::Display for LimitViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.observed {
            Some(o) => write!(
                f,
                "limit exceeded: {} (limit={}, observed={})",
                self.kind, self.limit, o
            ),
            None => write!(f, "limit exceeded: {} (limit={})", self.kind, self.limit),
        }
    }
}

impl std::error::Error for LimitViolation {}

// ============================================================================
// Convenience constructors
// ============================================================================

/// Build an `AnalyticsError::Internal` carrying a `LimitViolation` message.
pub fn limit_violation_error(kind: &'static str, limit: u64, observed: u64) -> AnalyticsError {
    AnalyticsError::Internal(format!(
        "limit exceeded: {} (limit={}, observed={})",
        kind, limit, observed
    ))
}

/// Returns an empty `RunOutput::PageRank` value as a generic placeholder.
pub fn empty_run_output() -> RunOutput {
    RunOutput::PageRank(serde_json::json!([]))
}

// ============================================================================
// Module sub-structure (WU2+) — descriptors live alongside this umbrella
// ============================================================================

pub mod cfg_descriptor;
pub mod dfg_descriptor;
pub mod dominators_cfg_descriptor;
pub mod slicing_descriptor;
pub mod taint_descriptor;

/// Per-function interprocedural summary descriptor (WU4).
/// Gated behind the `program-analysis-server` feature flag per D8.
#[cfg(feature = "program-analysis-server")]
pub mod interproc_summaries_descriptor;
