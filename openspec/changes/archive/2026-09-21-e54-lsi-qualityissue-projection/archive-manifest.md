# Archive Manifest — e54 — QualityIssue compatibility projection (M6.3)

> Cycle: A-lite | Milestone: M6 (Findings & Detector IR) | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e54-lsi-qualityissue-projection` |
| Parent umbrella | `cognicode-living-software-intelligence` |
| Milestone | **M6** (third slice) |
| Umbrella task | **7.6 Implement QualityIssue compatibility projection.** |
| Path | A-lite |
| Phases completed | explore → tasks → apply → verify → archive |
| Final status | **ARCHIVED** |
| Base SHA | `513bf315` (post-e53) |
| Verify verdict | **PASS** |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e54-lsi-qualityissue-projection/exploration-report.md` |
| Tasks | `openspec/changes/e54-lsi-qualityissue-projection/tasks.md` |
| Verification report | `openspec/changes/e54-lsi-qualityissue-projection/verification-report.md` |
| Implementation | commit (this cycle) — `feat(cognicode-core): M6.3 QualityIssue compatibility projection` |

## What was delivered

`domain/findings/quality_projection.rs` — pure
`project_quality_issue(&QualityIssue) -> Result<Finding, FindingError>` with
a conservative, documented policy:

- identity `quality-<id>`; kind `quality.<sanitized category>`;
- severity/status mapped from the legacy strings (unknown ⇒ Info/Open);
- detector `legacy.quality@legacy`;
- `evidence_class = C`, empty evidence, single `location` causal step.

Projected findings are therefore **not explainable** and **cannot block**
the new gates until a re-evidence pass upgrades them — the safe
compatibility direction (the new model represents every legacy issue
without inheriting legacy blocking authority). Enforced by test, not just
documented.

Also added `FindingError::InvalidIdentifier(DetectorIrError)` +
`From<DetectorIrError>` so projection code can propagate identifier errors.

## Evidence

- `cargo test -p cognicode-core --lib domain::findings` → 36 passed; 0 failed.
- `cargo check -p cognicode-core` → 0 errors.
- `cargo fmt -p cognicode-core --check` → clean.
- Domain purity: no I/O imports.

## Milestone progress (M6)

| Umbrella task | Status |
|---------------|--------|
| **7.1 Finding/Risk/EvidenceGrade lifecycle** | ✅ done (e53) |
| **7.2 Detector IR schema/parser/validator** | ✅ done (e52) |
| 7.3 AST detector backend | pending |
| 7.4 graph-pattern backend | pending |
| 7.5 dataflow backend | pending |
| **7.6 QualityIssue compatibility projection** | ✅ done (this cycle) |
| 7.7 Axiom rule import tooling | pending |

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.

## Verification command

```
cargo test -p cognicode-core --lib domain::findings
```

Returns `36 passed; 0 failed`. ✅
