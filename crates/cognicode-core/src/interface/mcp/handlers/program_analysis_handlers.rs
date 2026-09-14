//! MCP handlers for the M5 program-analysis algorithm IDs.
//!
//! Per design D1 (m5-mcp-wiring), each handler reuses
//! [`crate::application::program_analysis::ProgramAnalysisService`] as the
//! single dispatch entry point. The handlers extract a JSON-friendly value
//! from [`crate::domain::analytics::descriptor::RunOutput`] using the same
//! extractor pattern as the conformance harness, so the MCP wire output and
//! the direct `dispatch()` output are byte-identical for the same input.
//!
//! The interprocedural-summary handler is feature-gated behind
//! `program-analysis-server` (mirrors the dispatch surface).

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::application::program_analysis::ProgramAnalysisService;
use crate::domain::analytics::descriptor::{AlgorithmId, AnalyticsError, RunOutput};
use crate::domain::plan::limits::PlanLimits;

// ---------------------------------------------------------------------------
// Input shapes
// ---------------------------------------------------------------------------

/// Wrapper shared by every M5 tool call. `algorithm_params` matches the
/// conformance-corpus shape so callers can replay fixtures end-to-end.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProgramAnalysisToolInput {
    /// Algorithm-specific parameters (shape per tool).
    pub algorithm_params: serde_json::Value,
    /// Optional [`PlanLimits`] override. `None` (or empty) → defaults.
    #[serde(default)]
    pub limits: Option<PlanLimits>,
}

/// Common output shape: a JSON text body returned to the MCP client.
#[derive(Debug, Clone, Serialize)]
pub struct ProgramAnalysisToolOutput {
    /// The algorithm id (canonical string).
    pub algorithm: &'static str,
    /// Canonical-JSON representation of [`RunOutput`].
    pub value: serde_json::Value,
    /// 64-char SHA-256 hex digest of the canonical-JSON output (replay guard).
    pub digest: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Dispatch a tool call to `ProgramAnalysisService` and serialize the result.
fn dispatch_to_tool_output(
    svc: &ProgramAnalysisService,
    algorithm: &'static str,
    params: &serde_json::Value,
    limits: PlanLimits,
) -> Result<ProgramAnalysisToolOutput, String> {
    let id = AlgorithmId::from_static(algorithm);
    let res = svc.dispatch(&id, params, &limits);
    let value = match res {
        Ok(run_output) => run_output_to_json(&run_output)
            .ok_or_else(|| format!("unsupported RunOutput variant for `{algorithm}`"))?,
        Err(e) => return Err(format!("analytics_error: {e}")),
    };
    let bytes = serde_json::to_vec(&value).map_err(|e| format!("serialize_run_output:{e}"))?;
    let digest = sha256_hex(&bytes);
    Ok(ProgramAnalysisToolOutput {
        algorithm,
        value,
        digest,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Extract a [`serde_json::Value`] view from a [`RunOutput`].
///
/// Mirrors `application::program_analysis::conformance::run_corpus` extractor.
fn run_output_to_json(run_output: &RunOutput) -> Option<serde_json::Value> {
    match run_output {
        RunOutput::PageRank(v)
        | RunOutput::Scc(v)
        | RunOutput::Wcc(v)
        | RunOutput::BoundedShortestPaths(v) => Some(v.clone()),
        RunOutput::Dominators {
            nodes,
            immediate_dominators,
            depths,
        } => Some(serde_json::json!({
            "kind": "dominators",
            "nodes": nodes,
            "immediate_dominators": immediate_dominators,
            "depths": depths,
        })),
        RunOutput::ArticulationPoints {
            nodes,
            cut_vertices_counts,
        } => Some(serde_json::json!({
            "kind": "articulation_points",
            "nodes": nodes,
            "cut_vertices_counts": cut_vertices_counts,
        })),
        RunOutput::Bridges { edges } => Some(serde_json::json!({
            "kind": "bridges",
            "edges": edges,
        })),
        _ => None,
    }
}

// Convenience: turn an `AnalyticsError` into a flat string for the MCP error path.
#[allow(dead_code)]
fn format_analytics_err(e: AnalyticsError) -> String {
    format!("analytics_error: {e}")
}

// ---------------------------------------------------------------------------
// Per-tool handlers
// ---------------------------------------------------------------------------

pub fn handle_cfg(input: ProgramAnalysisToolInput) -> Result<String, String> {
    let svc = ProgramAnalysisService::new();
    let limits = input.limits.unwrap_or_default();
    dispatch_to_tool_output(&svc, "cfg_per_function", &input.algorithm_params, limits)
        .map(|out| serde_json::to_string(&out).unwrap_or_else(|e| format!("encode_error:{e}")))
}

pub fn handle_dominators_cfg(input: ProgramAnalysisToolInput) -> Result<String, String> {
    let svc = ProgramAnalysisService::new();
    let limits = input.limits.unwrap_or_default();
    dispatch_to_tool_output(&svc, "dominators_cfg", &input.algorithm_params, limits)
        .map(|out| serde_json::to_string(&out).unwrap_or_else(|e| format!("encode_error:{e}")))
}

pub fn handle_slice_forward(input: ProgramAnalysisToolInput) -> Result<String, String> {
    let svc = ProgramAnalysisService::new();
    let limits = input.limits.unwrap_or_default();
    dispatch_to_tool_output(&svc, "slice_forward", &input.algorithm_params, limits)
        .map(|out| serde_json::to_string(&out).unwrap_or_else(|e| format!("encode_error:{e}")))
}

pub fn handle_slice_backward(input: ProgramAnalysisToolInput) -> Result<String, String> {
    let svc = ProgramAnalysisService::new();
    let limits = input.limits.unwrap_or_default();
    dispatch_to_tool_output(&svc, "slice_backward", &input.algorithm_params, limits)
        .map(|out| serde_json::to_string(&out).unwrap_or_else(|e| format!("encode_error:{e}")))
}

pub fn handle_taint_flow(input: ProgramAnalysisToolInput) -> Result<String, String> {
    let svc = ProgramAnalysisService::new();
    let limits = input.limits.unwrap_or_default();
    dispatch_to_tool_output(&svc, "taint_flow", &input.algorithm_params, limits)
        .map(|out| serde_json::to_string(&out).unwrap_or_else(|e| format!("encode_error:{e}")))
}

#[cfg(feature = "program-analysis-server")]
pub fn handle_interproc_summary(input: ProgramAnalysisToolInput) -> Result<String, String> {
    let svc = ProgramAnalysisService::new();
    let limits = input.limits.unwrap_or_default();
    dispatch_to_tool_output(&svc, "interproc_summary", &input.algorithm_params, limits)
        .map(|out| serde_json::to_string(&out).unwrap_or_else(|e| format!("encode_error:{e}")))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::program_analysis::conformance::canonical_corpus;

    fn input_for(label: &str) -> ProgramAnalysisToolInput {
        let fx = canonical_corpus()
            .into_iter()
            .find(|f| f.label == label)
            .unwrap_or_else(|| panic!("missing canonical fixture for label `{label}`"));
        ProgramAnalysisToolInput {
            algorithm_params: fx.params,
            limits: None,
        }
    }

    #[test]
    fn handle_cfg_round_trips_through_mcp_layer() {
        let out = handle_cfg(input_for("linear_chain_three_blocks")).expect("dispatch ok");
        // Body is a JSON-encoded ProgramAnalysisToolOutput.
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert_eq!(
            parsed.get("algorithm").and_then(|v| v.as_str()),
            Some("cfg_per_function")
        );
        assert_eq!(
            parsed
                .get("digest")
                .and_then(|v| v.as_str())
                .map(|s| s.len()),
            Some(64)
        );
    }

    #[test]
    fn handle_taint_round_trips_through_mcp_layer() {
        let out = handle_taint_flow(input_for("linear_taint")).expect("dispatch ok");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert_eq!(
            parsed.get("algorithm").and_then(|v| v.as_str()),
            Some("taint_flow")
        );
    }

    #[cfg(feature = "program-analysis-server")]
    #[test]
    fn handle_interproc_summary_round_trips_through_mcp_layer() {
        let out =
            handle_interproc_summary(input_for("two_callers_one_callee")).expect("dispatch ok");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert_eq!(
            parsed.get("algorithm").and_then(|v| v.as_str()),
            Some("interproc_summary")
        );
    }

    #[test]
    fn digest_is_stable_across_two_calls() {
        let a = handle_slice_forward(input_for("linear_forward_slice")).expect("ok");
        let b = handle_slice_forward(input_for("linear_forward_slice")).expect("ok");
        // The two calls produce identical digests — this is the replay guard
        // boundary at the MCP layer.
        let pa: serde_json::Value = serde_json::from_str(&a).unwrap();
        let pb: serde_json::Value = serde_json::from_str(&b).unwrap();
        assert_eq!(pa.get("digest"), pb.get("digest"));
    }

    #[test]
    fn malformed_params_yield_err_string() {
        let input = ProgramAnalysisToolInput {
            algorithm_params: serde_json::json!({"not": "valid"}),
            limits: None,
        };
        let res = handle_cfg(input);
        assert!(res.is_err(), "malformed params must surface as Err");
    }
}
