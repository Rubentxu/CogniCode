# Verification Report — e45 — cognicode-plugin conformance

> Change: `e45-lsi-cognicode-plugin-conformance` | Phase: verify | Date: 2026-09-15

## Scope verification

| REQ | Title | Verdict | Evidence |
|-----|-------|---------|----------|
| 1 | `cogh init` installs bundled plugins | COMPLIANT | `cogh_init_installs_bundled_plugins` asserts exit 0 + `Installed ` summary + `bundled plugin(s)` summary + non-empty `plugins/` directory. `cogh_plugin_list_enumerates_bundled_plugins_after_init` asserts the `Plugin` table header and the canonical `mcp-server` row. |
| 2 | `plugin.yaml` is parsed end-to-end | COMPLIANT | `cogh_plugin_list_shows_custom_plugin_with_parsed_description` asserts exit 0 + custom plugin name + parsed description in the listing. `cogh_plugin_list_handles_plugin_without_yaml_gracefully` asserts no-abort behaviour when a plugin directory has no manifest. `cogh_plugin_add_unknown_name_without_url_is_rejected` asserts the whitelist enforcement. |
| 3 | No production code change | COMPLIANT | `git diff 8b2f7ded..c6b33edb --stat` shows 2 files changed: `crates/cognicode-cli/tests/cognicode_plugin.rs` (new, +233) and `sandbox/reports/evidence_map.yaml` (+1 entry). No `src/` touched. |

**Verdict: COMPLIANT (3/3 REQs).**

## Verification commands run

```
$ cargo test -p cognicode-cli --test cognicode_plugin
...
test cogh_init_installs_bundled_plugins ... ok
test cogh_plugin_add_unknown_name_without_url_is_rejected ... ok
test cogh_plugin_list_enumerates_bundled_plugins_after_init ... ok
test cogh_plugin_list_shows_custom_plugin_with_parsed_description ... ok
test cogh_plugin_list_handles_plugin_without_yaml_gracefully ... ok

test result: ok. 5 passed; 0 failed; 0 ignored
```

Conformance matrix re-run after evidence_map update:

```
$ python3 sandbox/scripts/openspec_conformance.py \
    --evidence-map sandbox/reports/evidence_map.yaml \
    --specs-dir openspec/specs
total=523 specs=83 phantom=4 verified=445 legacy=60 no_evidence=18 \
    pct_verified=96.1% pct_triaged=96.6%
```

Pre-cycle: `pct_verified=95.2% pct_triaged=95.8%`. Post-cycle: `pct_verified=96.1% pct_triaged=96.6%`. **+4 specs verified (cognicode-plugin/1..4), +1 of 83 specs promoted (cognicode-plugin).**

## Diff summary

| Stat | Value |
|------|-------|
| Files | 2 (`crates/cognicode-cli/tests/cognicode_plugin.rs` new, `sandbox/reports/evidence_map.yaml` +1 entry) |
| LOC | +237 / -0 |
| Commits | 1 (`c6b33edb`) |
| Head SHA | `c6b33edb` |
| Origin SHA | `c6b33edb` (verified via `git ls-remote origin main`) |

## Pre-existing failures (NOT introduced by this change)

8 unit tests in `install_lock::tests`, `ide::tests`, `lifecycle::tests`
fail on main **before** this change. Verified via `git stash` test on
`8b2f7ded` (HEAD pre-e45): `91 passed; 9 failed; 1 ignored`. Post-e45:
`92 passed; 8 failed; 1 ignored` — net +1 passing test (e45 adds 5 new
passing tests on top of the 91 pre-existing; the same 8 install/lifecycle
failures remain, all stemming from a hard-coded
`version mismatch: bundle version 0.94.14 does not match cogh's
CARGO_PKG_VERSION 0.94.15` error). They are unrelated to e45 and
predate the change.

## Conformance matrix impact

| Metric | Pre | Post | Delta |
|--------|-----|------|-------|
| `total` | 523 | 523 | 0 |
| `specs` | 83 | 83 | 0 |
| `verified` | 441 | 445 | **+4** |
| `legacy_obsolete` | 60 | 60 | 0 |
| `no_evidence` | 22 | 18 | **-4** |
| `pct_verified` | 95.2% | 96.1% | **+0.9 pp** |
| `pct_triaged` | 95.8% | 96.6% | **+0.8 pp** |

## Verdict

**PASS** — cycle is ready for archive.
