# Exploration Report — cycle e58.2 — authority verification & contract tightening

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

Review of e58.1 found a real P0 and three P1s.

## P0 — `restore` was fail-open for a forged approval

`AdmittedDetectorRecord.authority` and `admission.approval: Option<String>`
are both serializable, and `restore` treated `approval.is_some()` as proof of
authority. A hand-written record `{authority: gated, approval: "forged"}`
therefore restored as a `Gated` permit. The e58.1 test only covered
`Gated + approval=None`.

**Resolution:**
- `restore` is now **fail-closed**: it always yields `Candidate`.
- Promotion requires a `VerifiedPromotion` — private fields + private seal +
  no serde — minted only by `PromotionAuthority::verify(&dyn ApprovalVerifier,
  PromotionRequest)`. `DetectorAdmission::promote` accepts only that type.
- `restore_with(record, &dyn ApprovalVerifier)` is the only path that can
  recover `Gated`, and only when the verifier accepts the recorded approval.
- The shipped default verifier, `RejectAllApprovals`, authorises nothing;
  `EligibleSourceVerifier` is a documented placeholder (non-cryptographic)
  for M9/M13.
- `AdmissionSource::is_trusted_to_gate` is now actually consumed (by the
  placeholder verifier) instead of being dead.

## P1 — `semantic_digest` ignored the policy

`DetectorFindingPolicy` governs the severity/risk that reach the gate, but
`semantic_digest` only covered `(id, requires, steps)`, so `Low` → `Critical`
kept the same digest.

**Resolution — explicit separation:**
```
logic_digest    = (id, requires, steps)
policy_digest   = policy
semantic_digest = logic_digest + policy_digest
instance_digest = whole definition (incl. name + authority)
```
`DetectorExecutionRef` now carries all four in `DetectorDigests`.

## P1 — a backend could invent the `FindingKind`

`DetectorMatch.kind` was copied into the finding without checking it against
the detector's single `PRODUCE` kind.

**Resolution:** the executor validates every match kind against
`DetectorIr::produced_kind()` →
`BackendContractViolation::UnexpectedFindingKind { expected, produced }`.

## P1 — `requires = {}` was still legal

e58.1 allowed an empty `requires` for PRODUCE-only detectors, but the planner's
subset test then made such a detector eligible for *any* backend.

**Resolution:** V9 now requires at least one capability for **every** detector
(`DetectorIrError::NoDeclaredCapability`), checked before capability coverage.
An `Aggregation` capability will lift this when aggregation is implemented.

## P1 — `DetectorIr::can_block()` removed

After making authority an admission-boundary concern, the raw IR must not
answer "can I block?". Removed; the gate lives in
`ExecutionPermit → DetectorExecutionRef → FindingVerifier → FindingGate`.

## P1 — documentation corrected

`state.yaml` and the e58.1 reports claimed "authority unforgeable by
construction". Corrected to the precise statement: the in-memory capability
cannot be forged or deserialized, but **persisted promotion authenticity is
pending trusted approval verification (M9/M13)**.

## Note for e60 (recorded, not implemented)

`BackendRegistry::plan` selects a single backend whose capability set is a
superset of `requires`. A future detector needing `{GraphQuery, Dataflow,
SymbolicFeasibility}` would need an `ExecutionPlan<Vec<Stage>>` rather than
one `&dyn DetectorBackend`. Deferred until the real GraphBackend is in place;
do **not** make DataflowBackend advertise `GraphQuery` to win planning.
