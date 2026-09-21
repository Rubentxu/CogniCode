# Exploration Report — cycle e59.1 — graph + M5 taint correctness

> Cycle: A-lite | Milestone: M6 (entry to e60) | Phase: explore | Date: 2026-09-15

## Trigger

Review of e59 found one semantic gap in `GraphBackend` and asked for two M5
taint characterizations before any dataflow adapter imports `TaintPath` as
class-B evidence.

## G-1 — Graph: exclusion was applied *after* path selection

`find_paths` kept one BFS predecessor per node, so it produced a single
witness per sink and *then* discarded it if a node was excluded. When a
sanitized route and a clean route both exist, the arbitrary witness could be
the sanitized one, hiding the valid alternate path.

**Resolution:** exclusion is enforced **during** traversal (excluded nodes are
never entered), so the search returns the shortest *valid* witness. The old
post-hoc filter is gone; when no valid witness exists but unconstrained paths
do, a `path_excluded` diagnostic explains it.

## G-2 — Graph: multiple `FLOW` steps ran only the last one

The step scan used successive `flow = Some(...)` assignments, silently
executing only the final `FLOW`.

**Resolution:** `0 FLOW` → `no_flow_steps` diagnostic; `1 FLOW` → run;
`>1 FLOW` → `BackendError::UnsupportedIr` (fail loud until multi-flow
semantics are defined).

## T1/T2 — M5 taint characterization (both FAILED ⇒ fixed in M5)

Characterization tests were written **before** touching the engine
(`crates/cognicode-graph-algos/tests/taint_characterization.rs`).

**T1 — alternate sanitized path.** `intermediates_on_chain` reconstructed the
witness over the full successor graph, ignoring `untainted`. So the reported
causal chain could route **through a sanitizer** while the taint actually
arrived via a clean path. For M6 this matters: a witness that is not the
real causal chain would be imported as class-B evidence.

**T2 — unequal-depth multi-source merge.** Propagation re-enqueued a node only
when it was tainted for the *first* time. A later-arriving origin was recorded
at the merge node but never propagated to its descendants, so
`(sourceB, sink)` was silently missing.

Both tests failed against `main`, confirming two real M5 defects.

## Decision

Per the agreed discipline — *M6 does not repair or reimplement analysis; M6
characterizes M5, and if M5 is wrong we fix M5* — both defects were fixed in
`cognicode-graph-algos::algorithms::taint_forward`:

1. re-enqueue a node whenever it gains a **new** origin (bounded: at most
   `|statements| × |sources|` enqueues);
2. reconstruct the witness only over nodes this origin actually tainted.

No change was made to M6 for either defect.
