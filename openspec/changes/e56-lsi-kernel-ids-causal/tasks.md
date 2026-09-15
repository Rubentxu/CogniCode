# Tasks — cycle e56 — kernel ids + structured causal

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Work units

### WU-1 — Extract kernel ids (ungated)
- `git mv evidence_kernel/ids.rs domain/kernel_ids.rs`; retarget module doc.
- Add `ExecutionId` (`exec:N`) + tests.
- Replace `evidence_kernel/ids.rs` with a re-export shim.
- Register `pub mod kernel_ids;` in `domain/mod.rs`.

### WU-2 — Findings use kernel ids
- `Finding.evidence: Vec<EvidenceId>`; delete `EvidenceRef`.
- `ExecutionId` sourced from `kernel_ids` (removed the local copy).
- Re-export `EntityId/EvidenceId/FactId` from `findings`.

### WU-3 — Structured causal lineage
- `CausalStepKind` + `CausalStep { kind, subject, fact, evidence, detail }`
  with builders.
- QualityIssue projection → `CausalStepKind::Location`.

### WU-4 — Tests
- kernel id `exec:N` round-trip; navigable causal step round-trip;
  updated findings tests.

## Acceptance gate
- `cargo test -p cognicode-core --lib domain::findings` green (47 tests).
- `cargo test -p cognicode-core --lib kernel_ids` green (12 tests).
- `cargo check -p cognicode-core --features evidence-kernel` green;
  `cargo test -p cognicode-core --features evidence-kernel --lib evidence_kernel`
  green (82 tests).
- `cargo check --workspace` green; fmt clean; domain pure.
- Conventional commit, no AI trailers.
