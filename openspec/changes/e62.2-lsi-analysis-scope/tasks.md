# Tasks — cycle e62.2 — analysis scope pinning (U42, part 1)

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

| WU | Content |
|----|---------|
| WU1a | `AnalysisScope { workspace, snapshot }` |
| WU1b | `AnalysisInput.scope`; `DetectorExecutor` requires it (`MissingScope`) |
| WU1c | `DetectorExecutionRef.scope` captured from the run |
| WU1d | `ExecutionPermit::execution_ref(execution_id, scope)` |
| WU4a1 | `EvidenceLookup::scope()` |
| WU4a2 | `FindingVerifier` scope-first check → `VerificationError::ScopeMismatch` |
| WU4a3 | Adversarial test with identical ids in two snapshots |

## Acceptance gate
- `domain::findings` 101 (incl. `scope_mismatch_is_rejected_before_any_id_is_resolved`).
- `application::findings` 21; AST E2E 6; Graph E2E 4; Dataflow E2E 7;
  axiom-import E2E 3.
- `cargo check --workspace --all-targets` 0 errors; fmt clean;
  known-failure baseline unchanged.
