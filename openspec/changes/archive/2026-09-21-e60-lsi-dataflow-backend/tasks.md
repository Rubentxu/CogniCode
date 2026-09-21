# Tasks — cycle e60 — dataflow backend (M5 adapter)

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

| WU | Content |
|----|---------|
| WU1 | typed `TaintFlowRequest`/`TaintFlowResult`; `dispatch(TAINT_FLOW)` delegates |
| WU2 | pure `DataflowInput` DTO in `domain/findings` (+ `AnalysisInput.dataflow`) |
| WU3 | `TaintFlowRunner` application-local seam + impl for `ProgramAnalysisService` |
| WU4 | `M5DataflowBackend`, `{Dataflow}`, ceiling `B` |
| WU5 | IR/Input → source/sink/untaint sites |
| WU6 | `TaintFlowPath` → `DataflowPath` evidence + `Source → Flow* → Sink` |
| WU7 | counting-spy test: M5 is really invoked, with the expected sites |
| WU8 | E2E with AST + Graph + Dataflow registered; planner picks Dataflow |
| WU9 | fail-loud for capabilities no single backend covers |
| F-1 | IR: reachability rule V10 for `FLOW`/`EXCLUDE` |
| F-2 | `BackendError::Analysis` so engine errors never become "nothing found" |

## Acceptance gate
- `domain::findings` 97; `application::findings` 8; AST E2E 6; Graph E2E 4;
  Dataflow E2E 5.
- `cargo check --workspace --all-targets` 0 errors; fmt clean.
- known-failure baseline unchanged.
