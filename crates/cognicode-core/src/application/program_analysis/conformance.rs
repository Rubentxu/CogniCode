//! Conformance harness for M5 program-analysis algorithms.
//!
//! Implements the contract from `openspec/specs/program-analysis-conformance/spec.md`
//! (originally authored under `openspec/changes/m5-program-analysis-core/`,
//! archived to the canonical spec tree in 2026-09-15):
//!
//! - Canonical fixture corpus per algorithm kind (one Rust fixture each).
//! - Digest-pin protocol: SHA-256 of the canonical-JSON output.
//! - Replay determinism guard: byte-identical across two runs.
//! - Perf envelope: simple wall-clock measurement for the dispatch path.
//!
//! The corpus is **synthetic** today — the inputs are flat slices the
//! algorithms already accept. A later M5.1 slice lifts these inputs from
//! tree-sitter ASTs; the fixture shape will then change to "source +
//! snapshot id" instead of "raw adjacency + statements".
//!
//! ## Public API
//!
//! The functions and structs in this module are public so MCP handlers
//! and the conformance-test CLI driver can consume them. They are not
//! yet wired from outside the module, which is why they need
//! `#[allow(dead_code)]` until WU6 closes.
#![allow(dead_code)]

use sha2::{Digest, Sha256};
use std::time::Instant;

use crate::application::program_analysis::ProgramAnalysisService;
use crate::domain::analytics::descriptor::RunOutput;
#[cfg(feature = "program-analysis-server")]
use crate::domain::analytics::program_analysis::DFG;
use crate::domain::analytics::program_analysis::{
    CFG_PER_FUNCTION, DOMINATORS_CFG, SLICE_BACKWARD, SLICE_FORWARD, TAINT_FLOW,
};
use crate::domain::plan::limits::PlanLimits;

/// One canonical fixture: input params + a label used in error messages.
#[derive(Debug, Clone)]
pub struct ConformanceFixture {
    /// Algorithm id (one of the M5 constants).
    pub algorithm: &'static str,
    /// Human-readable label.
    pub label: &'static str,
    /// Input JSON the dispatch surface expects.
    pub params: serde_json::Value,
}

/// Returns the canonical M5 conformance corpus.
///
/// Order matters for replay diffs — never reorder these entries without
/// also re-pinning the digest.
pub fn canonical_corpus() -> Vec<ConformanceFixture> {
    vec![
        ConformanceFixture {
            algorithm: "cfg_per_function",
            label: "linear_chain_three_blocks",
            params: serde_json::json!({
                "function_id": "linear",
                "adjacency": [[1], [2], []],
                "root": 0,
                "exits": [2],
            }),
        },
        ConformanceFixture {
            algorithm: "cfg_per_function",
            label: "diamond_branch_four_blocks",
            params: serde_json::json!({
                "function_id": "diamond",
                "adjacency": [[1], [2, 3], [4], [4], []],
                "root": 0,
                "exits": [4],
            }),
        },
        ConformanceFixture {
            algorithm: "dominators_cfg",
            label: "diamond_dominators",
            params: serde_json::json!({
                "function_id": "diamond",
                "cfg_digest": "sha256:diamond",
                "adjacency": [[1], [2, 3], [4], [4], []],
                "root": 0,
            }),
        },
        ConformanceFixture {
            algorithm: "slice_forward",
            label: "linear_forward_slice",
            params: serde_json::json!({
                "function_id": "linear",
                "variable": "x",
                "definition_site": 0,
                "adjacency": [[1], [2], []],
            }),
        },
        ConformanceFixture {
            algorithm: "slice_backward",
            label: "diamond_backward_slice",
            params: serde_json::json!({
                "function_id": "diamond",
                "variable": "a",
                "definition_site": 0,
                "adjacency": [[1], [2, 3], [4], [4], []],
                "use_sites": [4],
            }),
        },
        ConformanceFixture {
            algorithm: "taint_flow",
            label: "linear_taint",
            params: serde_json::json!({
                "function_id": "rust_fn",
                "dfg_digest": "sha256:linear",
                "language": "rust",
                "statements": [
                    {"id": 0, "defs": ["x"], "uses": ["src"]},
                    {"id": 1, "defs": ["y"], "uses": ["x"]},
                    {"id": 2, "defs": [], "uses": ["y"]},
                ],
                "sources": [0],
                "sinks": [2],
                "untaints": [],
            }),
        },
        ConformanceFixture {
            algorithm: "taint_flow",
            label: "taint_breaks_on_untaint",
            params: serde_json::json!({
                "function_id": "rust_fn",
                "dfg_digest": "sha256:sanitized",
                "language": "rust",
                "statements": [
                    {"id": 0, "defs": ["x"], "uses": ["src"]},
                    {"id": 1, "defs": ["y"], "uses": ["x"]},
                    {"id": 2, "defs": [], "uses": ["y"]},
                ],
                "sources": [0],
                "sinks": [2],
                "untaints": [1],
            }),
        },
        // DFG conformance: 2 fixtures covering the basic emission contract.
        // e41 (RETIREMENT-LEDGER housekeeping) — closes the "DFG has no
        // conformance coverage" gap (no fixture of `algorithm = "dfg"` was
        // registered in m5-program-analysis-core; the dispatch exists at
        // program_analysis.rs:348 but was never exercised end-to-end).
        #[cfg(feature = "program-analysis-server")]
        ConformanceFixture {
            algorithm: "dfg",
            label: "linear_def_use_chain",
            params: serde_json::json!({
                "function_id": "linear_chain",
                "cfg_digest": "sha256:linear_chain_cfg",
                "statements": [
                    {"id": 0, "defs": ["x"], "uses": ["src"]},
                    {"id": 1, "defs": ["y"], "uses": ["x"]},
                    {"id": 2, "defs": ["z"], "uses": ["y"]},
                    {"id": 3, "defs": [], "uses": ["z"]},
                ],
            }),
        },
        #[cfg(feature = "program-analysis-server")]
        ConformanceFixture {
            algorithm: "dfg",
            label: "diamond_diamond_diamond",
            params: serde_json::json!({
                "function_id": "diamond",
                "cfg_digest": "sha256:diamond_cfg",
                "statements": [
                    {"id": 0, "defs": ["x"], "uses": ["src"]},
                    {"id": 1, "defs": ["a"], "uses": ["x"]},
                    {"id": 2, "defs": ["b"], "uses": ["x"]},
                    {"id": 3, "defs": ["y"], "uses": ["a", "b"]},
                    {"id": 4, "defs": [], "uses": ["y"]},
                ],
            }),
        },
        #[cfg(feature = "program-analysis-server")]
        ConformanceFixture {
            algorithm: "interproc_summary",
            label: "two_callers_one_callee",
            params: serde_json::json!({
                "function_id": "module",
                "call_graph": [[], [0], [0]],
                "functions": [
                    {"function_id": 0, "calls": [], "statements": []},
                    {"function_id": 1, "calls": [0], "statements": [{"id": 0, "defs": ["x"], "uses": []}]},
                    {"function_id": 2, "calls": [0], "statements": [{"id": 0, "defs": ["y"], "uses": ["z"]}]},
                ],
            }),
        },
    ]
}

/// Per-fixture execution outcome with the bytes the replay guard hashes.
#[derive(Debug, Clone)]
pub struct FixtureOutcome {
    pub label: String,
    /// Canonical-JSON bytes of the dispatch output.
    pub output_bytes: Vec<u8>,
    /// Wall-clock time the dispatch took (in microseconds).
    pub elapsed_us: u128,
    /// Whether the dispatch succeeded.
    pub ok: bool,
}

/// Run the canonical corpus through the service.
///
/// `digest_out` is filled with one digest per fixture (in corpus order).
pub fn run_corpus(
    svc: &ProgramAnalysisService,
    fixtures: &[ConformanceFixture],
) -> (Vec<FixtureOutcome>, Vec<String>) {
    let mut outcomes: Vec<FixtureOutcome> = Vec::with_capacity(fixtures.len());
    let mut digests: Vec<String> = Vec::with_capacity(fixtures.len());

    for fixture in fixtures {
        let id = match fixture.algorithm {
            "cfg_per_function" => CFG_PER_FUNCTION.clone(),
            "dominators_cfg" => DOMINATORS_CFG.clone(),
            "slice_forward" => SLICE_FORWARD.clone(),
            "slice_backward" => SLICE_BACKWARD.clone(),
            "taint_flow" => TAINT_FLOW.clone(),
            #[cfg(feature = "program-analysis-server")]
            "dfg" => DFG.clone(),
            #[cfg(feature = "program-analysis-server")]
            "interproc_summary" => INTERPROC_SUMMARY.clone(),
            other => panic!("unknown algorithm in fixture corpus: {other}"),
        };

        let start = Instant::now();
        let res = svc.dispatch(&id, &fixture.params, &PlanLimits::default());
        let elapsed_us = start.elapsed().as_micros();

        // Extract the inner JSON value from RunOutput. RunOutput is not
        // Serialize itself, but its variants all carry JSON-friendly
        // payloads. PageRank is the only variant M5 emits today.
        let (ok, output_bytes) = match res {
            Ok(run_output) => {
                let value: serde_json::Value = match run_output {
                    RunOutput::PageRank(v)
                    | RunOutput::Scc(v)
                    | RunOutput::Wcc(v)
                    | RunOutput::BoundedShortestPaths(v) => v,
                    RunOutput::Dominators {
                        nodes,
                        immediate_dominators,
                        depths,
                    } => serde_json::json!({
                        "kind": "dominators",
                        "nodes": nodes,
                        "immediate_dominators": immediate_dominators,
                        "depths": depths,
                    }),
                    RunOutput::ArticulationPoints {
                        nodes,
                        cut_vertices_counts,
                    } => serde_json::json!({
                        "kind": "articulation_points",
                        "nodes": nodes,
                        "cut_vertices_counts": cut_vertices_counts,
                    }),
                    RunOutput::Bridges { edges } => serde_json::json!({
                        "kind": "bridges",
                        "edges": edges,
                    }),
                    other => {
                        let _ = other;
                        serde_json::json!({"kind": "unsupported_run_output"})
                    }
                };
                let bytes = serde_json::to_vec(&value)
                    .unwrap_or_else(|e| format!("encode_error:{e}").into_bytes());
                (true, bytes)
            }
            Err(e) => (false, format!("dispatch_error:{e:?}").into_bytes()),
        };

        let mut hasher = Sha256::new();
        hasher.update(&output_bytes);
        let digest = hasher.finalize();
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();

        digests.push(hex);
        outcomes.push(FixtureOutcome {
            label: fixture.label.to_string(),
            output_bytes,
            elapsed_us,
            ok,
        });
    }

    (outcomes, digests)
}

/// Replay guard: run the corpus twice and assert byte-identical artifacts.
///
/// Returns the two digests per fixture. A regression changes the digest
/// between runs (deterministic algorithm + same input = same bytes, by
/// construction).
pub fn replay_guard(
    svc: &ProgramAnalysisService,
    fixtures: &[ConformanceFixture],
) -> Vec<ReplayResult> {
    let (_, digests_a) = run_corpus(svc, fixtures);
    let (_, digests_b) = run_corpus(svc, fixtures);
    digests_a
        .into_iter()
        .zip(digests_b)
        .zip(fixtures.iter())
        .map(|((a, b), fx)| {
            let identical = a == b;
            ReplayResult {
                label: fx.label.to_string(),
                digest_a: a,
                digest_b: b,
                identical,
            }
        })
        .collect()
}

/// Result of one replay check.
#[derive(Debug, Clone)]
pub struct ReplayResult {
    pub label: String,
    pub digest_a: String,
    pub digest_b: String,
    pub identical: bool,
}

/// Perf envelope result for one corpus run.
#[derive(Debug, Clone)]
pub struct PerfEnvelope {
    pub label: String,
    pub elapsed_us: u128,
}

/// Wall-clock envelope for the corpus (median, p95) — useful to publish
/// alongside the digest pins per design D7.
///
/// `budget_us` is the per-fixture budget in microseconds. Any fixture
/// above the budget contributes to a non-zero `over_budget_count`.
pub fn perf_envelope(
    svc: &ProgramAnalysisService,
    fixtures: &[ConformanceFixture],
    budget_us: u128,
) -> PerfSummary {
    let (outcomes, _) = run_corpus(svc, fixtures);
    let per_fixture: Vec<PerfEnvelope> = outcomes
        .iter()
        .map(|o| PerfEnvelope {
            label: o.label.clone(),
            elapsed_us: o.elapsed_us,
        })
        .collect();

    let mut elapsed: Vec<u128> = outcomes.iter().map(|o| o.elapsed_us).collect();
    elapsed.sort_unstable();
    let median_us = if elapsed.is_empty() {
        0
    } else {
        elapsed[elapsed.len() / 2]
    };
    let p95_us = if elapsed.is_empty() {
        0
    } else {
        let idx = ((elapsed.len() as f64) * 0.95).ceil() as usize;
        elapsed[idx.min(elapsed.len() - 1)]
    };
    let max_us = elapsed.last().copied().unwrap_or(0);
    let over_budget_count = outcomes.iter().filter(|o| o.elapsed_us > budget_us).count();
    let total = outcomes.len();

    PerfSummary {
        per_fixture,
        median_us,
        p95_us,
        max_us,
        budget_us,
        over_budget_count,
        total_count: total,
    }
}

/// Aggregate perf summary.
#[derive(Debug, Clone)]
pub struct PerfSummary {
    pub per_fixture: Vec<PerfEnvelope>,
    pub median_us: u128,
    pub p95_us: u128,
    pub max_us: u128,
    pub budget_us: u128,
    pub over_budget_count: usize,
    pub total_count: usize,
}

/// MCP-compatible JSON view of one corpus outcome.
///
/// Shape is deliberately flat: the MCP tool returns one record per
/// fixture with the digest, timing, and a status string. Callers that
/// want the raw output bytes can re-run the fixture against the service.
#[derive(Debug, Clone, serde::Serialize)]
pub struct McpFixtureReport {
    /// Algorithm id (canonical string).
    pub algorithm: String,
    /// Fixture label.
    pub label: String,
    /// 64-char SHA-256 hex digest of the canonical-JSON output.
    pub digest: String,
    /// Wall-clock time (microseconds).
    pub elapsed_us: u128,
    /// `true` iff dispatch returned `Ok`.
    pub ok: bool,
}

/// Build the MCP-facing fixture report for the whole corpus.
pub fn mcp_fixture_report(
    svc: &ProgramAnalysisService,
    fixtures: &[ConformanceFixture],
) -> Vec<McpFixtureReport> {
    let (outcomes, digests) = run_corpus(svc, fixtures);
    outcomes
        .into_iter()
        .zip(digests)
        .zip(fixtures.iter())
        .map(|((o, d), f)| McpFixtureReport {
            algorithm: f.algorithm.to_string(),
            label: o.label,
            digest: d,
            elapsed_us: o.elapsed_us,
            ok: o.ok,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_corpus_has_one_fixture_per_m5_algorithm() {
        let corpus = canonical_corpus();
        let algos: std::collections::BTreeSet<&str> = corpus.iter().map(|f| f.algorithm).collect();
        // Core (non-feature-gated) algorithms.
        for required in [
            "cfg_per_function",
            "dominators_cfg",
            "slice_forward",
            "slice_backward",
            "taint_flow",
        ] {
            assert!(algos.contains(required), "missing fixture for {required}");
        }
    }

    #[test]
    fn run_corpus_produces_digest_per_fixture() {
        let svc = ProgramAnalysisService::new();
        let corpus = canonical_corpus();
        let (outcomes, digests) = run_corpus(&svc, &corpus);
        assert_eq!(outcomes.len(), corpus.len());
        assert_eq!(digests.len(), corpus.len());
        for (o, d) in outcomes.iter().zip(digests.iter()) {
            assert!(o.ok, "fixture {} should succeed", o.label);
            assert_eq!(d.len(), 64, "digest is sha256 hex");
        }
    }

    #[test]
    fn replay_guard_is_byte_identical() {
        let svc = ProgramAnalysisService::new();
        let corpus = canonical_corpus();
        let results = replay_guard(&svc, &corpus);
        assert_eq!(results.len(), corpus.len());
        for r in &results {
            assert!(
                r.identical,
                "{}: digests differ between runs: {} vs {}",
                r.label, r.digest_a, r.digest_b
            );
        }
    }

    #[test]
    fn perf_envelope_reports_median_and_p95() {
        let svc = ProgramAnalysisService::new();
        let corpus = canonical_corpus();
        let summary = perf_envelope(&svc, &corpus, u128::MAX);
        assert_eq!(summary.total_count, corpus.len());
        assert_eq!(summary.per_fixture.len(), corpus.len());
        // Median should be ≤ max; p95 should be ≤ max.
        assert!(summary.median_us <= summary.max_us);
        assert!(summary.p95_us <= summary.max_us);
        // With an infinite budget nothing should be over budget.
        assert_eq!(summary.over_budget_count, 0);
    }
}
