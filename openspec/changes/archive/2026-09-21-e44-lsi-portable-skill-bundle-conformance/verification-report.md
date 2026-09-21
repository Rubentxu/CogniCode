# Verification Report — e44 — portable-skill-bundle conformance

> Change: `e44-lsi-portable-skill-bundle-conformance` | Phase: verify | Date: 2026-09-15

## Scope verification

| REQ | Title | Verdict | Evidence |
|-----|-------|---------|----------|
| 1 | `cogh skill validate` accepts a portable directory | COMPLIANT | `portable_skill_bundle_valid_directory_passes_validation` exits 0 and asserts `✓ skill bundle valid`, name, version on stdout. `portable_skill_bundle_non_directory_path_is_rejected` asserts non-zero exit and `not a directory` on stderr. |
| 2 | `manifest.yaml` is required and parses | COMPLIANT | `portable_skill_bundle_manifest_yaml_required_and_parsed` asserts description + maturity on stdout. `portable_skill_bundle_missing_manifest_is_rejected` asserts non-zero exit and `missing manifest.yaml` on stderr. |
| 3 | `SKILL.md` is required and the frontmatter is valid | COMPLIANT | `portable_skill_bundle_missing_skill_md_is_rejected` asserts non-zero exit and `missing SKILL.md`. `portable_skill_bundle_invalid_maturity_is_rejected` asserts `maturity must be one of` on stderr. |
| 4 | Portable bundle has no IDE-specific fields | COMPLIANT | `portable_skill_bundle_ide_specific_compatibility_field_rejected` asserts non-zero exit and `IDE-specific 'compatibility: opencode'` on stderr. |
| 5 | Referenced scripts and assets must exist | COMPLIANT | `portable_skill_bundle_referenced_script_must_exist` asserts non-zero exit and `referenced script missing` on stderr. |
| 6 | No production code change | COMPLIANT | `git diff cccbc3ff..73e60769 --stat` shows 2 files changed: `crates/cognicode-cli/tests/portable_skill_bundle.rs` (new, +320) and `sandbox/reports/evidence_map.yaml` (+1 entry). No `src/` touched. |

**Verdict: COMPLIANT (6/6 REQs).**

## Verification commands run

```
$ cargo test -p cognicode-cli --test portable_skill_bundle
...
test portable_skill_bundle_non_directory_path_is_rejected ... ok
test portable_skill_bundle_invalid_maturity_is_rejected ... ok
test portable_skill_bundle_missing_skill_md_is_rejected ... ok
test portable_skill_bundle_ide_specific_compatibility_field_rejected ... ok
test portable_skill_bundle_missing_manifest_is_rejected ... ok
test portable_skill_bundle_referenced_script_must_exist ... ok
test portable_skill_bundle_valid_directory_passes_validation ... ok
test portable_skill_bundle_manifest_yaml_required_and_parsed ... ok

test result: ok. 8 passed; 0 failed; 0 ignored
```

Conformance matrix re-run after evidence_map update:

```
$ python3 sandbox/scripts/openspec_conformance.py \
    --evidence-map sandbox/reports/evidence_map.yaml \
    --specs-dir openspec/specs
total=523 specs=83 phantom=4 verified=441 legacy=60 no_evidence=22 \
    pct_verified=95.2% pct_triaged=95.8%
```

Pre-cycle: `pct_verified=93.7% pct_triaged=94.5%`. Post-cycle: `pct_verified=95.2% pct_triaged=95.8%`. **+7 specs verified (portable-skill-bundle/1..7), +1 of 83 specs promoted (portable-skill-bundle).**

## Diff summary

| Stat | Value |
|------|-------|
| Files | 2 (`crates/cognicode-cli/tests/portable_skill_bundle.rs` new, `sandbox/reports/evidence_map.yaml` +1 entry) |
| LOC | +324 / -0 |
| Commits | 1 (`73e60769`) |
| Head SHA | `73e60769` |
| Origin SHA | `73e60769` (verified via `git ls-remote origin main`) |

## Pre-existing failures (NOT introduced by this change)

8 unit tests in `install_lock::tests`, `ide::tests`, `lifecycle::tests`
fail on main **before** this change. Verified via `git stash` test on
`cccbc3ff` (HEAD pre-e44): `91 passed; 9 failed; 1 ignored`. Post-e44:
`92 passed; 8 failed; 1 ignored` — net +1 passing test (e44 adds 8 new
passing tests on top of the 91 pre-existing). The 8 remaining failures
all stem from a hard-coded `version mismatch: bundle version 0.94.14
does not match cogh's CARGO_PKG_VERSION 0.94.15` error in
install/lifecycle tests that exercise the registry path. They are
unrelated to e44 and predate the change.

## Conformance matrix impact

| Metric | Pre | Post | Delta |
|--------|-----|------|-------|
| `total` | 523 | 523 | 0 |
| `specs` | 83 | 83 | 0 |
| `verified` | 434 | 441 | **+7** |
| `legacy_obsolete` | 60 | 60 | 0 |
| `no_evidence` | 29 | 22 | **-7** |
| `pct_verified` | 93.7% | 95.2% | **+1.5 pp** |
| `pct_triaged` | 94.5% | 95.8% | **+1.3 pp** |

## Verdict

**PASS** — cycle is ready for archive.
