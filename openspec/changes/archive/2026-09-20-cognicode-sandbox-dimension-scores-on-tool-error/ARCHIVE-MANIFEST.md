# Archive of cognicode-sandbox-dimension-scores-on-tool-error

This folder was moved from `openspec/changes/cognicode-sandbox-dimension-scores-on-tool-error/`
on 2026-09-20 (ISO).

Original location: `openspec/changes/cognicode-sandbox-dimension-scores-on-tool-error/`
Archived by: orchestrator (roadmap-completion initiative)
Archive date: 2026-09-20
Cycle: shape C blocker (v1.0.0 cut operational, G3 health closure — producer side)

## Implementation evidence

- Commit: `61e62851 fix(sandbox): emit None for operational dims when tool call errored`
- Author: orchestrator
- Date: 2026-09-20

## Scope recap

Single-file producer-side fix in `crates/cognicode-sandbox/src/main.rs`
(the `dimension_scores` block at lines 1530-1557) plus a new
regression-test mod with 5 tests.

The producer always populated `latencia`, `escalabilidad`, `consistencia`
whenever timing data was available, regardless of whether the tool call
itself succeeded. This produced inconsistent scenario records:
`correctitud=0` (correctly flagging the failure) but the operational
dims near 100 (because the call was fast-failing). The scorecard's
5-dim weighted average then read ~64 — below the 85/100 G3 threshold —
reporting "failing health" when the scenario was actually "tool failed,
dimensions not measurable" (a different category).

The fix gates lat/esc/con on `tool_call_error`:
```rust
latencia: if tool_call_error { None } else { Some(latencia).filter(|&v| !v.is_nan()) },
escalabilidad: if tool_call_error { None } else { Some(escalabilidad).filter(|&v| !v.is_nan()) },
consistencia: if tool_call_error { None } else { Some(consistencia).filter(|&v| !v.is_nan()) },
```

`robustez` is left as-is (it already handles errors via
`compute_robustness_score`, returning 0 when total operations includes
an error). `correctitud` is set by the caller (`score_scenario`) and
already returns None when ground_truth is absent.

## Verification

- `cargo check -p cognicode-sandbox --all-targets`: 0 errors.
- `cargo test -p cognicode-sandbox --bin sandbox-orchestrator`:
  177/177 PASS (5 new + 172 pre-existing).
- Workspace check: clean.

## Delta specs

None — this is a `fix(sandbox)` cycle. No durable spec additions.

## Knowledge graph

- **Bug class**: producer/consumer semantic inconsistency where the
  consumer (scorecard) interprets a serialised field by one contract
  (lat/esc/con are measurable even on tool failure) while the producer
  treats the field by another (the tool's success state is the gating
  condition). The fix gates the producer's serialisation on the same
  predicate the consumer requires.

- **Pattern**: when a producer always populates a field that the
  consumer keys on, the producer must either (a) populate consistently
  with the consumer's interpretation or (b) leave it null and rely on
  the consumer's missing-handling path. The original producer did
  neither correctly for lat/esc/con on tool failure — it populated as
  if (a) but the consumer expected (b). This cycle adopts (b).

- **G3 impact**: the 11 failing scenarios (all `mcp_error` with the
  buggy pattern `correctitud=0` + lat/esc/con=100) will move from
  "failing health ~64" to "incomplete_5dim" after this fix is in
  effect AND the affected scenarios are re-run. The re-run is NOT
  part of this cycle (orchestrator-driven in a follow-up).

## Carry-forward

- **Re-run the 11 affected scenarios** so their `result.json` files
  reflect the new producer shape. This is a sandbox-setup + run
  operation, not a code change. After re-run, G3 should drop from
  RED to AMBER `insufficient_5dim`.

- **G3 mixed-repeats edge case** in `release_scorecard.py`: the
  implementation excludes scenarios where ANY repeat is incomplete,
  contradicting its own docstring (line ~503). Verified via simulation
  that this divergence does NOT affect the current
  ci_smoke+quality+full_run subset (no mixed scenarios there). Tracked
  separately; would unlock more coverage if/when the corpus grows.

- **G4 corpus unverified (81 candidates, AMBER)**: not addressed by
  this cycle. Shape B (Tier-1 fill) is the canonical work item.

- **G6 insufficient repeats (AMBER)**: requires `--repeat >= 3` in
  the scorecard run, plus fresh corpus data. Not addressed by this
  cycle.

- **G5 analytics latency RED (367072ms > 5000ms budget)**: requires
  either Tier-B corpora exercise or latency budget recalibration per
  ADR candidate. Out of scope for this cycle.