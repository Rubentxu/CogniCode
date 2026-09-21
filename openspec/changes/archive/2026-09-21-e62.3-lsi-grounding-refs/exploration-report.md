# Exploration Report — cycle e62.3 — grounding refs, scope sentinel, evidence allocator (U42, part 2a)

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

e62.2 pinned an execution to its `AnalysisScope`. The reviewer closed the cycle
with two points that had to be settled **before** the write bridge could be
built:

1. **A hole in the new pin.** `AnalysisScope::new()` accepted
   `SnapshotId::NONE`, which the kernel declares as the invalid sentinel. An
   execution with `Some(scope)` whose snapshot was `NONE` satisfied
   `MissingScope` while being pinned to nothing.
2. **No evidence allocator existed.** The kernel's `EvidenceStore::add` takes an
   `Evidence` with an id the *caller* chose and rejects duplicates with
   `EvidenceIdCollision`. Every id seen in the tree was hand-assigned in
   fixtures. A write bridge cannot assign `EvidenceId(1..n)` per execution: the
   second execution in the same snapshot would collide.

The reviewer's rule for grounding was also fixed here, because it decides the
shape of the DTO:

```text
GroundingRef.fact   = authority (canonical fact)
GroundingRef.entity = hint     (navigation; never trusted over the fact)
```

## What was delivered (WU0 + WU1)

### WU0a — the invalid-snapshot sentinel is refused

- `AnalysisScope::try_new(ws, snap) -> Result<Self, AnalysisScopeError>`
  returning `AnalysisScopeError::InvalidSnapshot` for `SnapshotId::NONE`.
- `AnalysisScope::is_valid()`.
- `DetectorExecutor::execute` refuses the sentinel **before** planning or
  running any backend, with the new `ExecutionError::InvalidScope`.

`AnalysisScope::new` stays infallible (it is the internal/fixture constructor)
and now documents that `try_new` is the trust-boundary entry point.

### WU0b — the store allocates evidence ids, atomically

```rust
pub struct NewEvidence { fact: FactId, grade: EvidenceGrade, provenance: ProvenanceRecord }

async fn append_batch(&self, ws, snap, Vec<NewEvidence>) -> Result<Vec<EvidenceId>, KernelError>;
```

- Ids are allocated by the store from a **per-`(workspace, snapshot)`**
  sequence, so two executions in the same snapshot can never collide, and two
  snapshots cannot steal each other's numbers.
- The batch is **atomic**: every id is allocated first, then every record is
  committed; a failure mid-batch cannot leave orphaned evidence behind.
- `EvidenceStore::add` (explicit id) keeps the allocator ahead of the id it was
  given, so a later `append_batch` cannot re-use it.

### WU1 — `GroundingRef`, additive

```rust
pub struct GroundingRef { entity: Option<EntityId>, fact: FactId }
```

carried as `Option<GroundingRef>` on:

| DTO | Why it needs grounding |
|-----|------------------------|
| `AstConstruct` | the construct was projected from a fact |
| `GraphNode` | the endpoint exists in the graph view |
| `GraphEdge` | **the relation** — reachability is proved by traversing edges, so nodes alone only show the endpoints exist |
| `DataflowStatement` | may be synthesised from several observations, so it is frequently ungrounded |

The field is `Option` precisely so a backend cannot be tempted to pick a
convenient fact: an element that cannot be grounded may be explained but cannot
open the gate (`correct incomplete > fabricated complete`).

WU1 is **additive**: no backend reads grounding yet, no analysis behaviour
changed. Population into `ProducedEvidence` and the atomic causal-evidence model
is the next slice.

## Deferred to the next slice (explicitly not started)

- WU2 atomic causal evidence + `EvidenceBindings` (retiring the "one evidence
  id per path" invariant).
- WU3 `DetectorExecutor::prepare()` / `PreparedExecution::finalize()`.
- WU4 async canonical write bridge (why: **no `block_on` in the domain**).
- WU5 `KernelEvidenceReadModel::load()`.
- WU6 the full coherence verifier and the three adversarial UATs.
- Still deferred from e62.2: `ExecutionPlan<Vec<Stage>>`, real tree-sitter
  extractor, threading `LegacyRuleProvenance` into `Finding`.
