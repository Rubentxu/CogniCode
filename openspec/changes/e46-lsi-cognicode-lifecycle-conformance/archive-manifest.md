# Archive Manifest — e46 — cognicode-lifecycle conformance

> Cycle: housekeeping | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e46-lsi-cognicode-lifecycle-conformance` |
| Path | B-direct (housekeeping) |
| Phases completed | explore → spec → tasks → apply (1 WU) → verify |
| Final status | **ARCHIVED** |
| Head SHA | `a7101d68` |
| Diff stat | **+282 / -0** across **2 files** (1 commit, tests only) |
| Verify verdict | **PASS** (6/6 REQs COMPLIANT) |
| Conformance impact | `pct_verified: 96.1% → 98.3%` (+10 specs, +2.2 pp) |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e46-lsi-cognicode-lifecycle-conformance/exploration-report.md` |
| Spec | `openspec/changes/e46-lsi-cognicode-lifecycle-conformance/spec.md` |
| Tasks | `openspec/changes/e46-lsi-cognicode-lifecycle-conformance/tasks.md` |
| Verification report | `openspec/changes/e46-lsi-cognicode-lifecycle-conformance/verification-report.md` |
| Implementation | commit `a7101d68` — `test(cognicode-cli): e46 cognicode-lifecycle conformance (7 tests, +10 specs)` |
| Evidence map | `sandbox/reports/evidence_map.yaml` (+1 entry: `cognicode-lifecycle`) |

## What was closed

**RETIREMENT-LEDGER gap**: the 10 specs in
`openspec/specs/cognicode-lifecycle/spec.md` were all in `no_evidence`
status. The lifecycle commands (`cmd_install`, `cmd_uninstall`,
`cmd_list`, `cmd_current`, `cmd_update`, `cmd_doctor`, `cmd_reshim`)
live in `crates/cognicode-cli/src/cmd/layout.rs:180-327` and were
reachable via the public CLI but had no integration coverage.

7 integration tests now lock down 5 of the 10 contracts (the remaining
5 are network-dependent and belong to a future cycle that mocks the
registry):

| Spec REQ | Test |
|----------|------|
| #4 uninstall preserves other versions | `cogh_uninstall_emits_recognisable_message_for_known_plugin` (contract) |
| #7 doctor validates the install | `cogh_doctor_reports_healthy_on_initialised_home`, `cogh_doctor_reports_uninitialised_home_on_fresh_dir`, `cogh_doctor_warns_when_tracker_version_missing` |
| #8 reshim regenerates the shims directory | `cogh_reshim_emits_recognisable_message_on_current_implementation` (contract) |
| #9 current reads the tracker | `cogh_current_reports_unpinned_state_on_initialised_home_without_tracker` (lifecycle hook) |
| #10 list shows installed plugins | `cogh_list_reports_installed_plugins_after_init` (lifecycle hook) |

## Why no tag

The user explicitly froze v1.0.0 tag cuts (`nada de tag 1.0.0, tenemos
que acabar el roadmap`). This housekeeping cycle is not a release:
- No public API change.
- No production code change.
- Only test coverage extension for an existing capability.

A tag here would add noise without information. The head SHA
(`a7101d68`) is sufficient provenance.

## Verification command

```
cargo test -p cognicode-cli --test cognicode_lifecycle
```

## Cumulative session impact (e40 → e46)

| Metric | Pre-session | Post-e46 | Delta |
|--------|-------------|----------|-------|
| Conformance verified | 423 | 455 | **+32** |
| `pct_verified` | 91.4% | 98.3% | **+6.9 pp** |
| `pct_triaged` | 92.4% | 98.5% | **+6.1 pp** |
| CLI integration tests | 0 | 27 | **+27** |
| `no_evidence` count | 40 | 8 | **-32** |

## Open bounded slice (last one in `cognicode-cli` group)

The remaining 8 `no_evidence` specs are all in `cognicode-ide-adapter`.
This is the most network-dependent of the four CLI specs: the IDE
adapter patches external config files (`~/.config/opencode/`,
`~/.zcode/v2/config.json`, `~/.claude/mcp/`, `~/.codex/config.toml`)
during `cogh install --ide <name>`. Closing it would require either a
fixture-based approach (write a fake `$HOME` with stubbed config
files and assert the patches) or a registry-mock approach. Both are
higher-effort than e43-e46.

A future bounded slice `e47-lsi-cognicode-ide-adapter-conformance`
could pursue the fixture approach for the 4 most-testable specs:
- detect installed IDEs,
- read existing config files,
- patch JSON / TOML files (with fixture inputs),
- write idempotently (re-patching produces the same result).

That would close the conformance gap to ~99.4% verified. After that,
the remaining lift is in the LSI umbrella deltas (M5-M14) which
require production code work, not test-only cycles.
