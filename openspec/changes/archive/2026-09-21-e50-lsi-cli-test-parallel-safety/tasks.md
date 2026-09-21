# Tasks — cycle e50 — CLI test parallel safety

> Cycle: A-min | Phase: tasks | Date: 2026-09-15
> Change ID: `e50-lsi-cli-test-parallel-safety`

## Work units

### WU-1 — Serialise remaining env-touching tests

Add `use serial_test::serial;` where missing and `#[serial]` to every
env-touching test:

- `layout.rs::tests::resolve_from_env_var`
- `tracker.rs::tests::write_and_read_version`
- `lifecycle.rs::tests`: `doctor_reports_clean_install`,
  `install_creates_mcp_server_version_dir`,
  `uninstall_opencode_ide_removes_entry_and_skills`,
  `plugin_list_shows_bundled_plugins`,
  `install_lock_acquire_creates_lock_file`,
  `test_cogh_install_runs_successfully`,
  `test_cogh_list_shows_installed`,
  `test_cogh_current_returns_version`,
  `test_cogh_update_respects_lockfile`,
  `test_cogh_doctor_reports_health`,
  `test_install_lock_acquire_and_release`,
  `test_self_apply_opencode_adapter`
- `ide.rs::tests::detect_opencode_finds_config`
- `install_lock.rs::tests` and
  `installer_transaction.rs::tests::{advance_skips_through_all_stages, commit_writes_manifest_file}`

### WU-2 — Private temp paths in `registry` tests

`registry::tests::sha256_of_empty_file` and `sha256_detects_mismatch`
must each use their own `tempfile::tempdir()` instead of the shared
`temp_dir()/cogh-sha-<pid>.bin` path.

### WU-3 — Remove the shell wrapper from the IDE adapter tests

In `tests/cognicode_ide_adapter.rs`:
- Replace `home_wrapper()` with `fake_home() -> TempDir`.
- `run_with_home` uses `Command::new(cogh())` +
  `cmd.env("HOME", fake_home)` instead of spawning a wrapper script.
- Drop the now-unused `std::io::Write` / `PathBuf` imports.

### WU-4 — Shared hermetic-home helper

Add `#[cfg(test)] pub(crate) mod test_support` to `layout.rs` with
`TempCognicodeHome` (sets `COGNICODE_HOME` to a temp dir, restores on
drop). Use it in `install_lock.rs` and `installer_transaction.rs`.

### WU-5 — Hermetic `detect_opencode_finds_config`

Rewrite `ide::tests::detect_opencode_finds_config` to create a stub
`opencode.json` under a private temp `HOME` and assert `detect_opencode()`
finds it, instead of asserting on the developer's real config.

## Sequencing

Apply WU-1 → WU-2 → WU-3 → WU-4 → WU-5 in one commit.

## Acceptance gate

- `cargo test -p cognicode-cli` passes under default parallelism,
  repeatably (40/40 clean runs).
- `cargo test -p cognicode-cli -- --test-threads=1`: 101 passed;
  0 failed; 1 ignored, all integration suites green.
- No test creates a file under the developer's real `~/.cognicode`.
- No `with-home.sh` wrapper remains in `tests/cognicode_ide_adapter.rs`.
- Test-only change (plus a `#[cfg(test)]` helper module); no production
  behaviour change.
- Conventional commit, no AI trailers.
