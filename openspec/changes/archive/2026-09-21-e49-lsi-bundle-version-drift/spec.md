# Spec — cycle e49 — bundle version drift (root-cause fix)

> Cycle: A-min | Phase: spec | Date: 2026-09-15
> Change ID: `e49-lsi-bundle-version-drift`

## Context

The `cogh` installer loads a bundle manifest from
`~/.cognicode/bundle.yaml`, falling back to an embedded asset when the
on-disk file is absent. The embedded asset path was a hand-maintained
literal pinned to `v0.94.14`, while the workspace version is `0.94.15`.
Because `BundleManifest::assert_pkg_version()` strictly compares the
manifest version against `CARGO_PKG_VERSION`, every clean-home install
aborts with a spurious "CLI upgrade required" error.

## Requirements

### Requirement: Embedded bundle path tracks the crate version

The embedded fallback bundle path in
`crates/cognicode-cli/src/cmd/installer_transaction.rs` MUST be derived
from `env!("CARGO_PKG_VERSION")` at compile time, so that the crate
version is the single source of truth for which bundle is embedded.
The loader MUST NOT contain a hand-maintained version literal.

#### Scenario: No hard-coded version literal remains

- GIVEN `crates/cognicode-cli/src/cmd/installer_transaction.rs`
- WHEN the `load_bundle_manifest` fallback is inspected
- THEN the `include_str!` argument is built with
  `concat!(env!("CARGO_MANIFEST_DIR"), "/../../bundles/v", env!("CARGO_PKG_VERSION"), "/bundle.yaml")`
- AND no literal `v0.94.` version segment appears in the fallback path.

#### Scenario: Version bump without bundle fails the build

- GIVEN the embedded path is derived from `CARGO_PKG_VERSION`
- WHEN the workspace version is bumped to a version with no matching
  `bundles/v<version>/bundle.yaml`
- THEN `cargo build -p cognicode-cli` fails with an `include_str!`
  read error naming the missing bundle path
- (drift is a loud compile-time error, not a silent runtime failure).

### Requirement: A bundle exists for the current crate version

The repository MUST contain `bundles/v<CARGO_PKG_VERSION>/bundle.yaml`
whose `version` field equals `CARGO_PKG_VERSION`.

#### Scenario: Current-version bundle present and consistent

- GIVEN the workspace version is `0.94.15`
- WHEN `bundles/v0.94.15/bundle.yaml` is parsed
- THEN its `version` field equals `"0.94.15"`
- AND its component `version` fields equal `"0.94.15"`
- AND the manifest passes `BundleManifest::from_str`.

#### Scenario: Embedded manifest version matches CARGO_PKG_VERSION

- GIVEN the crate is built with the derived embedded path
- WHEN the embedded bundle is parsed and `assert_pkg_version()` is called
- THEN it returns `Ok(())` (no mismatch)
- AND a regression test asserts this invariant directly.

### Requirement: Lifecycle subprocess tests exercise the freshly built binary

`cogh_bin()` in `crates/cognicode-cli/src/cmd/lifecycle.rs` MUST resolve
the `cogh` binary relative to the running test executable
(`std::env::current_exe()`), not via a hard-coded
`<workspace>/target/debug` path. This keeps the subprocess tests honest
regardless of a custom `CARGO_TARGET_DIR`.

#### Scenario: No hard-coded target path remains

- GIVEN `crates/cognicode-cli/src/cmd/lifecycle.rs`
- WHEN `cogh_bin()` is inspected
- THEN it derives the binary from `std::env::current_exe()` (two
  directories up, joined with `cogh` + `EXE_SUFFIX`)
- AND it contains no literal `join("target")` / `join("debug")` path
  fragment.

#### Scenario: Subprocess tests use the current build

- GIVEN a custom `CARGO_TARGET_DIR`
- WHEN `test_install_with_ide_and_profile_dispatches_both` runs
- THEN the spawned `cogh` process is the one built by the current
  `cargo test` invocation
- AND the test passes.

### Requirement: Clean-home install no longer aborts on version mismatch

`install::run_install(home, "core")` on a clean home (no on-disk
`~/.cognicode/bundle.yaml`) MUST NOT fail with a version mismatch.

#### Scenario: `test_clean_home_install` passes

- GIVEN a temp `HOME` with no bundle on disk
- WHEN `install::run_install(&home, "core")` runs
- THEN it returns `Ok`
- AND the tracker version file and shims dir are materialised.

#### Scenario: `test_install_with_ide_and_profile_dispatches_both` passes

- GIVEN a temp home with no bundle on disk
- WHEN an install is dispatched with IDE-patching and a profile
- THEN the install stage no longer aborts with a version mismatch.

## Out of scope

- Removing stale historical bundles (`v0.94.1`, `v0.94.11`).
- Un-ignoring `test_cogh_install_runs_successfully`.
- The residual `install_lock` shared-state race.
- Any change to the `assert_pkg_version` comparison semantics
  (co-versioning remains a hard guarantee).
