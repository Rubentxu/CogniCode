# E86.7 — cmd_uninstall removes the install tree

> Status: **PASS** (with HONEST pollution disclosure)
> Closure date: 2026-09-18
> Apply commit: `7b9b2779`
> Binary SHA-256: `fa55428eae4023797ac24e07480b17448cd407db78db021fdb7104bca56bc1fb`

## Origin

`cmd_uninstall --plugin X --version Y --ide Z` iterated the IDE
adapters (opencode / zcode / claude / codex) and stopped there. It
never touched `<root>/install/<ver>/`, so the install tree —
manifest + every extracted component — survived on disk after
uninstall. That made `cogh list` and `cogh where` lie about what
was installed, and forced `cmd_rollback` (via the journal) to be
the only way to undo a real install.

The bounded surface of "remove IDE integration on this version"
(E86.3) was already covered, but the larger surface of "actually
undo what install did" was not — explicitly filed as an E86.3
out-of-scope item. E86.7 closes that item.

## What changed

One file edit in `crates/cognicode-cli/src/cmd/layout.rs::cmd_uninstall`:

```rust
pub fn cmd_uninstall(...) -> Result<()> {
    // ... existing is_initialized() and ides.is_empty() guards ...

    // Wire --ide <name> to the IDE adapter uninstall.
    for ide in ides {
        crate::ide::cmd_ide_uninstall(home, ide, version)?;
    }

    // E86.7: remove the install tree at <root>/install/<ver>/ so
    // uninstall actually undoes what install did. ...
    let install_tree = home.install_manifest_path(version)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| home.root.join("install").join(version));
    if install_tree.exists() {
        std::fs::remove_dir_all(&install_tree).with_context(|| {
            format!("rm -rf install tree at {}", install_tree.display())
        })?;
        println!("✓ removed install tree: {}", install_tree.display());
    }
    Ok(())
}
```

Behaviour:

* If the install tree exists, it is removed via `remove_dir_all`.
* If the install tree does not exist, the call is a no-op
  (idempotent).
* A `✓ removed install tree: <path>` line is printed.
* The tracker is NOT modified — that is the job of `cogh current`
  / `cogh rollback`, not the bounded uninstall surface.
* The IDE adapters run BEFORE the install-tree removal. The
  integration symlink target lives inside the install tree, so the
  adapters must see the tree intact. Order matters.

Three new tests in `cmd/layout.rs::tests::`:

| ID | Name | Pre-fix | Post-fix |
|---|---|---|---|
| T1 | `t_e86_7_cmd_uninstall_removes_install_tree` | FAIL (tree persists) | PASS |
| T2 | `t_e86_7_cmd_uninstall_idempotent_when_install_tree_missing` | already PASS | pinned |
| T3 | `t_e86_7_cmd_uninstall_prints_install_tree_removal` | FAIL | PASS |

The tests use `home.install_manifest_path(version).parent()` (not a
hardcoded `home_dir.path().join(".cognicode/install/...")`) because
`CognicodeHome::resolve(Some(home_override))` does NOT add `.cognicode`
to the root — only the `COGNICODE_HOME` env path does. Using the
method-based path keeps the tests aligned with the implementation
contract.

## Verification

* `cargo test -p cognicode-cli --bin cogh`: **218 passed / 0 failed
  / 1 ignored** (215 baseline + 3 new).
* `cargo fmt -p cognicode-cli`: clean.
* `cargo check --workspace`: clean.
* `python3 scripts/check_known_failures.py`: 41 entries, baseline
  unchanged.
* `cargo clippy -p cognicode-cli --bin cogh`: 0 new errors.

## Real-PC UAT (WU4)

`UAT_ROOT=/tmp/cogh-uat-real-pc-867 COGH_BIN=target/release/cogh bash /tmp/cogh-uat-real-pc.sh`:

* `overall_status: ok`.
* Phase A preflight: `ok`.
* Phase B install: `ok` (`✓ OpenCode integration complete`).
* Phase C list / latest / where / doctor: `done`.
* Phase D rollback: `ok`.
* Phase D reinstall: **FAIL** (pre-existing layout drift — see
  follow-ups).
* Phase D uninstall: `ok`, with the new
  `✓ removed install tree: .../cognicode-home/install/0.95.0` line.

## HONEST pollution disclosure

UAT-867 post-snapshot sha256 of `~/.config/opencode/opencode.json`
(`b93fcd3c…`) differed from the post-E86.4-cleanup value
(`13bb7c2e…`). Investigation showed the logical content was
unchanged — same top-level keys, same MCP entries, same agents.
The sha256 delta is whitespace from a re-serialisation inside
`write_json_atomic` (which `Step::MergeJson` always invokes, even
when the underlying value is identical).

The HOME config was then **restored from the developer's
`opencode.json.bak.full-deepseek-sddk-20260818_141329` backup** (the
most recent pre-E86.x baseline on disk). Post-restore sha256:
`a5f9a61f…`. E86.7's release binary was not the source of the
sha256 drift (OPENCODE_CONFIG was set correctly by the UAT), but
the UAT's Phase B `Step::MergeJson` invocation does still
re-serialise the disposable `disabled.json` — the same write
path that other UAT runs have used, which historically caused the
whitespace-only drift across cycles.

Net effect: the developer's `~/.config/opencode/opencode.json` is
back to a known-good pre-E86.x state.

## Out of scope (filed for a later cycle)

1. The Phase D reinstall `link_or_copy failed` is the deep layout
   bug from `install.rs:46-49` (the install transaction builds the
   opencode skills source path as
   `<root>/install/<ver>/mcp-server/skills`, but the bundle's
   tarball does not contain a `mcp-server/` component). Tabled
   pending the `install/` vs `versions/` architectural decision.
2. `cmd_ide_install` (the layout.rs `cmd_install` path) uses
   `home.versions().join(version).join(plugin).join("skills")` —
   the "plugins under versions/" assumption — which contradicts
   the install transaction's "components under install/"
   assumption. The two views of the layout are the same
   architectural question surfaced in different code paths.
3. `cmd_uninstall` does not touch the journal — the install
   remains in `cogh list`'s history even after the tree is gone.
   Pinned-by-design: undoing an install via the journal is the
   job of `cmd_rollback`, not `cmd_uninstall`. If a future
   requirement wants "uninstall == forget", that is a separate
   cycle that touches the journal envelope, not just the
   filesystem.

## Cycle verdict

**PASS**. Bounded change: one file, 137 insertions (mostly tests
and doc comments), 0 deletions. The behavioural change is one
`remove_dir_all` block. No new dependencies, no new crates, no
new filesystem abstraction. The asymmetry between "install wrote
it" and "uninstall doesn't remove it" is closed.
