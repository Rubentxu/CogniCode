# Design — cycle e56 — kernel ids + structured causal

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Id ownership

```
domain::kernel_ids   (UNGATED)   EntityId OccurrenceId SnapshotId FactId
                                 EvidenceId StableEntityId ExecutionId
        ▲
        │ pub use ...::*
domain::evidence_kernel::ids     (gated shim — path compatibility only)
```

`kernel_ids` is the single source of truth. `evidence_kernel::ids` re-exports
it, so `evidence_kernel::ids::FactId` and `domain::kernel_ids::FactId` are
literally the same type. Storage/persistence stays feature-gated; only the
vocabulary moves out.

Lifting the ids to the default build is an **additive** surface change
(previously reachable only with `--features evidence-kernel`).

## Structured causal lineage

```
CausalStep {
    kind:     CausalStepKind,      // Source|Flow|Call|Sanitizer|Guard|Sink
                                   // |RuntimeObservation|Verification|Location
    subject:  Option<EntityId>,    // navigable
    fact:     Option<FactId>,      // navigable
    evidence: Option<EvidenceId>,  // navigable
    detail:   String,              // human-readable, non-empty
}
```

Builders `with_subject` / `with_fact` / `with_evidence` keep construction
readable. `is_explainable` keeps requiring a non-empty detail per step.

Consumers can now resolve: finding → causal step → subject entity/fact/
evidence, instead of printing strings. The legacy projection still emits a
single `Location` step (no entity/fact/evidence available).

## Rollback

`git mv` back, restore the flat `CausalStep`, drop `ExecutionId` from
`kernel_ids`.
