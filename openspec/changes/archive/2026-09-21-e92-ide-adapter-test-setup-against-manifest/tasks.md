# Tasks — e92: IDE-adapter integration tests must stand up a bundle manifest

## Task 1: Add `plant_manifest` and `init_home_with_manifest` helpers

- **File**: `crates/cognicode-cli/tests/cognicode_ide_adapter.rs`
- **Description**: Add two helpers at the top of the file:
  - `plant_manifest(cogh_home, version_dir)` — writes a minimal v2 bundle
    manifest at `<cogh_home>/versions/<version_dir>/manifest.yaml` declaring
    the `core` profile, the `cognicode-mcp` component, and a single
    `skill_bundles[]` entry. Also creates the corresponding skill bundle
    directory under `<cogh_home>/versions/<version_dir>/skills/<bundle.id>/`
    with a `SKILL.md` placeholder, because `declared_skill_bundle_dirs`
    refuses a declared-but-missing bundle.
  - `init_home_with_manifest(fake_home, cogh_home)` — wraps the existing
    `init_home` helper and additionally calls
    `plant_manifest(cogh_home, "latest")` because `cmd_ide_install --plugin mcp-server`
    defaults to the `latest` pseudo-version.
- **Acceptance**: helpers compile, are public to the test module, and have
  doc comments documenting the fixture contract.
- **Status**: DONE

## Task 2: Wire `init_home_with_manifest` into the 3 originally-red tests

- **File**: `crates/cognicode-cli/tests/cognicode_ide_adapter.rs`
- **Description**: Replace `let _ = init_home(fake_home, cogh_home.path());`
  with `init_home_with_manifest(fake_home, cogh_home.path());` in:
  - `cogh_ide_install_opencode_writes_mcp_entry_preserving_existing`
  - `cogh_ide_uninstall_opencode_removes_mcp_entry` (additionally plants
    a manifest at `versions/0.94.15/manifest.yaml` because the test passes
    `--version 0.94.15`)
  - `cogh_ide_install_zcode_writes_zcode_specific_path`
- **Acceptance**: all three tests now exit 0; `cargo test -p cognicode-cli --test cognicode_ide_adapter` reports `7 passed; 0 failed`.
- **Status**: DONE

## Task 3: Add a negative test pinning the semver invariant (Scenario 4)

- **File**: `crates/cognicode-cli/tests/cognicode_ide_adapter.rs`
- **Description**: Add `cogh_ide_install_opencode_rejects_non_semver_manifest_version`
  that plants a manifest with `version: "latest"` (the literal string, not a
  semver) and asserts `cogh ide install opencode` exits non-zero with the
  expected error message.
- **Acceptance**: new test appears in `cargo test -p cognicode-cli --test cognicode_ide_adapter`'s run list and PASSES.
- **Status**: TO DO — deferred to a follow-up cycle. Not strictly required for
  the green-status recovery of the three originally-red tests; tracked in
  DEBT-SDDK-008.

## Task 4: Document the fixture contract in the module doc comment

- **File**: `crates/cognicode-cli/tests/cognicode_ide_adapter.rs`
- **Description**: Add a `//!` module doc comment at the top of the file
  explaining that, since `arch(debt2)`, every test in this file that drives
  `cogh ide install` / `cogh ide uninstall` MUST go through
  `init_home_with_manifest` (or plant a manifest directly) — never the bare
  `init_home`. This is the institutional-memory anchor against regression.
- **Acceptance**: module doc comment is present and references
  `init_home_with_manifest` and the `arch(debt2)` cycle.
- **Status**: DONE (added as part of Task 1's helpers).
