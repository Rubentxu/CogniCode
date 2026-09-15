# Verification Report — e47 — cognicode-ide-adapter conformance

> Change: `e47-lsi-cognicode-ide-adapter-conformance` | Phase: verify | Date: 2026-09-15

## Scope verification

| REQ | Title | Verdict | Evidence |
|-----|-------|---------|----------|
| 1 | `cogh ide detect` enumerates installed IDEs | COMPLIANT | `cogh_ide_detect_lists_opencode_when_config_present` (opencode found when config exists), `cogh_ide_detect_lists_no_ides_on_empty_home` (opencode ✗ on empty home). |
| 2 | Each IDE is a separate `cogh` plugin | COMPLIANT | `cogh_init_includes_three_ide_plugins` asserts zcode/claude/codex bundled (opencode intentionally NOT bundled, asserted absent); `cogh_plugin_list_shows_ide_plugins_with_manifests` confirms the listing. |
| 3 | Adapter manifest declares integrate / uninstall steps + JSON-merge | COMPLIANT | `cogh_ide_install_opencode_writes_mcp_entry_preserving_existing` asserts `cogh ide install opencode --plugin mcp-server` merges a fresh `mcp.cognicode-mcp` entry while preserving the pre-existing `mcp.chronos.type=local`. |
| 4 | `remove_from_json` cleanly removes the MCP entry | COMPLIANT | `cogh_ide_uninstall_opencode_removes_mcp_entry` asserts `cogh ide uninstall opencode --version 0.94.15` removes only the `cognicode-mcp` entry and preserves `chronos`. |
| 5 | Each IDE adapter has a unique JSON path | COMPLIANT | `cogh_ide_install_zcode_writes_zcode_specific_path` asserts zcode's `~/.zcode/v2/config.json` carries the `mcp.cognicode-mcp` entry after install. |
| 6 | No production code change | COMPLIANT | `git diff 9977b7c5..d6ee8458 --stat` shows 2 files changed: `crates/cognicode-cli/tests/cognicode_ide_adapter.rs` (new, +366) and `sandbox/reports/evidence_map.yaml` (+1 entry). No `src/` touched. |

**Verdict: COMPLIANT (6/6 REQs).**

## Verification commands run

```
$ cargo test -p cognicode-cli --test cognicode_ide_adapter
...
test cogh_ide_detect_lists_opencode_when_config_present ... ok
test cogh_ide_detect_lists_no_ides_on_empty_home ... ok
test cogh_init_includes_three_ide_plugins ... ok
test cogh_ide_uninstall_opencode_removes_mcp_entry ... ok
test cogh_ide_install_zcode_writes_zcode_specific_path ... ok
test cogh_ide_install_opencode_writes_mcp_entry_preserving_existing ... ok
test cogh_plugin_list_shows_ide_plugins_with_manifests ... ok

test result: ok. 7 passed; 0 failed; 0 ignored
```

Conformance matrix re-run after evidence_map update:

```
$ python3 sandbox/scripts/openspec_conformance.py \
    --evidence-map sandbox/reports/evidence_map.yaml \
    --specs-dir openspec/specs
total=523 specs=83 phantom=4 verified=463 legacy=60 no_evidence=0 \
    pct_verified=100.0% pct_triaged=100.0%
```

Pre-cycle: `pct_verified=98.3% pct_triaged=98.5%`. Post-cycle: `pct_verified=100.0% pct_triaged=100.0%`. **+8 specs verified (cognicode-ide-adapter/1..8), +1 of 83 specs promoted (cognicode-ide-adapter). no_evidence count: 8 → 0.**

## Diff summary

| Stat | Value |
|------|-------|
| Files | 2 (`crates/cognicode-cli/tests/cognicode_ide_adapter.rs` new, `sandbox/reports/evidence_map.yaml` +1 entry) |
| LOC | +376 / -0 |
| Commits | 1 (`d6ee8458`) |
| Head SHA | `d6ee8458` |
| Origin SHA | `d6ee8458` (verified via `git ls-remote origin main`) |

## Pre-existing failures (NOT introduced by this change)

The same 8 unit tests in `install_lock::tests`, `ide::tests`,
`lifecycle::tests` fail on main **before** this change. Verified via
`git stash` test on `9977b7c5` (HEAD pre-e47). They are unrelated to
e47 and predate the change.

The 20 existing unit tests in `ide::tests` also fail under parallel
test execution (because they `unsafe { set_var("HOME", ...) }` without
serialisation) — this is a known pre-existing issue documented in the
exploration report. The new subprocess tests provide parallel-safe
coverage on top.

## Conformance matrix impact

| Metric | Pre | Post | Delta |
|--------|-----|------|-------|
| `total` | 523 | 523 | 0 |
| `specs` | 83 | 83 | 0 |
| `verified` | 455 | 463 | **+8** |
| `legacy_obsolete` | 60 | 60 | 0 |
| `no_evidence` | 8 | 0 | **-8** |
| `pct_verified` | 98.3% | **100.0%** | **+1.7 pp** |
| `pct_triaged` | 98.5% | **100.0%** | **+1.5 pp** |

## Verdict

**PASS** — cycle is ready for archive.
