# Archive Manifest — e58.1 — trust/policy hardening

> Cycle: A-lite | Milestone: M6 (Findings & Detector IR) | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e58.1-lsi-trust-policy-hardening` |
| Parent umbrella | `cognicode-living-software-intelligence` |
| Milestone | **M6** — boundary hardening before Graph |
| Path | A-lite |
| Phases completed | explore → design → tasks → apply → verify → archive |
| Final status | **ARCHIVED** |
| Base SHA | `6b2ac1b1` (post-e58) |
| Verify verdict | **PASS** |

## What was delivered

| Gap | Fix |
|-----|-----|
| **G-1** `AdmittedDetector` forgeable | `ExecutionPermit` (private fields + private seal, no serde) is the only executable capability; `AdmittedDetector` is non-serializable plain data; persistence via `AdmittedDetectorRecord` + `restore` (re-validate + downgrade unapproved `Gated`); executor takes `&ExecutionPermit` |
| **G-2** backend influenced the gate | `DetectorBackend::evidence_ceiling()` + executor rejection; `DetectorMatch` loses severity/risk; `DetectorFindingPolicy` in the detector decides severity/risk |
| **G-3** AST over-advertised | `AstBackend` → `AstPattern` only; IR rule V9 (executable steps ⇒ ≥1 declared capability) |
| e58 checker bug | return code honoured; harness failure → exit 2; `--update` refused on harness failure; metadata preserved |

## Evidence

- `domain::findings`: **83 tests**; `findings_ast_e2e`: **6 tests**.
- The seven mandatory invariants all have a test (see verification report).
- `cargo check --workspace` 0 errors; fmt clean; domain pure.
- Checker: normal exit 0; `--package does-not-exist` exit 2; `--update` preserves metadata.

## Explicitly open

- **U42 full causal grounding** — needs the kernel evidence adapter
  (`CausalStep → EvidenceId → Evidence → FactId → Fact → Entity/Snapshot`).
  `state.yaml` keeps U42 incomplete.

## Milestone progress (M6)

| Umbrella task | Status |
|---------------|--------|
| 7.1 Finding/Risk/EvidenceClass lifecycle | ✅ (e53/e55/e56) |
| 7.2 Detector IR schema/parser/validator | ✅ (e52/e55/e58.1) |
| 7.3 AST detector backend | ✅ (e57) + boundary hardened (e58.1) |
| 7.4 graph-pattern backend | ready — seam is hardened |
| 7.5 dataflow backend | pending (over M5 `ProgramAnalysisService`) |
| 7.6 QualityIssue compatibility projection | ✅ (e54/e55/e56) |
| 7.7 Axiom rule import tooling | pending |
| U40-U48 | U40/U43/U47 pass; U42 open (needs kernel adapter) |

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.

## Verification commands

```
cargo test -p cognicode-core --lib domain::findings
cargo test -p cognicode-core --test findings_ast_e2e
python3 scripts/check_known_failures.py
```

Return `83 passed`, `6 passed`, and exit 0 respectively. ✅
