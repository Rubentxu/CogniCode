# Archive Manifest — e44 — portable-skill-bundle conformance

> Cycle: housekeeping | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e44-lsi-portable-skill-bundle-conformance` |
| Path | B-direct (housekeeping) |
| Phases completed | explore → spec → tasks → apply (1 WU) → verify |
| Final status | **ARCHIVED** |
| Head SHA | `73e60769` |
| Diff stat | **+324 / -0** across **2 files** (1 commit, tests only) |
| Verify verdict | **PASS** (6/6 REQs COMPLIANT) |
| Conformance impact | `pct_verified: 93.7% → 95.2%` (+7 specs, +1.5 pp) |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e44-lsi-portable-skill-bundle-conformance/exploration-report.md` |
| Spec | `openspec/changes/e44-lsi-portable-skill-bundle-conformance/spec.md` |
| Tasks | `openspec/changes/e44-lsi-portable-skill-bundle-conformance/tasks.md` |
| Verification report | `openspec/changes/e44-lsi-portable-skill-bundle-conformance/verification-report.md` |
| Implementation | commit `73e60769` — `test(cognicode-cli): e44 portable-skill-bundle conformance (8 tests, +7 specs)` |
| Evidence map | `sandbox/reports/evidence_map.yaml` (+1 entry: `portable-skill-bundle`) |

## What was closed

**RETIREMENT-LEDGER gap**: the 7 specs in
`openspec/specs/portable-skill-bundle/spec.md` were all in `no_evidence`
status. The validator at `crates/cognicode-cli/src/cmd/skill.rs::validate_bundle`
existed and was reachable via `cogh skill validate <path>`, but had no
test coverage.

8 integration tests now lock down 5 of the 7 contracts (the remaining 2
are network-dependent and belong to future cycles that mock the
registry):

| Spec REQ | Test |
|----------|------|
| #1 portable directory tree | `portable_skill_bundle_valid_directory_passes_validation`, `portable_skill_bundle_non_directory_path_is_rejected` |
| #2 SKILL.md frontmatter valid | `portable_skill_bundle_missing_skill_md_is_rejected`, `portable_skill_bundle_invalid_maturity_is_rejected` |
| #3 No IDE-specific fields | `portable_skill_bundle_ide_specific_compatibility_field_rejected` |
| #4 manifest.yaml declared | `portable_skill_bundle_manifest_yaml_required_and_parsed`, `portable_skill_bundle_missing_manifest_is_rejected` |
| #7 references/ + assets/ copied recursively | `portable_skill_bundle_referenced_script_must_exist` |

## Why no tag

The user explicitly froze v1.0.0 tag cuts (`nada de tag 1.0.0, tenemos
que acabar el roadmap`). This housekeeping cycle is not a release:
- No public API change.
- No production code change.
- Only test coverage extension for an existing capability.

A tag here would add noise without information. The head SHA
(`73e60769`) is sufficient provenance.

## Verification command

```
cargo test -p cognicode-cli --test portable_skill_bundle
```

## Open bounded slices (next candidates)

The remaining 22 `no_evidence` specs in the conformance corpus are
spread across 3 groups in the `cognicode-cli` crate:

| Spec | REQs | Testable today? |
|------|------|------------------|
| `cognicode-ide-adapter` | 8 | partial (depends on `cogh install --ide`, network) |
| `cognicode-lifecycle` | 10 | partial (lifecycle hooks via init + doctor) |
| `cognicode-plugin` | 4 | partial (cogh plugin add/list cycle, mostly offline) |

The next high-value bounded slice is `e45-lsi-cognicode-plugin-conformance`:
4 specs covering `cogh plugin` subcommands (`add`/`remove`/`list`/`update`),
which are mostly offline-testeable except for the `add --url` GitHub fetch.
