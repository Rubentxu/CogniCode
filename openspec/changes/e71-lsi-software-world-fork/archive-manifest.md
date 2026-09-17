# Archive Manifest — cycle e71 — SoftwareWorld foundation + fork + world-diff

> Cycle: A-lite (degraded-but-governed) | Milestone: M9 — Fork / Trial / Diff / Promote | Phase: archive | Date: 2026-09-17

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e71 |
| Milestone | M9 — first slice: software-world foundation, fork, world-diff |
| Requirement | ADR-046 (PROPOSED) — software world fork + trial + diff + promote |
| Path | A-lite — degraded-but-governed (DEBT-SDDK-003) |
| Base HEAD | `97ae14a` (e76 WU1) |
| Spec delta | none (the cycle refines the M9 concept at the application-layer seam; no spec delta carried — per AGENTS.md *"requirement ids stay in the umbrella change; evolutivos carry no duplicate spec delta"*) |

## Lifecycle note (honest)

**e71 was never instantiated as a formal SDDK cycle in the ledger.** Same pattern as e67-e70+e74-e75+e76:
three work units authored with deterministic checkpoints; each WU landed in a separate commit.
No `openspec/changes/e71-*/` directory existed at commit time — this commit creates the change
dir + archive-manifest so the on-disk record is complete.

Closure route: **D3-DEFER administrative closure** (per DEBT-SDDK-002 + DEBT-SDDK-003).

This is **NOT** equivalent to a successful normal orchestrator closure. No SDDK cycle row was
created in the ledger, no gate receipts were fabricated, no worker reviews were simulated.

## Commits

| Commit | WU | Summary |
|--------|----|---------|
| `3ae86c47` | WU1 | `feat(e71)` — SoftwareWorld foundation (M9 lineage + isolation, no fact store) — 566 insertions |
| `889f5699` | WU2 | `feat(e71)` — `fork()` — pure operation producing a child SoftwareWorld, parent unchanged — 366 insertions |
| `50e2803d` | WU3 | `feat(e71)` — world-diff wiring — e68 SemanticFactDelta + lineage labels + UAT decisivo — 526 insertions |

## Delivered

### WU1 — SoftwareWorld foundation

`crates/cognicode-core/src/application/software_world/{mod,world,world_tests}.rs`

- `SoftwareWorld` describes **isolation and lineage** of a candidate analysis target.
- Carries NO facts, NO read-set, NO evidence bundle, NO graph store handle (it is not a second
  source of truth).
- The only canonical reference is `SoftwareWorld::base_snapshot: SnapshotId` (bijective with
  `RevisionId`).
- Three concepts, one responsibility each:
  - `World` — isolation / lineage.
  - `Snapshot` — canonical analysed truth.
  - `FactDelta` — comparison of truth (e68 WU1).
- `FactDelta` does the diffing; `SoftwareWorld` does the bookkeeping. Nothing here replaces
  or shadows the kernel.

### WU2 — `fork()` pure operation

`crates/cognicode-core/src/application/software_world/{fork,fork_tests}.rs`

- `parent.fork() -> child` — pure operation.
- Parent is unchanged; child inherits the lineage label + base snapshot.
- `fork` does not write to the kernel; it allocates a new `SoftwareWorldId`.

### WU3 — world-diff wiring

`crates/cognicode-core/src/application/software_world/{diff,diff_tests}.rs`

- Wires `SoftwareWorld` + `SemanticFactDelta` (e68 WU1) into a single coherent surface.
- Lineage labels propagate from the world to the diff.
- UAT decisivo (per the commit message): exercises the end-to-end "two worlds → diff" path
  with realistic fixture inputs.

## Architectural decisions (from the code, not inferred)

1. **No second source of truth.** `SoftwareWorld` is bookkeeping only. The kernel holds
   canonical truth (snapshots, facts, evidence). Worlds reference snapshots; they don't
   fork facts.
2. **Parent unchanged on `fork()`.** This is the seam that lets e72/e73 reason about
   "candidate world vs base world" without race conditions or hidden coupling.
3. **`FactDelta` over `World`, not the other way.** Diffing lives in e68; worlds only
   carry enough metadata to compute the diff (lineage, base snapshot).
4. **No fact store, no read-set, no evidence.** `SoftwareWorld` does not depend on any
   kernel concept; it is below the M6/M7/M8 layer.

## Out of scope (deferred to e72/e73)

- The proposal surface (e72 WU1).
- The trial surface (e72 WU2-WU3).
- Promotion authority (e73).

## M9 status (per the umbrella `state.yaml`)

```text
M9 implementation        CLOSED  (e71 + e72 + e73 code in main; this archive)
M9 technical verification GREEN  (all scoped test suites pass)
M9 strategic decision    PASSED  by explicit user directive 2026-09-17
ADR-046 status           remains PROPOSED until its own acceptance/evidence
                         requirements are satisfied
administrative closure   D3-DEFER for e71/e72/e73
```

The "M9 strategic decision PASSED" reflects a **roadmap/governance decision** by the user.
It MUST NOT be interpreted as satisfying ADR-046's independent acceptance/evidence
requirements. ADR-046 remains PROPOSED until those requirements are separately demonstrated.

## Related cycles

- **e68** (peer): `SemanticFactDelta` — the diffing primitive e71 WU3 wires into.
- **e72** (successor): `ChangeProposal` + `TrialEvidence` + `TrialExecutor`.
- **e73** (successor): `PromotionEvaluation` + `PromotionPermit` + end-to-end promotion gate.
- **e74-e76** (peer): portable runtime / execution / self-hosting — closed before M9 was reconciled.

## Related debt

- `docs/debts/DEBT-SDDK-002.md` — release route blocked; closure via direct push.
- `docs/debts/DEBT-SDDK-003.md` — why this cycle ran in degraded-but-governed mode.
