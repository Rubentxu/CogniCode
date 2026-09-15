# Design — cycle e62.3 — grounding refs, scope sentinel, evidence allocator (U42, part 2a)

> Cycle: A-lite | Milestone: M6 | Phase: design | Date: 2026-09-15

## D1 — Where the sentinel check goes

Rejecting `SnapshotId::NONE` at construction (`try_new`) *and* at execution is
deliberate redundancy, and the two checks answer different questions:

```text
try_new   → "is this scope well formed?"     (constructor invariant)
execute   → "may this run proceed?"          (trust boundary)
```

The executor check is the one that cannot be bypassed: a caller may build a
scope with the infallible `new` (as fixtures do), but no backend will ever run
under it. A scope that is not pinned to a real snapshot cannot produce a finding
that is verifiable, so refusing it early is the only fail-closed option.

## D2 — Why the store, not the execution, allocates ids

Candidate allocators considered:

| Option | Rejected because |
|--------|------------------|
| counter in `PreparedExecution` | per-execution counter restarts at 1 → collision on the second run in a snapshot |
| caller supplies ids (status quo) | the bridge has no global view; duplicates become `EvidenceIdCollision` at persist time, after the analysis already ran |
| **store sequence per `(ws, snap)`** | chosen: the store already owns uniqueness (it is the only component that can see all evidence in a snapshot) |

`append_batch` also gives **atomicity per execution**, which matters more than it
first appears: writing four atoms with four `add()` calls would leave orphaned
evidence behind if the third failed, evidence that no finding will ever cite.

```text
allocate all ids  →  commit all  →  or commit none
```

This is deliberately *not* a distributed transaction or an abstract
transactional store: a snapshot-local atomic batch buys nearly all of the value
needed now.

## D3 — `GroundingRef` authority model

```text
grounding.fact   = authority
grounding.entity = hint
```

`entity` is stored even though it is not authoritative because it is what makes
a finding *navigable* ("this is a fact about entity 3"). The write bridge will
have to check it against the canonical fact's subject
(`GroundingError::EntityFactMismatch`) rather than trusting it, which is exactly
what `GroundingRef::entity_agrees_with(subject)` expresses: `None` = no opinion,
`Some(true/false)` = a checkable claim.

## D4 — Why grounding lives in the findings DTOs and not the kernel

The kernel already has the canonical `Fact`; what did not exist was the link
*from an analysis projection* back to it. Putting the link on the projection
rather than duplicating fact data keeps one source of truth and makes the
"cannot ground it" case representable (it is `None`, not a fabricated fact).

## D5 — Three orthogonal dimensions (fixed vocabulary)

```text
EvidenceClass   = strength of the analysis        (findings taxonomy, A–D)
Grounding       = connection to canonical truth   (Some/None + EntityId/FactId)
EvidenceGrade   = relation of evidence to fact    (kernel: Supports/Refutes/Corroborates)
```

Blocking will require all three to be right. In particular `EvidenceClass` is
**not** an evidence grade and must not be pushed into the kernel read model; a
valid but partially grounded dataflow route may stay class B while failing the
gate.

## D6 — The next slice's seam (recorded, not built)

```text
Admission → DetectorExecutor::prepare() → PreparedExecution
          → async grounding/persist (application layer)
          → PreparedExecution::finalize() → FindingAssembler → Finding
```

`PreparedExecution` will have private fields so it can only be created by the
executor: the application coordinator gets to do async I/O, nobody gets to skip
admission, planning or the backend-contract checks. `execute()` will be
`prepare() + finalize()` internally so there is exactly one implementation of
the seam.
