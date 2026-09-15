# Exploration Report — cycle e61 — selective Axiom → DetectorIr migration

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

Umbrella task **7.7 Build Axiom rule classifier/import tooling**.

## Key decision — this is NOT a revival of `cognicode-axiom`

The crate is archived; its value now is a **migration corpus**, not its old
architecture. The legacy Sonar importer only carried *catalogue metadata*
(`rule_id`, `name`, `severity`, `language`, `description`, tags) and emitted
TODO stubs: that is not sufficient evidence to responsibly fabricate an
executable `DetectorIr`. For example

```text
S1226  "Method parameters should not be reassigned"  severity=MAJOR  rust
```

does not let anyone infer `MATCH/FLOW/PRODUCE`.

So e61 is **selective translation**, and the guiding principle is
`correct incomplete > fabricated complete`.

## Boundary

```text
legacy rule → AxiomRuleReader → NormalizedLegacyRule
                                   │  AxiomDetectorTranslator
                                   ▼
        Translated(ImportedDetectorDefinition) | Skipped(ImportDiagnostic)
                                   │
                                   ▼  the CALLER (not the importer)
                DetectorAdmission::admit(ir, version, AdmissionSource::Imported)
                                   ▼
                            ExecutionPermit (Candidate)
```

**Cardinal rule:** `legacy authority / quality gate / severity ≠ execution
authority`. A legacy `BLOCKER` maps to *policy* only; authority still comes
solely from `DetectorAdmission` (+ a verified promotion). The importer has no
API that grants authority — it returns a `DetectorIr`, never a permit.

Translation and admission are therefore testable separately.

## Taxonomy (D/E never produce an approximate detector)

| Class | Legacy semantics | Result |
|-------|------------------|--------|
| A | single AST subject | `DetectorIr { requires: {AstPattern} }` |
| B | source → sink reachability | `{GraphQuery}` |
| C | source/sink/sanitizer flow | `{Dataflow}` |
| D | needs unsupported analysis | `Skipped(UnsupportedAnalysis)` |
| E | metadata only | `Skipped(MetadataOnly)` |

Capability is **derived from the legacy semantics**, never declared just to
satisfy the planner.

## Provenance

Kept **beside** the IR (`ImportedDetectorDefinition.legacy:
LegacyRuleProvenance { system, legacy_rule_id, legacy_language,
legacy_category, legacy_severity, source_revision }`), so the IR stays small
and semantic. Threading it through `DetectorExecutionRef → Finding` is a
follow-up (it needs an execution-context carrier, not an IR field — an origin
id in the IR would wrongly change the *semantic* digest of identical logic).

## Severity → policy table (revisable migration policy, never authority)

| Legacy | Finding severity | Risk |
|--------|------------------|------|
| BLOCKER | Critical | Critical |
| CRITICAL | Critical | High |
| MAJOR | Warning | Medium |
| MINOR | Warning | Low |
| INFO | Info | Low |
| UNKNOWN | Warning | Medium |

There is deliberately no `BLOCKER → Gated` path.

## Finding discovered during implementation

The shipped `EligibleSourceVerifier` only trusts `Builtin`/`HumanCurated`
provenance, so an `Imported` detector **cannot** be promoted with it — which is
the correct conservative default. A governance verifier (M9/M13) is required;
the standard promotion path is used either way, with nothing bespoke for
imports.
