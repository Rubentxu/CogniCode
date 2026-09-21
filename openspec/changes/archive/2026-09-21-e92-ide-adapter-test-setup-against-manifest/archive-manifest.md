# Archive Manifest — e92: IDE-adapter integration tests must stand up a bundle manifest

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/2026-09-21-e92-ide-adapter-test-setup-against-manifest` |
| Path | b-direct |
| Date | 2026-09-21 |
| Actor | jcode-orchestrator |
| Status | **CLOSED — `archive-closed-with-commit`** |
| Closes | DEBT-SDDK-006 (test gap from `arch(debt2)` / commit `9752cc52`) |
| Carries forward | DEBT-SDDK-007 (parent-symlink walk) → closed by e93 |
| Carries forward | DEBT-SDDK-008 (semver-invariant negative test) → TODO |

## Outcome

CLOSED. The cycle recovered 3 reproducibly-failing tests in
`crates/cognicode-cli/tests/cognicode_ide_adapter.rs` that had been red
since commit `9752cc52 arch(debt2)` (2026-09-18) by introducing two
helpers at the top of the test file:

- `plant_manifest(cogh_home, version_dir)` — writes a minimal v2 bundle
  manifest at `<cogh_home>/versions/<version_dir>/manifest.yaml` plus
  the declared-but-physical skill bundle directory
  (`<cogh_home>/versions/<version_dir>/skills/<bundle.id>/`) with a
  `SKILL.md` placeholder. Required because
  `declared_skill_bundle_dirs` refuses a declared-but-missing bundle,
  and opencode integration refuses a zero-bundle manifest.
- `init_home_with_manifest(fake_home, cogh_home)` — wraps the existing
  `init_home` helper and additionally calls
  `plant_manifest(cogh_home, "latest")` because
  `cogh ide install --plugin mcp-server` defaults to the `latest`
  pseudo-version.

Three tests were re-pointed to the new helper
(`cogh_ide_install_opencode_writes_mcp_entry_preserving_existing`,
`cogh_ide_uninstall_opencode_removes_mcp_entry`,
`cogh_ide_install_zcode_writes_zcode_specific_path`). The opencode
uninstall test additionally plants a manifest at `versions/0.94.15/`
because it explicitly passes `--version 0.94.15`.

## Acceptance verdicts

| REQ | Status | Evidence |
|---|---|---|
| REQ-E92-01 (the 3 originally-red tests now pass) | ✅ PASS | `cargo test -p cognicode-cli --test cognicode_ide_adapter` reports `7 passed; 0 failed` |
| REQ-E92-02 (no regression in cogh unit tests) | ✅ PASS | `cargo test -p cognicode-cli --bin cogh` reports `291 passed; 0 failed; 1 ignored` |
| REQ-E92-03 (no new warnings) | ✅ PASS | `cargo build -p cognicode-cli` reports the same 67 warnings as before; helpers do not introduce any |
| REQ-E92-04 (the 3 originally-red tests appear in verify-report with new callsites) | ✅ PASS | verify-report.md documents each test's new `init_home_with_manifest` callsite |

## Commits produced

| Commit | Description |
|---|---|
| `3f20f50f` | `fix(test): e92 — stand up bundle-manifest fixture for ide-adapter integration tests` |

## Side-effects recorded

- The verify-report carries forward **DEBT-SDDK-007** (parent-symlink
  walk producing false positives on hosts with `/home` system symlinks,
  causing ~50 `cognicode-core` test failures). e93 closes it.
- The verify-report defers **DEBT-SDDK-008** (Scenario 4 — a negative
  test that pins the manifest's semver invariant). The test was
  enumerated in the spec but not implemented; the scope of e92 was the
  three originally-red tests, and the negative test is a one-test
  follow-up that does not block any roadmap item.

## Decision

**PASS — CLOSED.** The cycle's scope (the three originally-red tests in
`cognicode_ide_adapter.rs`) is fully verified and the changes are
committed. DEBT-SDDK-006 (the test gap from `arch(debt2)`) is closed.
