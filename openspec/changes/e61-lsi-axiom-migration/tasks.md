# Tasks — cycle e61 — selective Axiom → DetectorIr migration

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

| WU | Content |
|----|---------|
| WU1 | `NormalizedLegacyRule` + `LegacyDetection` (explicit executable semantics, or `None`) |
| WU2 | `AxiomDetectorTranslator` with taxonomy A/B/C/D/E; D/E → stable diagnostics |
| WU3 | `ImportedDetectorDefinition { ir, legacy }` + `LegacyRuleProvenance` (beside the IR) |
| WU4 | `AxiomRuleReader` + `AxiomImporter` batch report (**no permits**) |
| WU5 | `policy_for(severity)` — policy only, never authority |
| WU6 | Shared `sanitize_segment` helper (removes the duplicate in the quality projection) |
| WU7 | U-A1..U-A7 unit tests + U-A8 end-to-end |

## Acceptance gate
- `application::findings` 21 (axiom 8 + dataflow 13).
- `findings_axiom_import_e2e` 3 (U-A8: same chain as a builtin; imported
  provenance not eligible for the default verifier; gates only through a
  governance verifier).
- `cargo check --workspace --all-targets` 0 errors; fmt clean;
  known-failure baseline unchanged.
