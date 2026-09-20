# Archive of scorecard-failure-class-coercion

This folder was moved from `openspec/changes/scorecard-failure-class-coercion/`
on 2026-09-20 (ISO).

Original location: `openspec/changes/scorecard-failure-class-coercion/`
Archived by: orchestrator (roadmap-completion initiative)
Archive date: 2026-09-20
Cycle: shape C blocker (v1.0.0 cut operational, Gate 2 unlock)

## Implementation evidence

- Commit: `94c13df4 fix(scorecard): coerce dict-shaped failure_class to flat string key`
- Author: orchestrator
- Date: 2026-09-20

## Scope recap

Single-file production fix in `sandbox/scripts/release_scorecard.py` plus a
new regression test file. The bug: `_aggregate_results()` used
`r['failure_class']` directly as a dict key for `failure_distribution`.
Whenever any `result.json` in `sandbox/results/{ci_smoke,quality,full_run}`
carried a dict-shaped `failure_class` (produced by
`cognicode-sandbox::determine_failure_class` when outcome == "mcp_error"),
the scorecard aborted at `gate_g5()` with `TypeError: cannot use 'dict' as a
dict key (unhashable type: 'dict')`. This was the silent blocker behind the
E31-G scorecard streak being stuck at `current_streak: 1/3` despite 10
history runs.

The fix adds a defensive helper `_normalize_failure_class(fc)` that
flattens dict shapes to canonical strings. Read-site fix only; producer-side
flattening is deferred to `sandbox-failure-class-flatten-bd` (out of scope
because the read-site fix is sufficient to unblock the streak).

## Verification

- 10/10 pytest in `sandbox/scripts/tests/test_failure_class_coercion.py`
- `release_scorecard.py` compiles clean (`python3 -m py_compile`)
- Scorecard run completes end-to-end on the live sandbox fixture:
  13 gates evaluated (was aborting at gate_g5 with TypeError). Result: 8 GREEN
  / 3 AMBER / 2 RED. The 2 RED are G3 (health 54.9 < 85 budget) and G5
  (analytics p95 367072ms > 5000ms budget) — pre-existing operational debt,
  not introduced by this fix.

## Delta specs

None — this is a `fix(scorecard)` cycle. No durable spec additions.

## Knowledge graph

- Documented failure mode: dict-shaped `failure_class` aborts the scorecard
  silently (TypeError, not a Python exception with a clean message).
- Documented invariant: `failure_class` MUST be hashable string-equivalent
  for `_aggregate_results` to function. Documented in
  `_normalize_failure_class` docstring.
- Producer-side follow-up queued: `sandbox-failure-class-flatten-bd`.

## Carry-forward

- **G3 health RED (54.9 < 85)** requires promoting more scenarios to
  `complete_5dim` (currently 19/81). Multi-cycle Tier-1 fill work
  (shape B).
- **G5 analytics latency RED (367072ms)** requires either Tier-B corpora
  exercise or latency budget recalibration per ADR candidate. Out of scope
  for this cycle.
- **Streak counter** still at 1/3 — needs ALL-GREEN runs (no RED, no
  non-whitelisted AMBER) to advance. The next streak run after this fix
  will reset to 0 because of G3/G5 RED, but the scorecard will now actually
  complete (no TypeError), so the operator can drive forward.