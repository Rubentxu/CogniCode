# Archive Manifest — e52 — Detector IR (M6.1)

> Cycle: A-lite | Milestone: M6 (Findings & Detector IR) | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e52-lsi-detector-ir` |
| Parent umbrella | `cognicode-living-software-intelligence` |
| Milestone | **M6 — Findings & Detector IR** (first slice) |
| Umbrella task | **7.2 Define Detector IR schema/parser/validator.** |
| Path | A-lite |
| Phases completed | explore → design → tasks → apply → verify → archive |
| Final status | **ARCHIVED** |
| Base SHA | `2fad1e1a` (post-e51) |
| Diff stat | new module (2 files) + 1-line registration; +765 LOC incl. 15 tests |
| Verify verdict | **PASS** |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e52-lsi-detector-ir/exploration-report.md` |
| Design | `openspec/changes/e52-lsi-detector-ir/design.md` |
| Tasks | `openspec/changes/e52-lsi-detector-ir/tasks.md` |
| Verification report | `openspec/changes/e52-lsi-detector-ir/verification-report.md` |
| Implementation | commit (this cycle) — `feat(cognicode-core): M6.1 detector IR domain model` |

## What was delivered

The declarative **Detector IR** domain contract for M6, additive and
pure (no I/O, mirroring the M5 `analytics::program_analysis` precedent):

- `AnalysisLevel` — the ordered escalation ladder (`AST_PATTERN` …
  `LLM_CONTEXT`) with `rank()` / `Display` / `Ord`.
- `DetectorStep` — `MATCH` / `FLOW` / `EXCLUDE` / `VERIFY` / `PRODUCE`.
- `SubjectPattern` / `FindingKind` / `DetectorId` — validated newtypes
  (dot-separated `ns.name` grammar).
- `DetectorAuthority` — `Candidate` (no GATE authority) vs `Gated`,
  with `can_block()` as the single gate predicate.
- `DetectorIr::validate()` — fail-loud validator implementing rules
  V1–V8 (identity, exactly-one trailing `PRODUCE`, step ordering, and
  the escalation floor: `FLOW ⇒ ≥ GRAPH_QUERY`, `VERIFY feasible_path ⇒
  ≥ SYMBOLIC`).

This satisfies the umbrella `detector-ir` requirements *"Validated
detector definition"* (admission validation + unsupported construct
fails loud) and *"AI detector authority"* (candidate cannot block).
Per the LSI process rule, requirement IDs stay in the umbrella change;
this evolutivo carries no duplicate spec delta.

## Evidence

- `cargo test -p cognicode-core --lib domain::findings` → 15 passed;
  0 failed.
- `cargo check -p cognicode-core` → 0 errors (2 pre-existing warnings).
- `cargo fmt -p cognicode-core --check` → clean.
- Domain purity: no `sqlx`/`tokio`/I/O imports.

## Milestone progress (M6)

| Umbrella task | Status |
|---------------|--------|
| 7.1 Finding/Risk/EvidenceGrade lifecycle | pending (next slice) |
| **7.2 Detector IR schema/parser/validator** | ✅ **done (this cycle)** |
| 7.3 AST detector backend | pending |
| 7.4 graph-pattern backend | pending |
| 7.5 dataflow backend | pending |
| 7.6 QualityIssue compatibility projection | pending |
| 7.7 Axiom rule import tooling | pending |

## Why no tag

Roadmap work in progress; user froze v1.0.0 tag cuts. No public release.

## Verification command

```
cargo test -p cognicode-core --lib domain::findings
```

Returns `15 passed; 0 failed`. ✅
