# Design — cycle e58.2 — authority verification & contract tightening

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Promotion pipeline

```
PromotionRequest { detector_id, source, approver }
        │
        ▼  PromotionAuthority::verify(&dyn ApprovalVerifier, request)
   VerifiedPromotion { request, _seal }        (private seal, no serde)
        │
        ▼  DetectorAdmission::promote(&ExecutionPermit, VerifiedPromotion)
   ExecutionPermit (Gated)
```

A caller cannot obtain a `VerifiedPromotion` without running a verifier, and
cannot construct one (private fields + seal). The shipped default,
`RejectAllApprovals`, authorises nothing; `EligibleSourceVerifier` is a
documented non-cryptographic placeholder for M9/M13.

## Restore semantics (fail-closed)

| Path | Result |
|------|--------|
| `restore(record)` | always `Candidate` |
| `restore_with(record, verifier)` | `Gated` **iff** the record claims Gated, records an approval, and the verifier accepts it; otherwise `Candidate` |

A stored `approval: Option<String>` is **never** treated as proof of
authority on its own.

## Digests

```
logic_digest    = sha256(id, requires, steps)
policy_digest   = sha256(policy)
semantic_digest = sha256(logic_digest + policy_digest)
instance_digest = sha256(whole definition incl. name + authority)
```

| Change | logic | policy | semantic | instance |
|--------|-------|--------|----------|----------|
| rename | = | = | = | ≠ |
| Candidate → Gated | = | = | = | ≠ |
| Risk Medium → Critical | = | ≠ | ≠ | ≠ |

## Backend contract additions

```
DetectorExecutor::execute
  1. plan(requires)
  2. backend.run(...)  → DetectorOutcome
  3. every match.kind MUST equal DetectorIr::produced_kind()   → UnexpectedFindingKind
  4. every produced evidence class ≥ ceiling                   → EvidenceCeilingExceeded
  5. persist evidence, assemble, emit ExecutionRecord
```

## IR V9

Every detector must declare ≥1 capability. `requires = {}` is rejected before
capability coverage, so the planner can never hand an unconstrained detector
to an arbitrary backend.

## Removed

`DetectorIr::can_block()` — the raw IR must not answer the gate question.
