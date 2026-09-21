# Proposal — e92: IDE-adapter integration tests must stand up a bundle manifest

## Intent

Restore green status of `cognicode-cli/tests/cognicode_ide_adapter.rs`. Three of seven
tests in that file (`cogh_ide_install_opencode_writes_mcp_entry_preserving_existing`,
`cogh_ide_uninstall_opencode_removes_mcp_entry`, `cogh_ide_install_zcode_writes_zcode_specific_path`)
have been red since commit `9752cc52 arch(debt2)` (2026-09-18) because the
`cmd_ide_install` / `cmd_ide_uninstall` code paths now resolve skill bundles from a
v2 bundle manifest at `<cogh_home>/versions/<version>/manifest.yaml` — and the tests
only ran `cogh init`, which creates the directory tree but does NOT write a manifest.

The fix is at the test boundary: the tests must stand up the manifest fixture they
implicitly depend on. No product code changes.

## Scope

In scope:

- `crates/cognicode-cli/tests/cognicode_ide_adapter.rs`
- OpenSpec change directory `openspec/changes/2026-09-21-e92-ide-adapter-test-setup-against-manifest/`

Out of scope:

- `cmd_ide_install` / `cmd_ide_uninstall` production code (correct as of `arch(debt2)`).
- Other test files in the workspace.
- The pre-existing `cognicode-core` symlink-check interaction (see verify-report and
  the parallel cycle it gets carried into — DEBT-SDDK-007 below).

## Why now

The verify-report of cycle `arch(debt2)` (commit `9752cc52`) ran
`cargo test --bin cogh` (the CLI unit tests) but did NOT run
`cargo test -p cognicode-cli --test cognicode_ide_adapter` (the integration test
binary that exercises `cmd_ide_install` against a real `tempfile`-isolated home).
That gap surfaced as 3 reproducibly red tests on every subsequent run. The cycle is
necessary to bring coverage up to spec and to document the new fixture contract so
future cycles don't re-introduce the same regression.

## Approach

Introduce two helpers at the top of `cognicode_ide_adapter.rs`:

1. `plant_manifest(cogh_home, version_dir)` — writes a minimal `BundleManifest` (v2
   schema) under `<cogh_home>/versions/<version_dir>/manifest.yaml` plus the
   declared-but-physical skill bundle directory the integrator expects
   (`<cogh_home>/versions/<version_dir>/skills/<bundle.id>/` with a placeholder
   `SKILL.md`).

2. `init_home_with_manifest(fake_home, cogh_home)` — runs the existing `init_home`
   helper and then calls `plant_manifest(cogh_home, "latest")` because
   `cmd_ide_install --plugin mcp-server` defaults to the `latest` pseudo-version.

Two tests (opencode install, opencode uninstall) get a single-line swap from
`let _ = init_home(fake_home, cogh_home.path());` to
`init_home_with_manifest(fake_home, cogh_home.path());`. The third test
(`cogh_ide_install_zcode_writes_zcode_specific_path`) gets the same swap. The
opencode uninstall test additionally plants a manifest at
`versions/0.94.15/` because it explicitly passes `--version 0.94.15`.

## Trade-offs

- **Helper shape**: we could have extended `init_home` to plant the manifest
  unconditionally, but doing so would couple `init_home`'s contract to a bundle
  manifest the test file does not own. Keeping the manifest plant as a separate,
  explicit helper preserves the existing `init_home` semantics (which other tests
  rely on).
- **Fixture minimalism**: the manifest declares exactly one component (`cognicode-mcp`)
  and one skill bundle (`skills-for-test`). Adding more would inflate the fixture
  without changing what these tests assert (only the MCP config merge).
- **OpenCode's strict requirement**: opencode integration errors out
  `cannot integrate OpenCode without a declared SkillBundleId` if the manifest
  declares zero bundles, so the fixture declares one. The integrator copies the
  bundle's directory into the IDE skill store; the tests do not assert on that
  copy, only on the MCP config merge.

## Risks

- **Other IDE integration tests may have the same latent gap.** A quick grep
  confirms `cognicode_ide_adapter.rs` is the only test binary that exercises
  `cmd_ide_install`; the others stop at `cogh init`. Risk: low.
- **Schema drift in `BundleManifest`.** The fixture hardcodes `apiVersion:
  cognicode.bundle/v2`. If the schema version is bumped again the fixture must
  follow. Risk: low — e87 stabilized v2.
- **Side-effect: hidden pre-existing failure in `cognicode-core` tests.**
  Running the full `cargo test -p cognicode-core --lib` reveals ~50 failures in
  `application/services/file_operations::tests::*`. Investigation shows those
  failures pre-date e92 and stem from the interaction between this environment's
  `/home -> /var/home` symlink and `InputValidator::validate_path`'s parent-symlink
  walk. Documented in the verify-report and carried forward as DEBT-SDDK-007,
  not addressed here.

## Acceptance criteria

1. `cargo test -p cognicode-cli --test cognicode_ide_adapter` reports
   `7 passed; 0 failed`.
2. `cargo test -p cognicode-cli --bin cogh` still reports
   `291 passed; 0 failed; 1 ignored`.
3. `cargo build --workspace --all-targets` succeeds with the same warning count as
   before (no new warnings introduced).
4. The three originally-red tests appear in the verify-report with their new
   `init_home_with_manifest` callsites.

## Refs

- Commit `9752cc52 arch(debt2)` — the change that introduced the manifest
  resolution in `cmd_ide_install`.
- `crates/cognicode-cli/src/cmd/ide.rs` — production code (unchanged).
- `crates/cognicode-cli/src/cmd/bundle_manifest.rs` —
  `declared_skill_bundle_dirs` helper that consumes the fixture manifest.
- `crates/cognicode-cli/src/cmd/layout.rs` —
  `CogniCodeHome::skills_root` / `skill_bundle` path layout the fixture
  mirrors.
