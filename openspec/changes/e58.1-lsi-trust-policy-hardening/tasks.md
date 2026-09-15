# Tasks — cycle e58.1 — trust/policy hardening

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Work units

### WU-1 — ExecutionPermit capability boundary
- `AdmittedDetector`: drop serde; plain data, not executable.
- `AdmittedDetectorRecord`: serializable persistence DTO.
- `ExecutionPermit` (private fields + private seal, no serde) +
  `AdmittedDetectorRecord` ↔ permit conversions.
- `DetectorAdmission::{admit → ExecutionPermit, promote(&permit),
  restore(&record)}`.
- Executor takes `&ExecutionPermit`.

### WU-2 — Backend contract (ceiling)
- `DetectorBackend::evidence_ceiling()`.
- Executor rejects evidence stronger than the ceiling
  (`BackendContractViolation`).

### WU-3 — Detector finding policy
- `DetectorFindingPolicy { default_severity, default_risk }` in `DetectorIr`.
- Remove severity/risk from `DetectorMatch`; assembler applies the policy.

### WU-4 — AST capability correction + IR rule V9
- `AstBackend` advertises `AstPattern` only.
- A detector with executable steps must declare ≥1 capability.

### WU-5 — Checker bug fix
- Honour `cargo test` return code; exit 2 on harness failure; refuse
  `--update`; preserve `first_seen`/`category`/`reason`.

## Acceptance gate (the seven mandatory tests)
| Invariant | Test |
|-----------|------|
| JSON cannot mint executable authority | `json_record_claiming_gated_without_approval_restores_as_candidate`, `a_json_record_cannot_mint_gate_authority` |
| Promotion is not a bare string | `promote_is_the_only_path_to_gated_and_needs_an_approval` |
| AST is not semantic | `ast_is_not_semantic` |
| AST cannot claim A/B evidence | `backend_cannot_overclaim_evidence_class` |
| Backend does not control risk | `detector_policy_not_the_backend_decides_severity_and_risk` |
| Broken harness cannot update the baseline | `--package does-not-exist [--update]` → exit 2 |
| Baseline keeps metadata on update | `--update` preserves `first_seen`/`reason` |

Plus: `cargo test -p cognicode-core --lib domain::findings` (83),
`--test findings_ast_e2e` (6), `cargo check --workspace` (0 errors), fmt
clean, domain pure, gated kernel green.
