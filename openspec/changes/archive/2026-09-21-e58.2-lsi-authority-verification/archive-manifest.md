# Archive Manifest — e58.2 — authority verification & contract tightening

> Cycle: A-lite | Milestone: M6 (Findings & Detector IR) | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e58.2-lsi-authority-verification` |
| Parent umbrella | `cognicode-living-software-intelligence` |
| Milestone | **M6** — closes the e58.1 P0 + P1s before Graph |
| Path | A-lite |
| Phases completed | explore → design → tasks → apply → verify → archive |
| Final status | **ARCHIVED** |
| Base SHA | `1b2ab25b` (post-e58.1) |
| Verify verdict | **PASS** |

## What was delivered

| Item | Fix |
|------|-----|
| **P0** fail-open restore | `restore` is fail-closed (always `Candidate`); `restore_with` recovers `Gated` only through an `ApprovalVerifier`; promotion requires a sealed `VerifiedPromotion` minted by `PromotionAuthority::verify`. A stored approval string is never proof. |
| P1 digest | `logic_digest` / `policy_digest` / `semantic_digest` / `instance_digest`; a policy change alters semantic (not logic) |
| P1 backend kind | matches validated against `DetectorIr::produced_kind()` → `UnexpectedFindingKind` |
| P1 empty requires | V9 requires ≥1 capability for every detector |
| P1 trap API | `DetectorIr::can_block()` removed |
| P1 docs | `state.yaml` + e58.1 correction notes: persisted promotion authenticity is **pending** (M9/M13) |

## Evidence

- `domain::findings`: **86 tests**; `findings_ast_e2e`: **6 tests**.
- Six mandatory invariants each covered (see verification report).
- `cargo check --workspace` 0 errors; fmt clean; domain pure; gated kernel 82 green;
  known-failure checker exit 0.

## Precise status statement

> `ExecutionPermit` cannot be directly forged or deserialized; raw detector
> authority is normalized at admission. Persisted promotion authenticity
> remains pending trusted approval verification (M9/M13); the default restore
> path never restores `Gated`.

## Milestone progress (M6)

| Umbrella task | Status |
|---------------|--------|
| 7.1 Finding/Risk/EvidenceClass lifecycle | ✅ |
| 7.2 Detector IR schema/parser/validator | ✅ |
| 7.3 AST detector backend | ✅ (e57) + boundaries hardened (e58.1/e58.2) |
| 7.4 graph-pattern backend | next |
| 7.5 dataflow backend | pending |
| 7.6 QualityIssue compatibility projection | ✅ |
| 7.7 Axiom rule import tooling | pending |
| U40-U48 | U40/U43/U47 pass; U42 open (kernel adapter) |

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.

## Verification commands

```
cargo test -p cognicode-core --lib domain::findings
cargo test -p cognicode-core --test findings_ast_e2e
python3 scripts/check_known_failures.py
```

Return `86 passed`, `6 passed`, and exit 0. ✅
