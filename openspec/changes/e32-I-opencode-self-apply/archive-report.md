# Archive Report: e32-I-opencode-self-apply

**Cycle ID**: `e32i_opencode_self_apply`
**Archived**: 2026-08-16
**Branch**: `main` (tip = bdadba1c)
**Path**: A-min
**Mode**: Standard

---

## Cycle Summary

E32-I delivered two fixes to `cogh install --ide opencode`:

1. **Dispatch-order fix**: `cogh install --ide <name> --profile <p>` now calls `install::run_install` first, then dispatches the IDE adapter (previously `--profile` was silently dropped when `--ide` was set).

2. **Absolute shim path**: `install::run_install` and `cmd_ide_install` both resolve `~/.cognicode/shims/cognicode-mcp` via `home.shim_path("cognicode-mcp")`, honouring `COGNICODE_HOME`. The MCP process spawner no longer depends on `~/.cognicode/shims` being on `PATH`.

**Verdict**: `PASS WITH WARNINGS` — 6/8 spec scenarios PASS at runtime; 2 are UNTESTED (test-coverage gaps, not behavioural defects). 0 CRITICAL, 1 WARNING (pre-existing `test_clean_home_install` failure unrelated to E32-I).

---

## Deltas Applied (MODIFIED Requirements)

### 1. `openspec/specs/cognicode-cli/spec.md`

**Requirement**: `cogh install` registers MCP server with IDEs
**Action**: MODIFIED — requirement text updated + 1 new scenario added

| Field | Change |
|-------|--------|
| Requirement text | Clarified dispatch behaviour; added item 5 (absolute shim path at `command[0]`); added `--profile` ordering rule |
| Scenario `cogh install --ide opencode` | Updated to assert `command[0]` equals absolute shim path |
| Scenario `cogh install --ide opencode --profile core` | **NEW** — asserts both bundle install + IDE adapter dispatch |

Scenarios preserved (no change): zcode, claude, codex, all.

### 2. `openspec/specs/cognicode-ide-adapter/spec.md`

**Requirement**: Adapter manifest declares integrate / uninstall steps
**Action**: MODIFIED — manifest template updated + 2 new scenarios added

| Field | Change |
|-------|--------|
| Manifest `mcp` step template | Changed `command` from `$COGNICODE_HOME/shims/cognicode-mcp` to `~/.cognicode/shims/cognicode-mcp` (resolved absolute path) |
| Scenario `cogh install --ide opencode` | Updated to assert `command[0]` equals absolute shim path |
| Scenario opencode integrate fresh entry | **NEW** — asserts shim path + `type: "stdio"` on a config with no prior entry |
| Scenario opencode integrate stale entry | **NEW** — asserts stale `/bin/cognicode-mcp` is replaced with resolved shim path |

---

## Verify Report Reference

- **File**: `openspec/changes/e32-I-opencode-self-apply/verify-report.md`
- **Verdict**: `PASS WITH_WARNINGS`
- **Scenarios**: 6 PASS / 2 UNTESTED
- **Test coverage**: 19/20 `lifecycle::tests` pass; 20/20 `ide::tests` pass
- **Issues**: 0 CRITICAL, 1 WARNING (pre-existing `test_clean_home_install` failure)

---

## Knowledge Impact

| Category | Impact |
|----------|--------|
| Specs updated | `openspec/specs/cognicode-cli/spec.md`, `openspec/specs/cognicode-ide-adapter/spec.md` |
| Specs made stale | None |
| ADRs superseded | None |
| Requirements touched | `cogh install registers MCP server with IDEs`, `Adapter manifest declares integrate / uninstall steps` |
| Jurisprudence candidate | No (no new requirements; only corrections to existing ones) |

---

## Ledger Status

**SKIPPED** — SDDK ledger CLI is broken (UNIQUE constraint / STORAGE_NOT_FOUND / FOREIGN KEY). Per the manual workaround documented in prior cycles, this archive report is written as a regular file and committed locally. No `sddk cycle` CLI calls were made.

---

## Files Changed in This Commit

| File | Change |
|------|--------|
| `openspec/specs/cognicode-cli/spec.md` | MODIFIED requirement + 1 new scenario |
| `openspec/specs/cognicode-ide-adapter/spec.md` | MODIFIED manifest template + 2 new scenarios |
| `openspec/changes/e32-I-opencode-self-apply/archive-report.md` | Created |

---

## Commit

```
chore(archive): E32-I sync delta specs to canonical
```

---

## Next Recommended Step

`/sddk-release` for `e32i_opencode_self_apply` — bumps version to `v0.94.14` and tags `main`.
