# Tasks — cycle e57 — Detector Executor + AST backend

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Work units

### WU-1 — Strong, split digests
- `digest.rs`: `sha256_hex`, `DetectorDigest` (`sha256:<64hex>`).
- `DetectorIr::semantic_digest()` `(id, requires, steps)`;
  `DetectorIr::instance_digest()` (whole definition).
- `DetectorExecutionRef { …, semantic_digest, instance_digest, … }`.

### WU-2 — Admission boundary
- `DetectorVersion`, `AdmissionSource`, `AdmissionRef`, `PromotionApproval`.
- `DetectorAdmission::admit` always yields `Candidate` (discards any claimed
  authority); `DetectorAdmission::promote` is the only path to `Gated`.
- `AdmittedDetector::execution_ref`.

### WU-3 — Backend output + ports
- `EvidenceKind` (class strength), `ProducedEvidence`, `CausalObservation`,
  `DetectorMatch`, `DetectorDiagnostic`, `DetectorOutcome`.
- `EvidenceSink` / `EvidenceLookup` ports + `InMemoryEvidenceStore`.

### WU-4 — Assembler (single Finding factory)
- `FindingAssembler::assemble`: assigns evidence class (strongest of the
  evidence, D when none), execution ref, evidence ids, origin, causal chain,
  finding id; rejects causal evidence outside the finding's evidence set.

### WU-5 — Verifier (referential truth, U42)
- `FindingVerifier::verify_for_gate` + `can_block`.

### WU-6 — Executor + planner + AST backend
- `DetectorBackend`, `BackendRegistry::plan`, `DetectorExecutor::execute`,
  `ExecutionRecord`.
- `AstBackend` over `AstInput`.

### WU-7 — E2E vertical slice
- `tests/findings_ast_e2e.rs`: U40 (executable), U43 (Candidate never
  blocks), promotion keeps semantic identity, unresolved evidence blocks,
  U47 (planning fails loud).

## Acceptance gate
- `cargo test -p cognicode-core --lib domain::findings` green (79).
- `cargo test -p cognicode-core --test findings_ast_e2e` green (5).
- `cargo check --workspace` green; fmt clean; domain pure.
- gated kernel tests still green (82).
- Conventional commit, no AI trailers.
