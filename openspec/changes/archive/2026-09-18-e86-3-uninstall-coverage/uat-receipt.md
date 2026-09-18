# E86.3 — Receipt (UAT re-run + per-test PASS table)

## Per-test PASS table

| Test | Phase | Pre-fix | Post-fix | Evidence |
|---|---|---|---|---|
| `t_e86_3_uninstall_errors_on_uninitialized_home` | WU1 RED → WU2 GREEN | FAIL (exit 0, "✓ OpenCode uninstall complete") | PASS (exit 1, "home not initialized at /tmp/...; run `cogh init` first") | direct sub-process test, observed stdout/stderr |
| `t_e86_3_uninstall_idempotent_second_call` | WU1 → WU2 pinned | already PASS | PASS | sub-process twice, both exit 0 |
| `t_e86_3_uninstall_opencode_handles_missing_config_file` | WU1 → WU2 pinned | already PASS | PASS | sub-process with no opencode config file, exit 0 |
| `t_e86_3_uninstall_without_ide_prints_helpful_message` | WU1 RED → WU2 GREEN | FAIL (exit 0, "uninstall: ... ides=[]") | PASS (exit 1, "uninstall requires at least one --ide flag") | direct sub-process test |
| `t_e86_3_uninstall_unknown_ide_errors_cleanly` | WU1 → WU2 pinned | already PASS | PASS | sub-process with `--ide vscode`, exit 1 with "not supported" |

## Regression guard tests (untouched, all PASS)

| Test | Cycle | Status |
|---|---|---|
| `test_install_with_ide_and_profile_dispatches_both` | E86.2.2 | PASS |
| 9 × `t_e86_2_2_*_env_var_split_*` | E86.2.2 | PASS |
| 4 × `t_e86_2_3_*_opencode_*` | E86.2.3 | PASS |
| `uninstall_opencode_ide_removes_entry_and_skills` | E32-H | PASS |
| 198 cogh tests baseline | various | PASS |

## Real-PC UAT (WU4)

Script: `/tmp/cogh-uat-real-pc.sh` (re-uses E86.2.2 UAT, Phase A → D).

```
COGH_BIN: /var/home/rubentxu/cargo-targets/release/cogh
UAT_ROOT: /tmp/cogh-uat-real-pc-863
binary SHA-256: 39c6bbafb096bab2b68e5362f568a4818ce3c92e4b7935de5cab468c3e9cff99
release_status: not published (local-only commit, not tagged, not pushed)
```

Phase outcomes:

* Phase A preflight: `ok`.
* Phase B bundle pre-fetch + sha256 + install: `ok`.
* Phase C list / latest / where / doctor: `done` (one doctor sub-test
  FAIL on `MCP: cognicode-mcp binary not found`, which is a profile
  choice unrelated to E86.3 — the doctor reads `home.bin()` for the
  mcp-server binary which is not built by `cogh install mcp-server`).
* Phase D rollback: `ok`.
* Phase D reinstall: `fail` (pre-existing layout bug, filed in
  E86.2.3 follow-ups).
* **Phase D uninstall: `ok`** — proves the bounded surface of E86.3
  lands end-to-end against the real release binary.

Pre/post HOME snapshot of `~/.config/opencode/`: identical sha256, mtime,
symlink target. **Zero pollution.**

## Cycle verdict

**PASS**. Two-file bounded change, 215 insertions, 0 deletions. No new
dependencies, no new crate, no new filesystem abstraction.

## What this cycle did NOT do (honest accounting)

* Did NOT remove the install tree under `versions/{ver}/` or
  `install/{ver}/` when uninstalling. That's a different lifecycle
  question (it requires journal-aware multi-step transactions) and is
  explicitly out of scope here.
* Did NOT fix the Phase D reinstall `link_or_copy failed` error —
  that's the pre-existing `install/{ver}/...` vs `versions/{ver}/...`
  layout bug filed in the E86.2.3 follow-ups.
* Did NOT add any IDE-specific uninstall tests beyond opencode (the
  opencode path is the one the UAT exercises; zcode/claude/codex
  already have their own per-IDE unit tests in `cmd/ide.rs`).
