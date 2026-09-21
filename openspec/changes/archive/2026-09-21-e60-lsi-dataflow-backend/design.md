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
| `MATCH <s>` | declared observation — **never** a source |
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

> **Correction (cycle e60.1).** This design originally documented
> `MATCH <s> → sources`, which was wrong: it made the same IR mean different
> things in Graph vs Dataflow, and could report an unrelated observation as
> having reached the sink. `FLOW.source` is now the **single authoritative
> reachability source** (IR rule V11 requires it to be declared by a `MATCH`).
> Also: `PlanLimits` are now actually enforced (`max_visited_nodes`,
> `max_visited_edges`, `max_path_count`/`max_result_rows`), erroring rather
> than truncating; the rest are explicitly documented as not enforced.

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
