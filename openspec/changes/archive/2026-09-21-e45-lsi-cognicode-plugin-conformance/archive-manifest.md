# Archive Manifest — e45 — cognicode-plugin conformance

> Cycle: housekeeping | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e45-lsi-cognicode-plugin-conformance` |
| Path | B-direct (housekeeping) |
| Phases completed | explore → spec → tasks → apply (1 WU) → verify |
| Final status | **ARCHIVED** |
| Head SHA | `c6b33edb` |
| Diff stat | **+237 / -0** across **2 files** (1 commit, tests only) |
| Verify verdict | **PASS** (3/3 REQs COMPLIANT) |
| Conformance impact | `pct_verified: 95.2% → 96.1%` (+4 specs, +0.9 pp) |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e45-lsi-cognicode-plugin-conformance/exploration-report.md` |
| Spec | `openspec/changes/e45-lsi-cognicode-plugin-conformance/spec.md` |
| Tasks | `openspec/changes/e45-lsi-cognicode-plugin-conformance/tasks.md` |
| Verification report | `openspec/changes/e45-lsi-cognicode-plugin-conformance/verification-report.md` |
| Implementation | commit `c6b33edb` — `test(cognicode-cli): e45 cognicode-plugin conformance (5 tests, +4 specs)` |
| Evidence map | `sandbox/reports/evidence_map.yaml` (+1 entry: `cognicode-plugin`) |

## What was closed

**RETIREMENT-LEDGER gap**: the 4 specs in
`openspec/specs/cognicode-plugin/spec.md` were all in `no_evidence`
status. The plugin manager (`cmd_plugin_*` in `layout.rs`) and the
manifest parser (`manifest.rs`) existed but had no CLI integration
coverage.

5 integration tests now lock down 2 of the 4 contracts at the CLI
surface (the parser side was already covered by 4 unit tests in
`manifest.rs`):

| Spec REQ | Test |
|----------|------|
| #4 Bundled plugins ship with cogh | `cogh_init_installs_bundled_plugins`, `cogh_plugin_list_enumerates_bundled_plugins_after_init` |
| #1 `plugin.yaml` is the canonical manifest format | `cogh_plugin_list_shows_custom_plugin_with_parsed_description`, `cogh_plugin_list_handles_plugin_without_yaml_gracefully`, `cogh_plugin_add_unknown_name_without_url_is_rejected` |

The remaining 2 REQs (#2 versions addressable by ref, #3 sha256
mandatory) are covered by the existing unit tests in `manifest.rs` for
the parser side; the end-to-end install + verify path requires
`cogh install` which is network-dependent and out of scope.

## Why no tag

The user explicitly froze v1.0.0 tag cuts (`nada de tag 1.0.0, tenemos
que acabar el roadmap`). This housekeeping cycle is not a release:
- No public API change.
- No production code change.
- Only test coverage extension for an existing capability.

A tag here would add noise without information. The head SHA
(`c6b33edb`) is sufficient provenance.

## Verification command

```
cargo test -p cognicode-cli --test cognicode_plugin
```

## Open bounded slices (next candidates)

The remaining 18 `no_evidence` specs in the conformance corpus are
spread across 2 groups in the `cognicode-cli` crate:

| Spec | REQs | Testable today? |
|------|------|------------------|
| `cognicode-ide-adapter` | 8 | partial (depends on `cogh install --ide`, network) |
| `cognicode-lifecycle` | 10 | partial (lifecycle hooks via init + doctor + install manifest) |

The next high-value bounded slice is `e46-lsi-cognicode-lifecycle-conformance`:
10 specs covering `cogh init`, `cogh list`, `cogh doctor` lifecycle
manifests and version pinning. Many can be locked down with
filesystem fixtures alone (no network).
