# Exploration Report — cycle e62.1 — kernel snapshot correctness (U42 prerequisite)

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

e62 (U42 kernel grounding) needs `EvidenceId → Evidence → FactId → Fact`. While
reviewing the kernel ports, the first prerequisite turned out to be a **real
kernel defect**, not a findings-layer gap.

## Defect — the evidence store is not snapshot-pinned

The kernel states that a pinned read must never mix snapshots, and `FactId` is
canonical **per snapshot** (`1..M` within each). But:

- `InMemoryEvidenceStore` keyed evidence by `(WorkspaceId, FactId)` — no
  snapshot;
- `EvidenceStore::add(&self, ws, e)` had **no snapshot parameter** at all, so a
  pinned write was inexpressible;
- `for_fact(ws, _snap, fact)` **ignored** the snapshot argument;
- its doc comment even claimed the pin was "transitive through the pinned
  fact", which is false when the same numeric fact id exists in two snapshots.

Consequence: with `snapshot A: FactId(1)` and `snapshot B: FactId(1)`, evidence
recorded for B appeared in an A-pinned read.

## Characterization first (WU-0)

A test was written **before** touching anything:

```text
snap A: evidence 101 → fact 1
snap B: evidence 202 → fact 1
for_fact(ws, snap A, fact 1)
   MUST return [101]
   MUST NOT return [202]
```

Against `91b8b54e` it could not even compile (`add` takes 2 arguments), which is
the same defect expressed at the type level: the old API could not pin a write.

## Also missing: point reads

U42 needs `FactStore::get` and `EvidenceStore::get`. Without them the adapter
would have to scan a snapshot's facts and all their evidence to resolve one id
— compensating for a kernel gap in the adapter.

## Fix (WU-1)

- `EvidenceStore::add(ws, snap, e)` and `EvidenceStore::get(ws, snap, id)`.
- `FactStore::get(ws, snap, id)`.
- `InMemoryEvidenceStore` keyed by `(workspace, snapshot, fact)` for the fact
  axis and `(workspace, snapshot, evidence id)` for the id axis.
- `KernelError::EvidenceIdCollision` — re-using an id inside one snapshot is
  rejected atomically, never merged.
- The misleading doc comment is corrected.

The schema is now conceptually:

```text
Facts     (workspace, snapshot, FactId)
Evidence  (workspace, snapshot, EvidenceId) ── FactId
```

## Not in this slice (e62.2)

`GroundingRef` on the analysis DTOs, atomic causal evidence, the
`EvidenceDescriptor` read model and `FindingVerifier` full grounding.
