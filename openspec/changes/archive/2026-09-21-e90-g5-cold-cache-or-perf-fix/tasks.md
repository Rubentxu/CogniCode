# Tasks — Cycle e90 (investigation only)

## WU0 — Baseline verification

```text
HEAD == origin/main == f69c53c9              confirmed
tracked tree                               clean apart from the e90 dir
sddk ledger verify                          not run (investigation only)
```

## WU1 — Reproduce the G5 RED state

- Run `just scorecard-streak` from a clean state and confirm G5 reports
  RED with `analytics: p95=367072ms > budget 5000ms`.
- Capture the scorecard run output to `sandbox/results/scorecard_run.json`.

**Status**: DONE (2026-09-21, current run: streak 0/3 RESET, G5 RED).

## WU2 — Trace the outlier to a specific algorithm

- Run the diagnostic command in `proposal.md` to enumerate the top 3
  `tool_call_ms` per analytics tool.
- Map the outliers to specific lines in
  `crates/cognicode-core/src/application/services/graph_insights.rs`.

**Status**: DONE (2026-09-21):
- `graph_insights` (367072ms) -> `analyze()` line 120 (`CommunityDetector::detect`) + line 145 (`surprising_connections`)
- `graph_communities` (53094ms) -> same `CommunityDetector::detect`

## WU3 — Categorise the failure mode

- Determine: is this a scorecard calibration issue or a product issue?
- Decision criteria: if `tool_call_ms` (the scorecard measurement) is
  the outlier, it is a product issue; if `total_ms - tool_call_ms`
  is the outlier, it is a container-startup / scorecard calibration issue.

**Status**: DONE (2026-09-21): product issue (tool_call_ms dominates).

## WU4 — Enumerate candidate solutions

- Option A (algorithmic), B (caching), C (decomposition), D (NOT recommended).
- Document trade-offs in `proposal.md` § "Candidate solutions".

**Status**: DONE (2026-09-21).

## WU5 — Recommend the next cycle (e91)

- Define WU1..WU5 for `e91-graph-insights-performance`.
- Cross-reference the spec location for the future cycle.

**Status**: DONE (2026-09-21, in `proposal.md`).

## Closure semantics

```text
investigation  COMPLETE — root cause identified and documented
implementation DEFERRED — to e91
spec           COMPLETE — REQ-G5-INVESTIGATION-01..05 verified
scorecard      unchanged — still RED, but honestly reporting
release        n/a — no code change to release
archive        DONE — via this cycle
```

## Carry-forward

e91-graph-insights-performance is **the next concrete cycle** that can
move the v1.0.0 readiness needle. Its work units are:

- WU1 — Profile `graph_insights` on a captured multi-repo fixture
- WU2 — Implement bounded-iteration `CommunityDetector::detect`
- WU3 — Add `graph_insights_multi_repo_under_budget` regression test
- WU4 — Re-run scorecard, expect G5 GREEN
- WU5 — Sync the latency budget to `openspec/specs/cognicode-analytics/spec.md`
