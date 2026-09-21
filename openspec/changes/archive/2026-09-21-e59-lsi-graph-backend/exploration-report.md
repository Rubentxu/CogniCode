# Exploration Report — cycle e59 — graph detector backend

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

e58.2 closed the authority/contract questions. e59 is the first test of
whether the seam generalises: a **second analysis paradigm** (graph) must run
through `admission → plan → backend → outcome → evidence → assembler →
finding → verifier → gate` with **no special case**.

## WU-0 (review follow-up) — promotion was sealed but not bound

`VerifiedPromotion` carried only `(detector_id, source, approver)` and
`promote` compared only the detector id, so a verified approval could be
re-aimed at a different `source`, `version` or logic.

**Resolution:** a `PromotionTarget { detector_id, version, semantic_digest,
source }` derived from the permit. `PromotionRequest::for_permit(&permit,
approver)` is the only public constructor (the caller supplies the approval
intent, never the approved object). `promote` compares the **full target**.

Consequences, all tested:
- same id but a different admission source ⇒ no promotion;
- different version or logic (semantic digest) ⇒ new approval required;
- different policy ⇒ semantic digest changes ⇒ new approval required;
- **renaming does not invalidate an approval** (semantic digest unchanged).

`restore_with` rebuilds the request from the record (`PromotionRequest::
from_record`) so a future verifier can check
`detector + version + semantic_digest + source`.

## WU-1..WU-6 — graph backend

- `GraphInput { nodes, edges }` added to `AnalysisInput`, mirroring `AstInput`.
- `GraphBackend`: `capabilities = {GraphQuery}`, `evidence_ceiling = B`.
- `FLOW source ->* sink` ⇒ BFS (bounded by `max_hops`), deterministic
  (source nodes in id order, neighbours in id order).
- `EXCLUDE path_contains X` ⇒ any path containing a node with subject `X` is
  dropped (diagnostic `path_excluded`).
- Accepted path ⇒ one `GraphPath` evidence (class B) + a `DetectorMatch` with
  a `Source → Flow → Sink` causal chain, every step attributed to evidence.
- The backend still builds **no** `Finding`.

## Deliberately deferred

- `BackendRegistry::plan` still returns a single `&dyn DetectorBackend`. A
  detector needing `{GraphQuery, Dataflow, SymbolicFeasibility}` will require
  `ExecutionPlan<Vec<Stage>>` (recorded in e58.2). `GraphBackend` advertises
  exactly `GraphQuery` and `DataflowBackend` must not over-claim.
- `DetectorMatch.kind` could be dropped entirely and derived from `PRODUCE`;
  the current validation is safe, so this is a cosmetic future refactor.
