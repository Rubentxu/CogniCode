# Spec — e48 — IDE adapter test stabilisation

> Change: `e48-lsi-ide-adapter-test-stabilisation` | Phase: spec | Date: 2026-09-15

## Purpose

Make the 8 HOME-mutating unit tests in `crates/cognicode-cli/src/cmd/ide.rs`
parallel-safe by annotating them with `#[serial]` from the `serial_test`
crate. This removes a CI hazard: today the IDE-adapter test module only
passes under `--test-threads=1`; after this cycle it passes under the
default parallel execution.

## Requirements

### Requirement: `serial_test` is available as a dev-dependency

`crates/cognicode-cli/Cargo.toml`'s `[dev-dependencies]` MUST include
`serial_test = "3"` (matching the version used in `cognicode-ladybug`).

#### Scenario: dev-dep is present

- GIVEN `crates/cognicode-cli/Cargo.toml`
- WHEN inspected
- THEN the `[dev-dependencies]` block contains `serial_test = "3"`.

### Requirement: HOME-mutating tests are annotated `#[serial]`

Each test in `crates/cognicode-cli/src/cmd/ide.rs::tests` and
`crates/cognicode-cli/src/cmd/lifecycle.rs::tests` that mutates `HOME`
via `unsafe { std::env::set_var("HOME", ...) }` MUST be annotated with
`#[serial]`. The corresponding `mod tests` blocks MUST import
`serial_test::serial` so the attribute resolves.

#### Scenario: All HOME-mutating tests carry `#[serial]`

- GIVEN `crates/cognicode-cli/src/cmd/ide.rs::tests`
- WHEN inspected for `#[serial]` attributes
- THEN each of the following test functions carries `#[serial]`:
  - `integrate_opencode_writes_mcp_entry`
  - `uninstall_opencode_removes_entry`
  - `integrate_zcode_creates_mcp_section`
  - `uninstall_zcode_removes_entry`
  - `integrate_claude_writes_mcp_file`
  - `uninstall_claude_removes_mcp_file`
  - `integrate_codex_inserts_mcp_server`
  - `uninstall_codex_removes_entry`

- AND GIVEN `crates/cognicode-cli/src/cmd/lifecycle.rs::tests`
- THEN each of the following test functions also carries `#[serial]`:
  - `init_creates_layout_and_bundled_plugins`
  - `install_opencode_ide_patches_config_and_skills`
  - `install_codex_ide_patches_toml_config`
  - `install_zcode_ide_patches_config_and_skills`
  - `install_claude_ide_patches_config_and_skills`
  - `test_clean_home_install`
  - `tracker_write_and_read_version_roundtrip`
  - `test_install_with_ide_and_profile_dispatches_both`

### Requirement: HOME-mutating tests pass under parallel execution

Running `cargo test -p cognicode-cli ide::tests` (default parallel)
MUST pass all 20 tests in the `ide::tests` module. The full CLI suite
under default parallelism MUST show a substantial reduction in
thread-safety-related failures compared to the pre-cycle baseline.

#### Scenario: Parallel execution passes

- GIVEN the 16 tests carry `#[serial]` (8 in `ide::tests` + 8 in `lifecycle::tests`)
- WHEN `cargo test -p cognicode-cli ide::tests` runs with default
  parallelism
- THEN the suite reports `20 passed; 0 failed`.

#### Scenario: CLI suite failure count drops

- GIVEN the pre-cycle baseline is `89 passed; 11 failed` under default
  parallelism (recorded before this cycle began)
- WHEN `cargo test -p cognicode-cli` runs with default parallelism
- THEN the suite reports at least `96 passed` (i.e., ≥ 7 fewer
  failures than the pre-cycle baseline).
- The remaining failures (if any) are pre-existing version-mismatch
  bugs (CARGO_PKG_VERSION vs bundle manifest version) that are
  unrelated to thread safety and were documented before this cycle.

## Out of scope

- Refactoring production code to accept `home: &Path` instead of
  reading `std::env::var("HOME")` internally.
- Fixing the 8 install/lifecycle unit tests that fail because of the
  hard-coded `version mismatch` error (CARGO_PKG_VERSION 0.94.15 vs
  bundle 0.94.14).
- Adding `serial_test` to other crates that may have similar issues.
