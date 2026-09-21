# Exploration Report — cycle e60 — dataflow backend (M5 adapter)

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

e59.1 left M5 taint producing *true* causal witnesses, so a dataflow backend
can safely import `TaintPath` as class-B evidence. The goal is the third
analysis paradigm through the **unchanged** M6 core.

## Boundary (frozen)

```
domain/findings          DetectorBackend (the port), DataflowInput (pure DTO)
        ▲
        │ implements
application/findings     M5DataflowBackend → ProgramAnalysisService → taint_flow
                                                                        ↓
                                                  cognicode-graph-algos (M5)
```

`DataflowInput` knows nothing about `TaintPath`, `Statement`, `RunOutput`,
`AlgorithmId` or M5 JSON. The IR→engine translation lives in the adapter.

## Findings during implementation

### F-1 — the IR floor for `FLOW`/`EXCLUDE` was wrong

`DetectorStep::required_capabilities()` demanded `GraphQuery` for every
`FLOW`, so a dataflow detector declaring `{Dataflow}` failed admission with
`MissingCapability { GraphQuery }`.

`FLOW`/`EXCLUDE` are **reachability** constructs: they are satisfied by a
graph engine (`GraphQuery`) *or* a dataflow engine (`Dataflow`). Fixed with a
dedicated rule (V10): a detector with a reachability step must declare at
least one of those two, else
`DetectorIrError::MissingReachabilityCapability { index }`. `required_capabilities`
no longer lists `FLOW`/`EXCLUDE` (their "floor" is a disjunction).

### F-2 — `dispatch(TAINT_FLOW)` had its own copy of the algorithm call

Rather than have the adapter parse the JSON `RunOutput`, e60 adds a typed
`ProgramAnalysisService::taint_flow(&TaintFlowRequest, &PlanLimits) ->
TaintFlowResult` and makes `dispatch(TAINT_FLOW)` **delegate** to it: one
implementation, two presentations (typed for M6, JSON for MCP/API/CLI).

## What was built

- `TaintFlowRequest` / `TaintFlowResult` / `TaintFlowPath` / `TaintFlowStatement`
  typed facade (application).
- `DataflowInput` / `DataflowFunction` / `DataflowStatement` / `DataflowLocation`
  pure domain DTO; `AnalysisInput.dataflow`.
- `TaintFlowRunner` — a tiny **application-local** seam (not a domain port,
  not infrastructure) so the adapter can be composed and spied on.
- `M5DataflowBackend<R: TaintFlowRunner>`: `capabilities = {Dataflow}`,
  `evidence_ceiling = B`; maps `MATCH`/`FLOW source` → sources, `FLOW sink` →
  sinks, `EXCLUDE` → untaints; emits `DataflowPath` evidence plus a
  `Source → Flow* → Sink` causal chain.
- Engine errors stay errors: `AnalyticsError → BackendError::Analysis →
  ExecutionError::Backend`, never an empty outcome.

## Deliberately deferred

`ExecutionPlan<Vec<Stage>>` — first prove AST / Graph / Dataflow as
*individual* backends (done), then design multi-stage plans from real
evidence. A detector needing capabilities no single backend offers fails loud.
