# Design — cycle e59.1 — graph + M5 taint correctness

> Cycle: A-lite | Milestone: M6 (entry to e60) | Date: 2026-09-15

## Graph: constraint-aware witness

```
BEFORE
  BFS (ignore excludes)  → one arbitrary witness → discard if excluded
  ⇒ a valid alternate path could be hidden

AFTER
  BFS with excluded nodes never entered → shortest VALID witness
  ⇒ if no witness exists but unconstrained paths do: `path_excluded` diagnostic
```

Multi-FLOW:

| FLOW steps | behaviour |
|-----------|-----------|
| 0 | `no_flow_steps` diagnostic |
| 1 | run |
| >1 | `BackendError::UnsupportedIr` (fail loud) |

## M5 taint fixes

```
T1  witness reconstruction constrained to nodes the origin actually reached
      (previously: full succ graph ⇒ witness could cross an untainted sanitizer)

T2  a node is re-enqueued whenever it gains a NEW origin
      (previously: only on the first taint ⇒ later origins never propagated)
```

Termination (T2): each enqueue adds at least one new `(statement, origin)`
pair, bounded by `|statements| × |sources|`.

Both fixes are in `cognicode-graph-algos::algorithms::taint_forward`; **no M6
code changed for them**, preserving the M5/M6 boundary (M6 imports analysis
primitives, it does not reimplement them).
