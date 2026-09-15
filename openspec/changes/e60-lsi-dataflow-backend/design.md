# Design — cycle e60 — dataflow backend (M5 adapter)

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Layers

```
DOMAIN
  DetectorBackend (port)     DataflowInput (pure DTO, no M5 types)
        ▲
APPLICATION
  M5DataflowBackend<R: TaintFlowRunner>
        │  IR → TaintFlowRequest
        ▼
  ProgramAnalysisService::taint_flow(&TaintFlowRequest, &PlanLimits)
        │            ▲
        │            └── dispatch(TAINT_FLOW) delegates here (one impl, two presentations)
        ▼
  TaintFlowResult ⟵ taint_forward (cognicode-graph-algos, M5)
        │
        ▼
  DetectorOutcome (DataflowPath evidence + Source→Flow*→Sink causal)
```

`DetectorBackend` **is** the port: no `IDataflowService`, no second taint
engine. `TaintFlowRunner` is an application-local composition/test seam only.

## IR → engine mapping

| IR | M5 request field |
|----|------------------|
| `MATCH <s>` | `sources` |
| `FLOW <a> -> <b>` | `a` → `sources`, `b` → `sinks` |
| `EXCLUDE <x>` | `x` → `untaints` |

Reachability is **not** recomputed in M6: whatever M5 returns is reported, so
a path M5 eliminated cannot reappear.

## Capabilities

| Backend | capabilities | ceiling |
|---------|--------------|---------|
| AstBackend | `{AstPattern}` | C |
| GraphBackend | `{GraphQuery}` | B |
| M5DataflowBackend | `{Dataflow}` | B |

The capability describes what the backend **offers to the Detector IR**, not
the techniques it uses internally (M5DataflowBackend builds a DFG, but that is
not a `GraphQuery` offer).

## IR reachability rule (V10)

```
FLOW / EXCLUDE present  ⇒  requires ∩ {GRAPH_QUERY, DATAFLOW} ≠ ∅
otherwise               ⇒  DetectorIrError::MissingReachabilityCapability
```

## Error handling

```
AnalyticsError → BackendError::Analysis → ExecutionError::Backend
```

An engine error never degrades into an empty `DetectorOutcome`.
