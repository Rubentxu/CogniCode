# Verification Report — cycle e58.1 — trust/policy hardening

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e58.1 — trust/policy hardening |
| Path | A-lite |
| Base HEAD | `6b2ac1b1` (post-e58) |
| Verify verdict | **PASS** |

## Verification

### V-1 — Domain + E2E tests

```
cargo test -p cognicode-core --lib domain::findings   # 83 passed
cargo test -p cognicode-core --test findings_ast_e2e  # 6 passed
```

### V-2 — The seven mandatory invariants

| Invariant | Evidence |
|-----------|----------|
| JSON cannot mint executable authority | `json_record_claiming_gated_without_approval_restores_as_candidate` (admission) + `a_json_record_cannot_mint_gate_authority` (E2E): a forged `Gated` record restores as `Candidate` |
| Promotion is not a bare string | `promote_is_the_only_path_to_gated_and_needs_an_approval`; executor signature takes `&ExecutionPermit` only |
| AST is not semantic | `ast_is_not_semantic`: `plan({SemanticResolution})` fails against the AST-only registry |
| AST cannot claim A/B evidence | `backend_cannot_overclaim_evidence_class`: runtime evidence from a C-ceiling backend → `BackendContractViolation::EvidenceCeilingExceeded` |
| Backend does not control risk | `detector_policy_not_the_backend_decides_severity_and_risk`; `DetectorMatch` has no severity/risk fields |
| Broken harness cannot update the baseline | `--package does-not-exist` → exit **2**; `--update` likewise exit 2, file untouched |
| Baseline keeps metadata on update | `--update` preserves `first_seen` / `category` / `reason` (verified by diff) |

### V-3 — Build / purity / format / kernel

- `cargo check --workspace` → 0 errors.
- `cargo fmt -p cognicode-core --check` → clean.
- No `sqlx`/`tokio`/I/O imports in `domain/findings/`.
- Gated kernel unaffected.

### V-4 — Type-level unforgeability

`ExecutionPermit` is not `Serialize`/`Deserialize` and its fields/seal are
private; the only constructors are `DetectorAdmission::{admit, promote,
restore}`. This is enforced by the type system, not by a runtime check.

## U42 status

**Still open.** The verifier proves referential existence, not the full
kernel chain (`Evidence → FactId → Fact → Entity/Snapshot/Provenance`).
Requires the kernel evidence adapter; the `EvidenceLookup` port is the seam.

## Conclusion

Authority is now unforgeable by construction, a backend can no longer
influence the gate epistemically, the AST backend advertises only what it
does, and the known-failure checker is safe. PASS.
