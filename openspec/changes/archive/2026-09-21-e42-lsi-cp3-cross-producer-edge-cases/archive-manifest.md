# Archive Manifest — e42 — CP-3 cross-producer edge cases

> Cycle: housekeeping | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e42-lsi-cp3-cross-producer-edge-cases` |
| Path | B-direct (housekeeping) |
| Phases completed | explore → spec → tasks → apply (1 WU) → verify |
| Final status | **ARCHIVED** |
| Head SHA | `f102655b` |
| Diff stat | **+262 / -0** across **1 file** (1 commit, tests only) |
| Verify verdict | **PASS** (3/3 REQs COMPLIANT) |
| Ledger impact | none (housekeeping cycle) |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e42-lsi-cp3-cross-producer-edge-cases/exploration-report.md` |
| Spec | `openspec/changes/e42-lsi-cp3-cross-producer-edge-cases/spec.md` |
| Tasks | `openspec/changes/e42-lsi-cp3-cross-producer-edge-cases/tasks.md` |
| Verification report | `openspec/changes/e42-lsi-cp3-cross-producer-edge-cases/verification-report.md` |
| Implementation | commit `f102655b` — `test(cognicode-core): CP-3 cross-producer edge cases (e42)` |

## What was closed

**RETIREMENT-LEDGER gap**: the CP-3 cross-producer join contract
(`lsp_facts::reference_subject`, line 213) was implemented in e38.1
with a documented fallback rule (file-path fallback when `container`
is `None` or unresolvable). The implementation had ONE happy-path test
(`resolvable_container_normalizes_to_enclosing_symbol_fact_side_fqn`)
covering only the successful resolution path. The two fallback paths
were untested.

Two RED-first tests now lock down the fallback contract:

| Test | Coverage |
|------|----------|
| `cp3_unreported_container_falls_back_to_file_path` | `container: None` ⇒ call does NOT join any symbol |
| `cp3_unresolvable_container_falls_back_to_file_path` | `container: Some(unknown)` ⇒ call does NOT join `real_fn` and no entity is fabricated under the bogus name |

## Why no tag

The user explicitly froze v1.0.0 tag cuts (`nada de tag 1.0.0, tenemos
que acabar el roadmap`). This housekeeping cycle is not a release:
- No public API change.
- No production code change.
- Only test coverage extension for an existing contract.

A tag here would add noise without information. The head SHA
(`f102655b`) is sufficient provenance.

## Verification command

```
cargo test -p cognicode-core --lib --features evidence-kernel 'lsp_facts::tests'
```
