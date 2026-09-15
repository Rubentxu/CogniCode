# Spec — cycle e50 — CLI test parallel safety

> Cycle: A-min | Phase: spec | Date: 2026-09-15
> Change ID: `e50-lsi-cli-test-parallel-safety`

## Context

`cargo test -p cognicode-cli` runs its unit tests on threads in one
process. Many tests mutate or read process-global state: `HOME` and
`COGNICODE_HOME` via `std::env::set_var`, and env-derived paths
(`layout::cognicode_home()`, `CognicodeHome::resolve(None)`, the IDE
config paths). Without complete isolation the suite is flaky under
default parallel execution.

## Requirements

### Requirement: Env-mutating tests are serialised

Every unit test in `cognicode-cli` that mutates process-global env
(`HOME`, `COGNICODE_HOME`, `XDG_*`) or reads env-derived state MUST be
annotated `#[serial]`, and its `mod tests` MUST import
`serial_test::serial`.

#### Scenario: No env-touching test is left un-serialised

- GIVEN the `cognicode-cli` unit-test modules
  (`ide`, `lifecycle`, `layout`, `tracker`, `install_lock`,
  `installer_transaction`)
- WHEN each test that calls `set_var`/`remove_var` or reaches
  `cognicode_home()` / `CognicodeHome::resolve(None)` is inspected
- THEN it carries `#[serial]`
- AND the containing `mod tests` imports `serial_test::serial`.

### Requirement: Filesystem-dependent unit tests are hermetic

Unit tests that exercise filesystem stages driven by `cognicode_home()`
(install lock, installer transaction) MUST redirect `COGNICODE_HOME` to
a private temp dir and restore it on drop, so they neither race on nor
pollute the developer's real `~/.cognicode`.

#### Scenario: Installer-transaction and lock tests do not touch real home

- GIVEN `install_lock::tests` and
  `installer_transaction::tests::{advance_skips_through_all_stages, commit_writes_manifest_file}`
- WHEN they run
- THEN they use `crate::layout::test_support::TempCognicodeHome`
- AND every path they create lives under that temp dir.

### Requirement: Tests use private temp paths

No two tests MAY share a fixed temporary path.

#### Scenario: `registry` sha256 tests do not clobber each other

- GIVEN `registry::tests::sha256_of_empty_file` and `sha256_detects_mismatch`
- WHEN they run concurrently
- THEN each writes its fixture inside its own `tempfile::tempdir()`
- AND neither references a `temp_dir()/cogh-sha-<pid>` shared path.

### Requirement: Integration tests set the child `HOME` without a wrapper

`tests/cognicode_ide_adapter.rs` MUST set the spawned `cogh` process's
`HOME` via `Command::env("HOME", ...)` rather than a shell wrapper
script.

#### Scenario: No shell wrapper and no ETXTBSY

- GIVEN `tests/cognicode_ide_adapter.rs`
- WHEN the helper that runs `cogh` is inspected
- THEN it uses `Command::env("HOME", fake_home)` on the `cogh` binary
- AND no `with-home.sh` wrapper (or `exec`-via-shell) is created.

### Requirement: Tests do not depend on the developer's real config

`ide::tests::detect_opencode_finds_config` MUST construct its own stub
opencode config in a private temp `HOME` instead of asserting on the
developer's `~/.config/opencode/opencode.json`.

#### Scenario: Detection test is hermetic

- GIVEN `detect_opencode_finds_config`
- WHEN it runs on a machine with no real opencode config
- THEN it still passes, because it writes a stub config under its own
  temp `HOME` and asserts `detect_opencode()` finds it.

### Requirement: The suite is green under default parallel execution

`cargo test -p cognicode-cli` MUST pass with no failures under default
parallelism, repeatably.

#### Scenario: Repeated parallel runs are clean

- GIVEN a freshly built `cognicode-cli`
- WHEN `cargo test -p cognicode-cli` is run 40 times with default
  parallelism
- THEN every run reports 0 failures.

## Out of scope

- Changing `cognicode_home()` to accept an explicit path.
- Pre-existing fmt drift (`tests/cogh_cli.rs`) and clippy warnings.
