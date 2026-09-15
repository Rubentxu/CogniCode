# Exploration Report — cycle e60.1 — semantic contract tightening

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

Review of e60 found two real gaps before the AST/Graph/Dataflow contract can
be called frozen.

## Gap 1 — `MATCH + FLOW` meant different things per backend

`M5DataflowBackend` built its M5 `sources` as `MATCH subjects ∪ {FLOW.source}`,
while `GraphBackend` used only `FLOW.source`. So the same IR had different
semantics depending on which backend ran it, and a path originating from an
*unrelated* `MATCH` observation could be reported with a message claiming the
`FLOW.source` reached the sink — incoherent evidence.

**Resolution (C1/C2):**
- `FLOW.source` is the **single authoritative reachability source**. `MATCH`
  subjects are declared observations and never seed a traversal.
- IR rule **V11**: every `FLOW.source` must be declared by a `MATCH`, else
  `DetectorIrError::UndeclaredFlowSource { index }` — fail loud instead of
  leaving the ambiguity.
- e60's design document is corrected (C6).

## Gap 2 — `PlanLimits` were carried but not enforced

`M5DataflowBackend` built explicit limits and passed them to the runner, but
`ProgramAnalysisService::taint_flow` ignored them (`_limits`), so the adapter
only *appeared* to bound cost.

**Resolution (C4/C5):** enforce the cheaply checkable, deterministic limits and
**error** (never truncate):
- `max_visited_nodes` (statement count) and `max_visited_edges` (DFG edge
  count) before the run;
- `max_path_count`, else `max_result_rows`, on the reported path count after
  the run.

`time_ms`, `cancellation`, `max_memory_bytes`, `max_depth`, `max_hops` are
**not** enforced by this v1 facade and are now documented as such rather than
silently assumed.

An exceeded limit propagates
`AnalyticsError::LimitExceeded → BackendError::Analysis → ExecutionError::Backend`
— never a truncated result and never "no findings".
