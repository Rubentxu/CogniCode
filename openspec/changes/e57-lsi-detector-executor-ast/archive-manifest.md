# Archive Manifest — e57 — Detector Executor + AST backend

> Cycle: A-lite | Milestone: M6 (Findings & Detector IR) | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e57-lsi-detector-executor-ast` |
| Parent umbrella | `cognicode-living-software-intelligence` |
| Milestone | **M6** — first executable vertical slice |
| Path | A-lite |
| Phases completed | explore → design → tasks → apply → verify → archive |
| Final status | **ARCHIVED** |
| Base SHA | `1e7aa4ab` (post-e56) |
| Verify verdict | **PASS** |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e57-lsi-detector-executor-ast/exploration-report.md` |
| Design | `openspec/changes/e57-lsi-detector-executor-ast/design.md` |
| Tasks | `openspec/changes/e57-lsi-detector-executor-ast/tasks.md` |
| Verification report | `openspec/changes/e57-lsi-detector-executor-ast/verification-report.md` |
| E2E test | `crates/cognicode-core/tests/findings_ast_e2e.rs` |
| Implementation | commit (this cycle) — `feat(cognicode-core): M6 detector executor + AST backend` |

## What was delivered

The first executable, **safe** traversal of the Detector IR, plus the
mandatory execution seam for all later backends:

```
IR → DetectorAdmission → AdmittedDetector → DetectorExecutor
   → BackendRegistry::plan → AstBackend → DetectorOutcome
   → EvidenceSink (EvidenceId) → FindingAssembler → Finding
   → FindingVerifier → Gate
```

| Gap closed | How |
|------------|-----|
| Forgeable authority | `DetectorAdmission::admit` forces `Candidate`; only `promote(approval)` yields `Gated`; the executor takes only an `AdmittedDetector` |
| Shape-only explanation | `FindingVerifier::verify_for_gate` (evidence resolves; causal grounding) distinct from `Finding::validate` |
| Weak digest | SHA-256; `semantic_digest` (stable) split from `instance_digest` |
| Backend divergence | backends emit raw `DetectorOutcome`; the single `FindingAssembler` owns the finding |

## Evidence

- `domain::findings`: **79 tests**; 1 infrastructure test.
- `findings_ast_e2e`: **5 tests** (U40 executable, U43 Candidate cannot block,
  promotion keeps semantic identity, unresolved evidence blocks, U47 loud plan).
- `cargo check --workspace` → 0 errors; fmt clean; domain pure.
- Gated kernel: `--features evidence-kernel` + 82 tests green.

## Milestone progress (M6)

| Umbrella task | Status |
|---------------|--------|
| 7.1 Finding/Risk/EvidenceClass lifecycle | ✅ (e53/e55/e56) |
| 7.2 Detector IR schema/parser/validator | ✅ (e52/e55) |
| **7.3 AST detector backend** | ✅ **executor + AST backend (this cycle)** |
| 7.4 graph-pattern backend | next (reuses the seam) |
| 7.5 dataflow backend | pending (over M5 `ProgramAnalysisService`) |
| 7.6 QualityIssue compatibility projection | ✅ (e54/e55/e56) |
| 7.7 Axiom rule import tooling | pending |
| U40-U48 end-to-end conformance | partially (U40, U43, U47 pass in e57) |

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.

## Verification command

```
cargo test -p cognicode-core --lib domain::findings
cargo test -p cognicode-core --test findings_ast_e2e
```

Returns `79 passed` and `5 passed` (0 failures). ✅
