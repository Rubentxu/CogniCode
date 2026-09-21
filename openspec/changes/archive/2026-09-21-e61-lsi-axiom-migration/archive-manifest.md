# Archive Manifest — e61 — selective Axiom → DetectorIr migration

> Cycle: A-lite | Milestone: M6 (Findings & Detector IR) | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e61-lsi-axiom-migration` |
| Parent umbrella | `cognicode-living-software-intelligence` |
| Milestone | **M6** — umbrella task 7.7 |
| Path | A-lite |
| Final status | **ARCHIVED** |
| Base SHA | `a3d26360` (post-e60.1) |
| Verify verdict | **PASS** |

## What was delivered

- `NormalizedLegacyRule` + `LegacyDetection` — the legacy DTO carries **explicit
  executable semantics or nothing**; catalogue metadata alone is never enough.
- `AxiomDetectorTranslator` — taxonomy A/B/C/D/E; D/E produce stable skip
  diagnostics, never an approximate detector.
- `ImportedDetectorDefinition { ir, legacy }` + `LegacyRuleProvenance` (beside
  the IR, keeping it small and semantic).
- `AxiomRuleReader` trait + `AxiomImporter` batch report — **creates no
  permits**.
- `policy_for(severity)` — policy only; **no** `BLOCKER → Gated` path.
- Shared `sanitize_segment` helper (removed a duplicate in the quality
  projection).
- 8 unit tests (U-A1..U-A7 + batch) and 3 end-to-end tests (U-A8).

## Explicitly NOT done

- Reviving `cognicode-axiom` (archived by design; it is a corpus, not an
  engine).
- Migrating the whole historical corpus (~hundreds of rules): the taxonomy and
  the reader seam are the deliverable; migration is incremental by nature.
- Threading `LegacyRuleProvenance` into `Finding` (needs an execution-context
  carrier; putting it in the IR would corrupt the semantic digest).

## Milestone progress (M6)

| Umbrella task | Status |
|---------------|--------|
| 7.1 Finding/Risk/EvidenceClass lifecycle | ✅ |
| 7.2 Detector IR schema/parser/validator | ✅ |
| 7.3 AST detector backend | ✅ |
| 7.4 graph-pattern backend | ✅ |
| 7.5 dataflow backend | ✅ |
| 7.6 QualityIssue compatibility projection | ✅ |
| **7.7 Axiom rule import tooling** | ✅ **done (this cycle)** |
| U40-U48 | U40/U41/U43/U47 pass; **U42 remains the M6 closure** |

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.

## Verification commands

```
cargo test -p cognicode-core --lib application::findings
cargo test -p cognicode-core --test findings_axiom_import_e2e
```
