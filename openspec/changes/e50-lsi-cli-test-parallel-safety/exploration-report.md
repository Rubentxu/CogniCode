# Exploration Report — cycle e50 — CLI test parallel safety

> Cycle: A-min | Phase: explore | Date: 2026-09-15

## Trigger

With the version-drift defects fixed (cycle e49), `cargo test -p
cognicode-cli` still failed intermittently under **default parallel**
execution while passing 100% under `--test-threads=1`. Observed
failures varied run to run (3-6 tests), all in the `cogh` unit-test
binary:

```
install_lock::tests::lock_acquire_and_release
install_lock::tests::lock_guard_releases_on_drop
lifecycle::tests::test_cogh_list_shows_installed
lifecycle::tests::uninstall_opencode_ide_removes_entry_and_skills
ide::tests::integrate_opencode_writes_mcp_entry
ide::tests::detect_opencode_finds_config
installer_transaction::tests::commit_writes_manifest_file
registry::tests::sha256_of_empty_file
tests/cognicode_ide_adapter: cogh_plugin_list_shows_ide_plugins_with_manifests (ETXTBSY)
```

Flaky tests are a cardinal sin in this repo (AGENTS.md): they must be
isolated, not merely re-run.

## Investigation — five independent isolation defects

### D-1 — env-mutating tests not serialised (the main one)

`serial_test`'s `#[serial]` was applied by e48 to only 8 of the many
tests that mutate process-global env. Rust runs unit tests on threads
within one process, and `cognicode_home()` / `CognicodeHome::resolve(None)`
/ the IDE config paths all read `HOME` / `COGNICODE_HOME`. Any test that
mutates those while another reads them corrupts the reader.

Audit (test fns touching env or env-derived state, not yet `#[serial]`):

| Module | Tests |
|--------|-------|
| `layout.rs` | `resolve_from_env_var` |
| `tracker.rs` | `write_and_read_version` |
| `lifecycle.rs` | 12 tests (`run_cogh`, `install_lock::`, `set_var`) |
| `installer_transaction.rs` | `advance_skips_through_all_stages`, `commit_writes_manifest_file` |

(Note: `#[serial]` only serialises annotated tests *among themselves*;
un-annotated tests still run in parallel. So **every** env-touching test
must be annotated, not just a subset.)

### D-2 — shared fixed temp path between two tests

`registry::tests::sha256_of_empty_file` and `sha256_detects_mismatch`
both wrote to `temp_dir/cogh-sha-<pid>.bin` — the **same path**. In
parallel they clobber each other's content, so the hash check sees the
wrong bytes.

### D-3 — shell-wrapper exec race (ETXTBSY)

`tests/cognicode_ide_adapter.rs` created a `with-home.sh` wrapper and
immediately exec'd it. Under parallel execution this intermittently
returned `ETXTBSY` ("Text file busy", os error 26). The wrapper existed
only to set the child's `$HOME`.

### D-4 — tests using the real `~/.cognicode`

`install_lock::tests` and the two `installer_transaction.rs` fs-touching
tests resolved `cognicode_home()` with no override, so they wrote to the
developer's **real** `~/.cognicode/locks`, `/cache`, `/shims`,
`/install/...` — both a race surface (the value changed under them when
another test set `COGNICODE_HOME`) and home pollution.

### D-5 — test depending on the developer's real config

`ide::tests::detect_opencode_finds_config` asserted that
`opencode_config_path().exists()` — i.e. that the developer happens to
have `~/.config/opencode/opencode.json`. When a concurrent test pointed
`HOME` at a temp dir, the path vanished and the assertion failed. It
would also fail on any clean machine or CI.

## Landscape

| Path | Role |
|------|------|
| `crates/cognicode-cli/src/cmd/layout.rs` | `cognicode_home()` (env), `CognicodeHome::resolve` |
| `crates/cognicode-cli/src/cmd/{ide,lifecycle,install_lock,installer_transaction,registry,tracker}.rs` | test modules with isolation defects |
| `crates/cognicode-cli/tests/cognicode_ide_adapter.rs` | integration tests + shell wrapper |
| `crates/cognicode-cli/Cargo.toml` | `serial_test`, `tempfile` dev-deps (present) |

## Strategy

1. **Serialise every env-touching test** (D-1) with `#[serial]`, adding
   `use serial_test::serial;` where missing.
2. **Give each registry test its own temp dir** (D-2).
3. **Drop the shell wrapper**: set the child's `HOME` with
   `Command::env("HOME", &fake_home)` (D-3). This removes the subprocess
   shell and the exec race entirely.
4. **Add a shared `layout::test_support::TempCognicodeHome`** helper
   (`#[cfg(test)] pub(crate)`) that redirects `COGNICODE_HOME` to a temp
   dir and restores it on drop. Use it in `install_lock` and
   `installer_transaction` so the tests are hermetic (D-4).
5. **Make `detect_opencode_finds_config` hermetic** with a stub config in
   a temp `HOME` (D-5).

## Impact model

| Artifact | Impact | Confidence |
|----------|--------|-----------|
| `cognicode-cli` unit + integration tests | the whole surface under repair | KNOWN |
| production code | none (test-only; the only non-test change is a `#[cfg(test)]` helper module) | KNOWN |
| other crates | none | LIKELY |

## Out of scope

- Redesigning `cognicode_home()` to take an explicit path (the root cause
  of the whole class). That is a larger, cross-cutting refactor.
- The pre-existing `cargo fmt --check` drift in `tests/cogh_cli.rs`.
- The 57 pre-existing clippy warnings in the crate.
