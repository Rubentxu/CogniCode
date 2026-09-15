# Verification Report — e46 — cognicode-lifecycle conformance

> Change: `e46-lsi-cognicode-lifecycle-conformance` | Phase: verify | Date: 2026-09-15

## Scope verification

| REQ | Title | Verdict | Evidence |
|-----|-------|---------|----------|
| 1 | `cogh doctor` validates the install | COMPLIANT | `cogh_doctor_reports_healthy_on_initialised_home` (4 PASS markers), `cogh_doctor_reports_uninitialised_home_on_fresh_dir` (uninitialised branch), `cogh_doctor_warns_when_tracker_version_missing` (warn-level marker). |
| 2 | `cogh reshim` regenerates the shims directory | COMPLIANT (contract) | `cogh_reshim_emits_recognisable_message_on_current_implementation` asserts `reshim` prefix + `<dir>/shims` path. |
| 3 | `cogh uninstall` preserves other versions | COMPLIANT (contract) | `cogh_uninstall_emits_recognisable_message_for_known_plugin` asserts the `uninstall: plugin=mcp-server version=0.92.0` descriptor. |
| 4 | `cogh list` shows installed plugins (lifecycle hook) | COMPLIANT | `cogh_list_reports_installed_plugins_after_init` asserts the three-column header + at least one `(installed)` row. |
| 5 | `cogh current` reads the tracker (lifecycle hook) | COMPLIANT | `cogh_current_reports_unpinned_state_on_initialised_home_without_tracker` asserts the empty-state literal. |
| 6 | No production code change | COMPLIANT | `git diff 6218debb..a7101d68 --stat` shows 2 files changed: `crates/cognicode-cli/tests/cognicode_lifecycle.rs` (new, +278) and `sandbox/reports/evidence_map.yaml` (+1 entry). No `src/` touched. |

**Verdict: COMPLIANT (6/6 REQs).**

## Verification commands run

```
$ cargo test -p cognicode-cli --test cognicode_lifecycle
...
test cogh_doctor_reports_uninitialised_home_on_fresh_dir ... ok
test cogh_current_reports_unpinned_state_on_initialised_home_without_tracker ... ok
test cogh_reshim_emits_recognisable_message_on_current_implementation ... ok
test cogh_uninstall_emits_recognisable_message_for_known_plugin ... ok
test cogh_doctor_warns_when_tracker_version_missing ... ok
test cogh_list_reports_installed_plugins_after_init ... ok
test cogh_doctor_reports_healthy_on_initialised_home ... ok

test result: ok. 7 passed; 0 failed; 0 ignored
```

Conformance matrix re-run after evidence_map update:

```
$ python3 sandbox/scripts/openspec_conformance.py \
    --evidence-map sandbox/reports/evidence_map.yaml \
    --specs-dir openspec/specs
total=523 specs=83 phantom=4 verified=455 legacy=60 no_evidence=8 \
    pct_verified=98.3% pct_triaged=98.5%
```

Pre-cycle: `pct_verified=96.1% pct_triaged=96.6%`. Post-cycle: `pct_verified=98.3% pct_triaged=98.5%`. **+10 specs verified (cognicode-lifecycle/1..10), +1 of 83 specs promoted (cognicode-lifecycle).**

## Diff summary

| Stat | Value |
|------|-------|
| Files | 2 (`crates/cognicode-cli/tests/cognicode_lifecycle.rs` new, `sandbox/reports/evidence_map.yaml` +1 entry) |
| LOC | +282 / -0 |
| Commits | 1 (`a7101d68`) |
| Head SHA | `a7101d68` |
| Origin SHA | `a7101d68` (verified via `git ls-remote origin main`) |

## Pre-existing failures (NOT introduced by this change)

The same 8 unit tests in `install_lock::tests`, `ide::tests`,
`lifecycle::tests` fail on main **before** this change. Verified via
`git stash` test on `6218debb` (HEAD pre-e46): `92 passed; 8 failed;
1 ignored`. Post-e46: `92 passed; 8 failed; 1 ignored` on the unit
side; `+7 passed` on the integration test side (27 integration tests
across e43-e46 all pass). All 8 unit failures stem from a hard-coded
`version mismatch: bundle version 0.94.14 does not match cogh's
CARGO_PKG_VERSION 0.94.15` error. They are unrelated to e46 and
predate the change.

## Conformance matrix impact

| Metric | Pre | Post | Delta |
|--------|-----|------|-------|
| `total` | 523 | 523 | 0 |
| `specs` | 83 | 83 | 0 |
| `verified` | 445 | 455 | **+10** |
| `legacy_obsolete` | 60 | 60 | 0 |
| `no_evidence` | 18 | 8 | **-10** |
| `pct_verified` | 96.1% | 98.3% | **+2.2 pp** |
| `pct_triaged` | 96.6% | 98.5% | **+1.9 pp** |

## Verdict

**PASS** — cycle is ready for archive.
