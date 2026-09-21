# Verification Report — e43 — cogh CLI conformance coverage

> Change: `e43-lsi-cogh-cli-conformance-coverage` | Phase: verify | Date: 2026-09-15

## Scope verification

| REQ | Title | Verdict | Evidence |
|-----|-------|---------|----------|
| 1 | `cogh --version` reports the binary version | COMPLIANT | `cogh_version_reports_semver_token` runs `cogh --version`, asserts exit 0 and `cogh <semver>` shape on stdout. |
| 2 | `cogh list` renders the plugin table | COMPLIANT | `cogh_list_on_uninitialised_home_reports_not_initialized` covers the uninitialised branch (REQ scenario 1); `cogh_list_on_initialised_home_renders_table` covers the table-render branch (REQ scenario 2). |
| 3 | `cogh current` reads the tracker | COMPLIANT | `cogh_current_with_no_pinned_version_reports_unset` covers the absent-tracker branch (REQ scenario 1); `cogh_current_reads_pinned_version_from_tracker` covers the populated-tracker branch (REQ scenario 2). |
| 4 | `cogh doctor` validates installation | COMPLIANT | `cogh_doctor_on_uninitialised_home_reports_not_initialized` covers the uninitialised branch (REQ scenario 1); `cogh_doctor_on_initialised_home_reports_healthy` covers the healthy-home branch (REQ scenario 2). |
| 5 | No production code change | COMPLIANT | `git diff 76662c88..02e92f87 --stat` shows 2 files changed: `crates/cognicode-cli/tests/cogh_cli.rs` (new, +255) and `sandbox/reports/evidence_map.yaml` (1 entry added). No `src/` touched. |

**Verdict: COMPLIANT (5/5 REQs).**

## Verification commands run

```
$ cargo test -p cognicode-cli --test cogh_cli
...
test cogh_doctor_on_uninitialised_home_reports_not_initialized ... ok
test cogh_list_on_uninitialised_home_reports_not_initialized ... ok
test cogh_current_with_no_pinned_version_reports_unset ... ok
test cogh_version_reports_semver_token ... ok
test cogh_current_reads_pinned_version_from_tracker ... ok
test cogh_list_on_initialised_home_renders_table ... ok
test cogh_doctor_on_initialised_home_reports_healthy ... ok

test result: ok. 7 passed; 0 failed; 0 ignored
```

Conformance matrix re-run after evidence_map update:

```
$ python3 sandbox/scripts/openspec_conformance.py \
    --evidence-map sandbox/reports/evidence_map.yaml \
    --specs-dir openspec/specs
total=523 specs=83 phantom=4 verified=434 legacy=60 no_evidence=29 \
    pct_verified=93.7% pct_triaged=94.5%
```

Pre-cycle: `pct_verified=91.4% pct_triaged=92.4%`. Post-cycle: `pct_verified=93.7% pct_triaged=94.5%`. **+11 specs verified, +4 of 83 specs promoted (cognicode-cli/1..11).**

## Diff summary

| Stat | Value |
|------|-------|
| Files | 2 (`crates/cognicode-cli/tests/cogh_cli.rs` new, `sandbox/reports/evidence_map.yaml` +1 entry) |
| LOC | +259 / -0 |
| Commits | 1 (`02e92f87`) |
| Head SHA | `02e92f87` |
| Origin SHA | `02e92f87` (verified via `git ls-remote origin main`) |

## Pre-existing failures (NOT introduced by this change)

The full `cargo test -p cognicode-core --lib` run on main had 39 failures in `application::services::file_operations` **before** this change. Verified via `git stash` before commit `02e92f87`. This change does NOT touch `cognicode-core` and does NOT introduce any new failure.

## Conformance matrix impact

| Metric | Pre | Post | Delta |
|--------|-----|------|-------|
| `total` | 523 | 523 | 0 |
| `specs` | 83 | 83 | 0 |
| `verified` | 423 | 434 | **+11** |
| `legacy_obsolete` | 60 | 60 | 0 |
| `no_evidence` | 40 | 29 | **-11** |
| `pct_verified` | 91.4% | 93.7% | **+2.3 pp** |
| `pct_triaged` | 92.4% | 94.5% | **+2.1 pp** |

## Verdict

**PASS** — cycle is ready for archive.
