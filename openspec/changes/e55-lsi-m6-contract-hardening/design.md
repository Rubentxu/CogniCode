# Design — cycle e55 — M6 contract hardening

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Three orthogonal concepts

```
AnalysisCapability   — WHAT the detector needs       (a SET)
EscalationTier       — WHAT it costs to provide it   (linear cost)
DetectorAuthority    — WHETHER it may block          (gate)
```

`AnalysisCapability` is **not** a ladder. `LlmReasoning` does not imply
`SymbolicFeasibility`; a detector must declare each capability it needs.
`EscalationTier` is derived (`max(tier(c) for c in requires)`) and is used
only for planning/cost.

## Capability floors (validator V7)

| Step | Requires |
|------|----------|
| `MATCH` | — |
| `FLOW`, `EXCLUDE` | `GraphQuery` |
| `VERIFY{feasible_path:true}` | `SymbolicFeasibility` |
| `VERIFY{feasible_path:false}` | — |
| `PRODUCE` | — |

A step whose floor is not covered by `requires` fails admission with
`DetectorIrError::MissingCapability{capability, index}` — the "unsupported
construct fails loud" requirement, now expressed as capabilities rather
than a linear level.

## Execution authority (P0)

```
DetectorIr.authority = Candidate
        │
        │ DetectorExecutionRef::from_definition(&ir, version, exec_id)
        ▼
DetectorExecutionRef { id, version, authority_at_execution=Candidate,
                       detector_digest, execution_id }
        │
        │ Finding.detector
        ▼
Finding::can_block(gate)  ──►  requires detector.authority_at_execution.can_block()
```

Capturing authority **at execution** is the whole point: promoting a
detector from `Candidate` to `Gated` later must not retroactively make its
past findings blocking.

`detector_digest` is the fnv1a64 digest of the canonical serialization of
the definition, so a finding is tied to the exact detector *content*.

## Status disposition

```
Open ──► Accepted ──► Fixed
   │         │
   │         ├──► FalsePositive   (never real)
   │         ├──► RiskAccepted    (real, risk accepted)
   │         └──► Suppressed      (policy exemption)
```

Only `Open`/`Accepted` participate in gates. Legacy mapping:
`wontfix ⇒ RiskAccepted`, `suppressed|ignored|exempt ⇒ Suppressed`,
`false_positive ⇒ FalsePositive`.

## Namespaced identifiers

`namespaced::NamespacedName` is the single validator (`namespace.name`,
non-empty segments) behind `SubjectPattern`, `FindingKind`, `DetectorId`.

**Tracked divergence**: the kernel `RelationKind` still uses `ns:name`
(colon). It is feature-gated and has its own fixtures; unifying it is a
follow-up (see the ledger note). Storage may translate internally.

## Deferred to e56

- Extract `EntityId/FactId/EvidenceId/SnapshotId/ExecutionId` into an
  ungated `domain::kernel_ids`; make `EvidenceRef = EvidenceId`.
- Structured `CausalStep` (kind + optional EntityId/FactId/EvidenceId).

Both touch the feature-gated kernel and are isolated to keep e55 bounded.

## Rollback

Revert `domain/findings/*` and the umbrella doc edits.
