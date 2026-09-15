# Verification Report — cycle e58.2 — authority verification & contract tightening

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e58.2 — authority verification & contract tightening |
| Path | A-lite |
| Base HEAD | `1b2ab25b` (post-e58.1) |
| Verify verdict | **PASS** |

## Verification

### V-1 — Domain + E2E tests

```
cargo test -p cognicode-core --lib domain::findings   # 86 passed
cargo test -p cognicode-core --test findings_ast_e2e  # 6 passed
```

### V-2 — The six mandatory invariants

| Invariant | Evidence |
|-----------|----------|
| JSON `Gated + approval="forged"` cannot restore Gated | `json_record_with_forged_approval_cannot_restore_gated`; E2E `a_persisted_record_cannot_restore_gate_authority_by_default` — default restore and `restore_with(RejectAllApprovals)` both yield `Candidate` |
| A caller cannot mint a `VerifiedPromotion` | `promote_requires_a_verified_promotion`: the reject-all verifier yields `PromotionRejected`; `VerifiedPromotion` has private fields/seal and no serde |
| Policy change changes semantic, not logic digest | `policy_change_changes_semantic_digest_but_not_logic_digest` |
| Candidate→Gated keeps the semantic digest | E2E `promoted_detector_blocks_and_keeps_semantic_identity` (+ logic equal, instance different) |
| Backend kind ≠ PRODUCE ⇒ contract violation | `backend_cannot_invent_a_finding_kind` |
| `requires={}` cannot fall on AstBackend | `empty_requires_is_rejected` (V9, checked before coverage) |

### V-3 — Build / purity / format / kernel / checker

- `cargo check --workspace` → 0 errors.
- `cargo fmt -p cognicode-core --check` → clean.
- Domain purity: no I/O imports in `domain/findings/`.
- Gated kernel: 82 `evidence_kernel` tests green.
- `scripts/check_known_failures.py` → exit 0 (41 baseline).

### V-4 — Documentation corrected

`state.yaml` no longer claims "unforgeable by construction"; it states that
the in-memory capability cannot be forged but **persisted promotion
authenticity is pending trusted approval verification (M9/M13)**. Correction
notes added to the e58.1 verification report and archive manifest.

## Explicitly open

- **U42** full causal grounding (kernel evidence adapter).
- **e60 planning question**: multi-stage execution plans
  (`ExecutionPlan<Vec<Stage>>`) — recorded, not implemented.
- Persisted promotion authenticity (M9/M13 governance).

## Conclusion

The P0 fail-open restore is closed, the digest separation and backend kinds
are enforced, `requires={}` is rejected, and the raw IR no longer exposes a
gate predicate. PASS.
