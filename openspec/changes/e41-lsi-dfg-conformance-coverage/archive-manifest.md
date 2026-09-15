# Archive Manifest — e41 — DFG conformance coverage

> Cycle: housekeeping | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e41-lsi-dfg-conformance-coverage` |
| Path | B-direct (housekeeping) |
| Phases completed | explore → spec → tasks → apply (1 WU) → verify |
| Final status | **ARCHIVED** |
| Head SHA | `9dda3e89` |
| Diff stat | **+43 / -2** across **1 file** (1 commit) |
| Verify verdict | **PASS** (4/4 REQs COMPLIANT) |
| Ledger impact | none (housekeeping cycle) |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e41-lsi-dfg-conformance-coverage/exploration-report.md` |
| Spec | `openspec/changes/e41-lsi-dfg-conformance-coverage/spec.md` |
| Tasks | `openspec/changes/e41-lsi-dfg-conformance-coverage/tasks.md` |
| Verification report | `openspec/changes/e41-lsi-dfg-conformance-coverage/verification-report.md` |
| Implementation | commit `9dda3e89` — `test(cognicode-core): register DFG fixtures in conformance corpus (e41)` |

## M5 progress impact

The M5 conformance corpus now covers **all 7 algorithm kinds** (was 6/7):

| Algorithm | Fixture count (after e41) |
|-----------|----------------------------|
| `cfg_per_function` | 2 |
| `dominators_cfg` | 1 |
| `slice_forward` | 1 |
| `slice_backward` | 1 |
| `interproc_summary` | 1 |
| `taint_flow` | 2 |
| `dfg` | 2 ← NEW |

`run_corpus()`, `replay_guard`, and `perf_envelope` now exercise the full M5
algorithm surface end-to-end. The conformance matrix (415 verified REQs) does
not change because this is sub-coverage of an existing requirement, not new
spec content.

## Why no tag

The user explicitly froze v1.0.0 tag cuts (`nada de tag 1.0.0, tenemos
que acabar el roadmap`). This housekeeping cycle is not a release:
- No public API change.
- No semantic-version bump.
- No migration or schema change.
- Only test corpus coverage extended.

A tag here would add noise without information. The head SHA
(`9dda3e89`) is sufficient provenance for this cycle.

## Verification command

```
cargo test -p cognicode-core --lib --features program-analysis-server 'program_analysis'
```
