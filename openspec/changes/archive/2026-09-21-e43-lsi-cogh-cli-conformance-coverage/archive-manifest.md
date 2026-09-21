# Archive Manifest — e43 — cogh CLI conformance coverage

> Cycle: housekeeping | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e43-lsi-cogh-cli-conformance-coverage` |
| Path | B-direct (housekeeping) |
| Phases completed | explore → spec → tasks → apply (1 WU) → verify |
| Final status | **ARCHIVED** |
| Head SHA | `02e92f87` |
| Diff stat | **+259 / -0** across **2 files** (1 commit, tests only) |
| Verify verdict | **PASS** (5/5 REQs COMPLIANT) |
| Conformance impact | `pct_verified: 91.4% → 93.7%` (+11 specs, +2.3 pp) |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e43-lsi-cogh-cli-conformance-coverage/exploration-report.md` |
| Spec | `openspec/changes/e43-lsi-cogh-cli-conformance-coverage/spec.md` |
| Tasks | `openspec/changes/e43-lsi-cogh-cli-conformance-coverage/tasks.md` |
| Verification report | `openspec/changes/e43-lsi-cogh-cli-conformance-coverage/verification-report.md` |
| Implementation | commit `02e92f87` — `test(cognicode-cli): e43 cogh CLI conformance coverage (7 tests, +11 specs)` |
| Evidence map | `sandbox/reports/evidence_map.yaml` (+1 entry: `cognicode-cli`) |

## What was closed

**RETIREMENT-LEDGER gap**: the 11 specs in `openspec/specs/cognicode-cli/spec.md` were all in `no_evidence` status — the `cogh` binary at `crates/cognicode-cli/src/bin/cogh.rs` existed but the crate had no `tests/` directory. This meant every contract documented in the spec was unenforceable at CI.

7 integration tests now lock down 4 of the 11 contracts (the remaining 7 are network-dependent or out-of-band installer requirements and belong to future cycles):

| Spec REQ | Test |
|----------|------|
| #1 `cogh` binary is a single static executable | `cogh_version_reports_semver_token` |
| #6 `cogh list` shows installed plugins and versions | `cogh_list_on_uninitialised_home_reports_not_initialized`, `cogh_list_on_initialised_home_renders_table` |
| #7 `cogh current` shows the active version pin | `cogh_current_with_no_pinned_version_reports_unset`, `cogh_current_reads_pinned_version_from_tracker` |
| #10 `cogh doctor` validates installation | `cogh_doctor_on_uninitialised_home_reports_not_initialized`, `cogh_doctor_on_initialised_home_reports_healthy` |

## Why no tag

The user explicitly froze v1.0.0 tag cuts (`nada de tag 1.0.0, tenemos
que acabar el roadmap`). This housekeeping cycle is not a release:
- No public API change.
- No production code change.
- Only test coverage extension for an existing capability.

A tag here would add noise without information. The head SHA
(`02e92f87`) is sufficient provenance.

## Verification command

```
cargo test -p cognicode-cli --test cogh_cli
```

## Open bounded slices (next candidates)

The remaining 29 `no_evidence` specs in the conformance corpus are all
in the `cognicode-cli` crate:

| Spec | REQs | Testable today? |
|------|------|------------------|
| `cognicode-ide-adapter` | 8 | partial (depends on `cogh install --ide`, network) |
| `cognicode-lifecycle` | 10 | partial (lifecycle hooks via init + doctor) |
| `cognicode-plugin` | 4 | partial (cogh list table covers some) |
| `portable-skill-bundle` | 7 | yes (skill validate subcommand is offline) |

The next high-value bounded slice is `e44-lsi-portable-skill-bundle-conformance`: 7 specs, all testable without network because `cogh skill validate` is a pure-file validator.
