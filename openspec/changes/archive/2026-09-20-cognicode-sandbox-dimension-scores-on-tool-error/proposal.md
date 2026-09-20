# Proposal: cognicode-sandbox-dimension-scores-on-tool-error

## Intent

Fix a semantic inconsistency in `cognicode-sandbox` where the per-scenario
`dimension_scores` field reports full 5 dimensions (latencia,
escalabilidad, consistencia, robustez) even when the underlying tool call
failed with `mcp_error`, `timeout`, or any other non-pass outcome.

This inconsistency is the root cause of the G3 (Sandbox Health Score) gate
staying RED in the E31-G scorecard: 11 scenarios report
`correctitud=0` (correctly flagging the tool failure) but
`latencia=100, escalabilidad=100, consistencia=95, robustez=0`. The
5-dim weighted average is then ~64, well below the 85/100 G3 threshold.
The scorecard reports these scenarios as "failing health" when they are
actually "tool failed, dimensions not measurable" — semantically
different.

The other dimensions should mirror `correctitud`'s behavior: when the
tool call errors, set them to `None` so the scorecard recognizes the
scenario as `incomplete_5dim` (not evaluable) rather than as a
"failed with high operational scores" false positive.

## Root cause

`crates/cognicode-sandbox/src/main.rs:1479-1484` computes the four
"always-computed dimensions" unconditionally:

```rust
let latencia = compute_latency_score(tool_call_ms, &metrics);
let escalabilidad = compute_scalability_score(workspace_size_kb, tool_call_ms, &metrics);
let consistencia = compute_consistency_score(tool_call_ms, workspace_size_kb, &[]);
let robustez = compute_robustness_score(if tool_call_error { 1 } else { 0 }, 1);
```

`robustez` already keys on `tool_call_error` (line 1484) and reports 0
when the tool errored. `latencia`, `escalabilidad`, and `consistencia`
do not: they use `tool_call_ms` (which may be the time-to-error, e.g.
1ms for a fast-failing call) and report ~100 because the call was fast.

The block at lines 1530-1537 wraps these into
`Some(DimensionScores { ... })` unconditionally, then attaches the
wrapper to the `ScenarioResult` (line 1650).

## Scope

**In scope:**
- Modify the dimension_scores block at
  `crates/cognicode-sandbox/src/main.rs:1530-1537` to set
  `latencia`, `escalabilidad`, and `consistencia` to `None` when
  `tool_call_error` is true. `robustez` is left as-is (already
  handles errors via `compute_robustness_score`).
- `correctitud` already correctly returns `None` when ground_truth is
  absent; its value when present is the responsibility of
  `score_scenario` (out of scope).
- Regression tests in `crates/cognicode-sandbox/src/main.rs`'s
  `mod determine_failure_class_tests` (or a sibling test mod) covering
  the producer behavior: a scenario with a tool error should serialize
  with `latencia=None, escalabilidad=None, consistencia=None`.

**Out of scope:**
- Changing the `sandbox_core::scoring::compute_health_score` formula.
  The Rust aggregator already uses `unwrap_or(0.0)` for missing dims
  and `compute_dimension_averages` already filters by `if let Some(v)
  = ...`, so the producer-side None values are safe.
- Re-running the scorecard to confirm G3 changes status. That happens
  in a follow-up cycle (orchestrator runs `just scorecard-streak` after
  merge).
- Re-classifying the 11 scenarios' existing result.json files (the
  fix only applies to future runs).
- Fixing the G3 mixed-repeats edge case in `release_scorecard.py`
  (separately tracked as a docstring-vs-implementation divergence;
  not on the critical path because the ci_smoke+quality+full_run
  subset has no mixed scenarios — verified via simulation).

## Approach

Single-file change. The block at lines 1530-1537 becomes:

```rust
// Build dimension_scores. When the tool call errored (mcp_error,
// timeout, protocol violation, etc.), the operational dimensions
// (latencia, escalabilidad, consistencia) are NOT measurable because
// the call did not produce a real response. Mark them as None so the
// scorecard recognizes the scenario as incomplete_5dim rather than
// computing a misleadingly high health score from a fast-failing call.
// robustez is computed from the error count and stays Some(0) (already
// handles the error case via compute_robustness_score).
// correctitud already returns None when ground_truth is absent.
let dimension_scores = Some(DimensionScores {
    correctitud,
    latencia: if tool_call_error { None } else {
        Some(latencia).filter(|&v| !v.is_nan())
    },
    escalabilidad: if tool_call_error { None } else {
        Some(escalabilidad).filter(|&v| !v.is_nan())
    },
    consistencia: if tool_call_error { None } else {
        Some(consistencia).filter(|&v| !v.is_nan())
    },
    robustez: Some(robustez).filter(|&v| !v.is_nan()),
});
```

The `tool_call_error` variable is already in scope (line 1476:
`let tool_call_error = tool_response.is_none();`).

## Risks

- **Behavior change for `summary.dimension_scores`**: aggregated
  averages per language/tier will now exclude error scenarios from the
  operational dimensions. This is the desired behavior (the prior
  behavior was misleading), but downstream reports built on
  `compute_dimension_averages` may show slightly different numbers.
  Verified: `compute_dimension_averages` (sandbox_core/history.rs:227)
  filters by `if let Some(v) = ...`, so None values are safely
  ignored.
- **G3 health impact**: the fix removes 11 scenarios from the
  verdict-driving set (they move from "failing health 64" to
  "incomplete_5dim"). Predicted G3 status: AMBER
  `insufficient_5dim` (8 scenarios complete, all >= 85; 73 scenarios
  incomplete; verdict cannot be GREEN because coverage is
  incomplete). This is strictly better than RED. No regression.
- **Other consumers of `dimension_scores`**: greps show the field is
  read by `crates/cognicode-sandbox/src/{main.rs,reporting.rs}` and
  the scorecard scripts. All handle `Option<f64>` (the producer type)
  or missing JSON keys (the scorecard, which already filters
  `if v is None`). No fix needed downstream.

## Verification

- `cargo check -p cognicode-sandbox --all-targets`: 0 errors.
- `cargo test -p cognicode-sandbox --lib determine_failure_class_tests`:
  all existing tests still pass (the existing tests cover
  `determine_failure_class`, not the dimension_scores block, but they
  exercise the same code path).
- New tests in a sibling test mod: 4 cases covering
  `tool_call_error=true` (assert dims are None) and
  `tool_call_error=false` (assert dims are Some).
- `python3 sandbox/scripts/release_scorecard.py --runs sandbox/results/...`
  is run AFTER merging this fix and AFTER re-running the affected
  scenarios. The orchestrator (not this cycle) drives that.

## Closure criteria

- 0 new clippy warnings introduced.
- All existing tests in `crates/cognicode-sandbox` continue to pass.
- New regression tests pass.
- The change is documented in `crates/cognicode-sandbox/CHANGELOG.md`
  (if such a file exists) or as a code comment at the change site.
- The corresponding `openspec/specs/release-readiness-gate/spec.md`
  is NOT modified (the spec already says "5-dim health" without
  prescribing what dims to report on errors — the producer's prior
  behavior was a divergence that this fix corrects).

## Note on prior cycle

This is the **third cycle** of the E33-follow-up scorecard-stabilization
track. It is a producer-side complement to cycle 2
(`scorecard-failure-class-coercion`, merged in commit `94c13df4`).
Cycle 2 fixed the read-site to handle dict-shaped failure_class;
cycle 3 fixes the producer to emit None for operational dimensions on
tool errors. Together they unblock the G3 gate for the v1.0.0 cut.

Roadmap-completion initiative: shape C blocker (v1.0.0 cut operational,
G3 health closure).