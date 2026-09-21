# Archive Manifest — e32-I-opencode-self-apply

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/e32-I-opencode-self-apply` |
| Path | a-min |
| Date | 2026-09-21 |
| Actor | jcode-orchestrator |
| Status | **SUPERSEDED — `archive-with-release-receipt`** |
| Superseded at | 2026-09-21T06:45Z |

## Outcome

PASS WITH WARNINGS. Cycle delivered two fixes to `cogh install --ide opencode`:

1. **Dispatch-order fix** (commit `fbd8738b`): `cogh install --ide <name>
   --profile <p>` now calls `install::run_install` first, then dispatches
   the IDE adapter (previously `--profile` was silently dropped when
   `--ide` was set).

2. **Absolute shim path** (commit `fbd8738b`): `install::run_install` and
   `cmd_ide_install` both resolve `~/.cognicode/shims/cognicode-mcp`
   via `home.shim_path("cognicode-mcp")`, honouring `COGNICODE_HOME`.
   The MCP process spawner no longer depends on `~/.cognicode/shims`
   being on `PATH`.

## Verdict

`PASS WITH WARNINGS` — 6/8 spec scenarios PASS at runtime; 2 are
UNTESTED (test-coverage gaps, not behavioural defects). 0 CRITICAL,
1 WARNING (pre-existing `test_clean_home_install` failure unrelated
to E32-I).

## Release

- Tag: `v0.94.14`
- Tag SHA: `23c9e23d35b98695cc74902c4eed1d29cd5d243f` (annotated)
- Release commit: `e4f856adf87c4fc2c3fc0e2cbc39c45405ca7c81`
- Apply stack merge commit: `bdadba1c`

## Commits produced

| SHA | Subject |
|-----|---------|
| `fbd8738b` | fix(cogh): E32-I dispatch-order fix --ide now respects --profile |
| `91a8168a` | feat(cli): E35 IDE targeting - ZCode, Claude, Codex support |
| `51dc325a` | chore(fmt): apply rustfmt across cognicode-cli/core/macros/runtime |
| `b46bfbd6` | fix(cogh): correction pass for e34_plugin_cleanup |
| `e86af04c` | feat(cogh): e34 plugin cleanup — retire plugin registry dead code |
| `f3bdc7d9` | chore(verify): E32-I verify-report - PASS WITH WARNINGS |
| `e04d9ef6` | chore(spec): persist E32-I spec + tasks for cycle audit trail |
| `bdadba1c` | chore(merge): merge feat/e32-i-opencode-self-apply |
| `d7334c71` | chore(archive): E32-I sync delta specs to canonical |
| `1d85711b` | chore(merge): E32-I delta spec sync from feature branch |
| `e4f856ad` | chore(release): bump workspace version 0.94.11 → 0.94.14 |

## Deltas synced to canonical spec

- `openspec/specs/cognicode-cli/spec.md` — Requirement `cogh install
  registers MCP server with IDEs` MODIFIED + 1 new scenario (`--ide` +
  `--profile` dispatch)
- `openspec/specs/cognicode-ide-adapter/spec.md` — Requirement `Adapter
  manifest declares integrate / uninstall steps` MODIFIED + 2 new
  scenarios (fresh entry, stale entry replacement)

## Cross-references

- Cycle artifacts: `archive-report.md`, `release-receipt.md`, `spec.md`,
  `tasks.md`, `verify-report.md`
- Sequencer: completes the E32 distribution program (E32-A through E32-I
  per the E32 status section in `docs/ROADMAP.md`)
- Successor: e86-2..4 (cogh lifecycle), e87/e88 (channels/UAT), per the
  e84 distribution contract umbrella.

## Closure semantics

```text
implementation      CLOSED (real, on main)
release            DONE    (v0.94.14 tagged and pushed per release-receipt)
verification       PASS WITH WARNINGS (6/8 scenarios; 2 UNTESTED gaps)
archive closure    DONE    (this manifest)
```
