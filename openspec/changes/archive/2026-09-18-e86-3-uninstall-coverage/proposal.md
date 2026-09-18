# E86.3 — Uninstall coverage

> Status: **PASS**
> Closure date: 2026-09-18
> Apply commit: `41818bf3`
> Binary SHA-256: `39c6bbafb096bab2b68e5362f568a4818ce3c92e4b7935de5cab468c3e9cff99`

## Origin

The E86.2.2 + E86.2.3 real-PC UAT script runs `cogh uninstall` in
Phase D against a tmp HOME that gets rolled back between Phase B and
Phase D. With the original `cmd_uninstall`, that uninstall silently
no-oped because:

1. There was no `home.is_initialized()` guard (while `cmd_install`
   had one), so a wiped home looked identical to a freshly-initialised
   one.
2. There was no diagnostic for the `ides=[]` case — uninstall exited 0
   with a one-liner that gave the user no hint that nothing happened.

Two RED tests pinned both gaps. Both now GREEN.

## What changed

Single file edit: `crates/cognicode-cli/src/cmd/layout.rs::cmd_uninstall`.

```rust
pub fn cmd_uninstall(
    home: &CognicodeHome,
    plugin: &str,
    version: &str,
    ides: &[String],
) -> Result<()> {
    if !home.is_initialized() {
        return Err(anyhow!(
            "home not initialized at {}; run `cogh init` first",
            home.root.display()
        ));
    }
    if ides.is_empty() {
        return Err(anyhow!(
            "uninstall requires at least one --ide flag (e.g. --ide opencode); \
             supported: opencode, zcode, claude, codex"
        ));
    }
    println!(/* ... */);
    for ide in ides { crate::ide::cmd_ide_uninstall(home, ide, version)?; }
    Ok(())
}
```

Five new tests in `cmd/lifecycle.rs::tests::`:

| ID | Name | Status before | Status after |
|---|---|---|---|
| T1 | `t_e86_3_uninstall_errors_on_uninitialized_home` | RED | GREEN |
| T2 | `t_e86_3_uninstall_idempotent_second_call` | already PASS | pinned |
| T3 | `t_e86_3_uninstall_opencode_handles_missing_config_file` | already PASS | pinned |
| T4 | `t_e86_3_uninstall_without_ide_prints_helpful_message` | RED | GREEN |
| T5 | `t_e86_3_uninstall_unknown_ide_errors_cleanly` | already PASS | pinned |

T2 / T3 / T5 were already correct (the `Step::RmRf` + `Step::RemoveFromJson`
machinery is idempotent, and `cmd_ide_uninstall`'s unknown-IDE branch was
already clean). Pinning them as RED-shaped tests protects the contract from
future regressions — they would have caught the bug pattern of "small change
to uninstall accidentally breaks idempotency."

## Verification

* `cargo test -p cognicode-cli --bin cogh`: **207 passed / 0 failed /
  1 ignored** (baseline 202 + 5 new).
* `test_install_with_ide_and_profile_dispatches_both` (E86.2.2 regression
  guard): PASS.
* 9 E86.2.2 env-split tests: PASS.
* 4 E86.2.3 IDE-resolver tests: PASS.
* 1 pre-existing E32-H uninstall test (`uninstall_opencode_ide_removes_entry_and_skills`):
  PASS — proves the guards don't break the happy path.
* `cargo fmt -p cognicode-cli`: clean.
* `cargo check --workspace`: clean.
* `python3 scripts/check_known_failures.py`: 41 entries, baseline
  unchanged.
* `cargo clippy -p cognicode-cli --bin cogh`: **0 new errors**. The 134
  pre-existing warnings are all unused imports / dead code in baseline;
  out of scope.

## Real-PC UAT re-run

`UAT_ROOT=/tmp/cogh-uat-real-pc-863 COGH_BIN=target/release/cogh bash /tmp/cogh-uat-real-pc.sh`:

* `overall_status: ok`.
* Phase A binary_check: `ok` (cogh 0.95.0).
* Phase B bundle_prefetch + sha256 + install: `ok`.
* Phase C list / latest / where / doctor: all `done`.
* Phase D rollback: `ok`.
* Phase D reinstall: `fail` (pre-existing layout bug — `install/{ver}/`
  vs `versions/{ver}/` — still filed as an E86.2.3 follow-up).
* **Phase D uninstall: `ok`** — prints `uninstall: plugin=mcp-server
  version=0.95.0 ides=["opencode"]` then `✓ OpenCode uninstall complete`.

Pre/post HOME snapshot of `~/.config/opencode/`:

```
pre  symlink target: /home/rubentxu/.cognicode/install/0.95.0/mcp-server/skills
post symlink target: /home/rubentxu/.cognicode/install/0.95.0/mcp-server/skills
pre  config sha256:  501c92bb75cf600c79d7ed56c1722281ee204b70dc9a31e1b40cc07c7134f737
post config sha256:  501c92bb75cf600c79d7ed56c1722281ee204b70dc9a31e1b40cc07c7134f737
pre  config mtime:   1789722950
post config mtime:   1789722950
```

Zero pollution.

## Out of scope (filed for a later cycle)

The E86.2.3 follow-ups remain:

1. `install.rs::run_install` — skills source path uses
   `cognicode-home/install/{ver}/...` but the real layout is
   `cognicode-home/versions/{ver}/...`. Symptom: `link_or_copy failed`
   in Phase D reinstall of the UAT.
2. `ide::detect_opencode` should use `Path::is_file()` not
   `Path::exists()` so Phase D reinstall does not falsely trigger a
   second integration.

Plus one E86.3 candidate not pursued here:

3. `cmd_uninstall` does not remove the install tree at
   `versions/{ver}/` or `install/{ver}/`. This is a deliberate scope
   decision: "remove IDE integration on this version" (which E86.3
   closes) is a different lifecycle question from "undo an install"
   (which would need a journal-aware, multi-step transaction). Tabled
   until the lifecycle journal work has a recipient transaction type.

## Cycle verdict

**PASS**. Bounded cycle. Two files changed, 215 insertions, 0 deletions.
No new dependencies, no new crate, no new filesystem abstraction.
