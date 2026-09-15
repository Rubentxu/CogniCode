# Design — cycle e60.1 — semantic contract tightening

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Reachability is owned by `FLOW.source`

```
MATCH  <s>            declared observation        (never a traversal seed)
FLOW   <a> -> <b>     a = authoritative source    b = sink
EXCLUDE <x>           x = untaint site
```

Both backends now agree:

| Backend | source sites | sink sites | untaint sites |
|---------|--------------|-----------|---------------|
| `GraphBackend` | `FLOW.source` | `FLOW.sink` | `EXCLUDE` |
| `M5DataflowBackend` | `FLOW.source` | `FLOW.sink` | `EXCLUDE` |

IR rule **V11** makes the incoherent form impossible to admit:

```
FLOW.source ∉ MATCH subjects  ⇒  DetectorIrError::UndeclaredFlowSource
```

## Limit enforcement (v1, honest scope)

| Limit | Enforced? | Where |
|-------|-----------|-------|
| `max_visited_nodes` | ✅ | statement count before the run |
| `max_visited_edges` | ✅ | DFG edge count after building edges |
| `max_path_count` | ✅ | reported path count after the run |
| `max_result_rows` | ✅ (fallback for path count) | same |
| `time_ms`, `cancellation`, `max_memory_bytes`, `max_depth`, `max_hops` | ❌ not enforced | documented as such |

Exceeding an enforced limit returns `AnalyticsError::LimitExceeded(kind)`;
the adapter maps it to `BackendError::Analysis`, so the executor surfaces
`ExecutionError::Backend`. Never truncate, never degrade to an empty outcome.
