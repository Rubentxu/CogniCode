# Design — cycle e61 — selective Axiom → DetectorIr migration

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Pipeline

```
AxiomRuleReader.read(rule_id) ─► NormalizedLegacyRule
                                     │
                        AxiomDetectorTranslator::translate
                                     │
            ┌────────────────────────┴───────────────────────┐
            ▼                                                ▼
 Translated(ImportedDetectorDefinition)           Skipped(ImportDiagnostic)
   { ir, legacy: LegacyRuleProvenance }             kind ∈ {MetadataOnly,
            │                                                 UnsupportedAnalysis,
            │  (caller)                                        Unmappable}
            ▼
 DetectorAdmission::admit(ir, version, AdmissionSource::Imported)
            ▼
      ExecutionPermit(Candidate)
```

The importer cannot produce an `ExecutionPermit`; authority stays at the
admission boundary.

## IR construction per class

| Class | `requires` | steps |
|-------|-----------|-------|
| A | `{AstPattern}` | `MATCH s, PRODUCE k` |
| B | `{GraphQuery}` | `MATCH src, FLOW src→sink, PRODUCE k` |
| C | `{Dataflow}` | `MATCH src, FLOW src→sink, EXCLUDE…, PRODUCE k` |

Steps follow the canonical IR order (MATCH, FLOW, EXCLUDE…, PRODUCE) so V5/V11
hold; the translator validates the result and **skips** (with a diagnostic) if
it is invalid rather than emitting an approximate detector.

Finding kind: `{sanitize(category)}.{sanitize(rule_id)}` — deterministic.
Detector id: `{sanitize(system)}.{sanitize(rule_id)}`.

## Authority

```
legacy BLOCKER ──► DetectorFindingPolicy(Critical, Critical)   [policy]
legacy BLOCKER ──✗──► DetectorAuthority::Gated                 [never]
```

The translated IR is emitted with `authority = Candidate`; `admit` normalises
anyway. Only `VerifiedPromotion` (with an `ApprovalVerifier`) can gate.

## Provenance placement

`LegacyRuleProvenance` lives beside the IR. It is deliberately not an IR field:
an origin id inside the definition would change the **semantic** digest of two
detectors with identical logic, which is exactly what the digest split
(e58.2/e59) exists to prevent. Threading provenance to `Finding` is a
follow-up needing an execution-context carrier.
