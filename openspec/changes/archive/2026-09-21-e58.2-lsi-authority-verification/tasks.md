# Tasks — cycle e58.2 — authority verification & contract tightening

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Work units

### WU-1 (P0) — Verified promotion
- `PromotionRequest`, `ApprovalVerifier`, `RejectAllApprovals`,
  `EligibleSourceVerifier`, `PromotionAuthority`, `VerifiedPromotion`
  (private seal, no serde).
- `DetectorAdmission::promote(&permit, VerifiedPromotion)`.

### WU-2 (P0) — fail-closed restore
- `restore` always `Candidate`.
- `restore_with(record, &dyn ApprovalVerifier)` is the only `Gated` recovery.
- `AdmissionError::{PromotionRejected, PromotionMismatch}`.

### WU-3 (P1) — digest separation
- `logic_digest` / `policy_digest` / `semantic_digest` / `instance_digest`
  + `DetectorDigests` on `DetectorExecutionRef`.

### WU-4 (P1) — backend kind validation
- `DetectorIr::produced_kind()`; executor rejects mismatches with
  `BackendContractViolation::UnexpectedFindingKind`.

### WU-5 (P1) — reject empty `requires`
- V9 requires ≥1 capability for every detector (checked before coverage).

### WU-6 (P1) — remove `DetectorIr::can_block()`

### WU-7 (P1) — correct documentation
- `state.yaml` + e58.1 verification/archive correction notes.

## Acceptance gate (the six mandatory tests)
| Invariant | Test |
|-----------|------|
| JSON `Gated + approval="forged"` cannot restore Gated | `json_record_with_forged_approval_cannot_restore_gated`, `a_persisted_record_cannot_restore_gate_authority_by_default` |
| A caller cannot mint a `VerifiedPromotion` | `promote_requires_a_verified_promotion` (type-sealed; reject-all yields none) |
| Policy change changes semantic, not logic digest | `policy_change_changes_semantic_digest_but_not_logic_digest` |
| Candidate→Gated keeps semantic digest | E2E `promoted_detector_blocks_and_keeps_semantic_identity` |
| Backend kind ≠ PRODUCE ⇒ violation | `backend_cannot_invent_a_finding_kind` |
| `requires={}` cannot fall on AstBackend | `empty_requires_is_rejected` |
