# Design — cycle e62.1 — kernel snapshot correctness

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Keying

```
BEFORE (wrong)
  Evidence: (workspace, FactId)              ← snapshot lost
  add(ws, e)                                 ← unpinnable write
  for_fact(ws, _snap, fact)                  ← snapshot ignored

AFTER
  Facts:    (workspace, SnapshotId) → Vec<Fact>          (unchanged, already pinned)
  Evidence: (workspace, SnapshotId, FactId)     → Vec<Evidence>
            (workspace, SnapshotId, EvidenceId) → Evidence
  add(ws, snap, e), get(ws, snap, id), for_fact(ws, snap, fact)
```

## Why the snapshot must be on the id axis

`FactId` (and therefore `EvidenceId`) is canonical *per snapshot*: batch ids
start at 1 within each snapshot, and the kernel explicitly allows re-using the
id space in a different snapshot. Two snapshots therefore legitimately hold
`fact 1`, each with its own evidence. A key without the snapshot silently
merges them — precisely the "historical read remains stable" invariant the
kernel claims.

## Collisions

`EvidenceIdCollision(EvidenceId, SnapshotId)` is returned when `add` re-uses an
id already present in the target snapshot. The write is atomic: neither the
fact axis nor the id axis is touched on rejection, so a failed `add` leaves no
partial state (mirrors `FactStore::commit`'s batch semantics).
