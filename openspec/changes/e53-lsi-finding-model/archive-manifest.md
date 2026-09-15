# Archive Manifest — e53 — Finding & evidence-class model (M6.2)

> Cycle: A-lite | Milestone: M6 (Findings & Detector IR) | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e53-lsi-finding-model` |
| Parent umbrella | `cognicode-living-software-intelligence` |
| Milestone | **M6** (second slice) |
| Umbrella task | **7.1 Define Finding/Risk/EvidenceGrade lifecycle.** |
| Path | A-lite |
| Phases completed | explore → tasks → apply → verify → archive |
| Final status | **ARCHIVED** |
| Base SHA | `811b9389` (post-e52) |
| Verify verdict | **PASS** |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e53-lsi-finding-model/exploration-report.md` |
| Tasks | `openspec/changes/e53-lsi-finding-model/tasks.md` |
| Verification report | `openspec/changes/e53-lsi-finding-model/verification-report.md` |
| Implementation | commit (this cycle) — `feat(cognicode-core): M6.2 finding & evidence-class model` |

## What was delivered

- `EvidenceClass { A..D }` — strength scale with `satisfies(minimum)`.
- `FindingSeverity`, `RiskLevel`, `FindingStatus` (+ ranks / `is_active`).
- `FindingId`, `EvidenceRef`, `DetectorRef`, `CausalStep`.
- `Finding` with `validate()` / `is_explainable()` / `can_block(gate)`.
- `FindingGate { min_evidence_class, min_risk }` + `admits()`.

Both umbrella `finding-evidence-model` requirements are satisfied:
*Finding evidence chain* (blocking findings link evidence + detector/version
and expose a causal chain) and *Evidence grades* (A–D; a grade-D hypothesis
cannot clear a grade-B gate). No duplicate spec delta — requirement IDs stay
in the umbrella change per the LSI process rule.

## Evidence

- `cargo test -p cognicode-core --lib domain::findings` → 26 passed; 0 failed.
- `cargo check -p cognicode-core` → 0 errors.
- `cargo fmt -p cognicode-core --check` → clean.
- Domain purity: no I/O imports.

## Milestone progress (M6)

| Umbrella task | Status |
|---------------|--------|
| **7.1 Finding/Risk/EvidenceGrade lifecycle** | ✅ **done (this cycle)** |
| **7.2 Detector IR schema/parser/validator** | ✅ done (e52) |
| 7.3 AST detector backend | pending |
| 7.4 graph-pattern backend | pending |
| 7.5 dataflow backend | pending |
| 7.6 QualityIssue compatibility projection | pending |
| 7.7 Axiom rule import tooling | pending |

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.

## Verification command

```
cargo test -p cognicode-core --lib domain::findings
```

Returns `26 passed; 0 failed`. ✅
