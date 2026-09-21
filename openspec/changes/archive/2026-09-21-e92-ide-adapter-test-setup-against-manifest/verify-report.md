# Verify Report — e92: IDE-adapter integration tests must stand up a bundle manifest

## Scope of verification

- Direct test: `cargo test -p cognicode-cli --test cognicode_ide_adapter`
- Owning component: `cargo test -p cognicode-cli --bin cogh`
- Build: `cargo build -p cognicode-cli`

## Results

### Direct test

```text
$ cargo test -p cognicode-cli --test cognicode_ide_adapter
running 7 tests
test cogh_ide_detect_lists_no_ides_on_empty_home ... ok
test cogh_ide_detect_lists_opencode_when_config_present ... ok
test cogh_init_includes_three_ide_plugins ... ok
test cogh_plugin_list_shows_ide_plugins_with_manifests ... ok
test cogh_ide_install_zcode_writes_zcode_specific_path ... ok
test cogh_ide_uninstall_opencode_removes_mcp_entry ... ok
test cogh_ide_install_opencode_writes_mcp_entry_preserving_existing ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The three tests that were red before this cycle
(`cogh_ide_install_opencode_writes_mcp_entry_preserving_existing`,
`cogh_ide_uninstall_opencode_removes_mcp_entry`,
`cogh_ide_install_zcode_writes_zcode_specific_path`) now pass.

### Owning component (CLI unit tests)

```text
$ cargo test -p cognicode-cli --bin cogh
test result: ok. 291 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

No regression. (The 1 ignored test is `cogh_install_with_already_existing_home`
— pre-existing, unrelated to this cycle.)

### Build

```text
$ cargo build -p cognicode-cli --bin cogh --bin cogh-bin
    Finished `dev` profile [unoptimized + debuginfo] target(s)
warning: `cognicode-cli` (bin "cogh") generated 67 warnings
```

67 warnings — same as before this cycle. None introduced by the helpers
(no `unused`, no `dead_code` flagged on `plant_manifest` /
`init_home_with_manifest`).

## Acceptance criteria

| # | Criterion | Status |
|---|-----------|--------|
| 1 | `cargo test -p cognicode-cli --test cognicode_ide_adapter` reports `7 passed; 0 failed` | ✅ |
| 2 | `cargo test -p cognicode-cli --bin cogh` reports `291 passed; 0 failed; 1 ignored` | ✅ |
| 3 | `cargo build -p cognicode-cli` succeeds with no new warnings | ✅ (67 warnings, identical to baseline) |
| 4 | The three originally-red tests appear in the verify-report with their new `init_home_with_manifest` callsites | ✅ — see diff below |

## Diff summary

```text
crates/cognicode-cli/tests/cognicode_ide_adapter.rs
  + fn plant_manifest(cogh_home: &Path, version_dir: &str)          // ~50 lines
  + fn init_home_with_manifest(fake_home: &Path, cogh_home: &Path) // ~5 lines
  ~ 3 test bodies: let _ = init_home(...) → init_home_with_manifest(...)
  ~ 1 test body:    + plant_manifest(cogh_home.path(), "0.94.15")
```

No product code changed. No other test file touched.

## Side-finding: pre-existing failure in `cognicode-core` (NOT addressed here)

While running the broader `cargo test -p cognicode-core --lib` to sanity-check
that no regression leaked into the core crate, the verifier observed
**~50 pre-existing failures** clustered in
`application::services::file_operations::tests::*`. Investigation:

1. Confirmed the failures pre-date this cycle by `git stash`-ing the e92
   change and re-running; the same tests fail with the same error message.
2. Added a temporary `eprintln!("RESULT_DEBUG: {result:?}")` to
   `test_edit_file_single_match` to capture the underlying error:
   ```text
   RESULT_DEBUG: Err(InvalidParameter("Symlink detected in path: /home"))
   ```
3. The host filesystem has `/home → /var/home` as a symlink
   (`ls -la /home` → `lrwxrwxrwx. 1 root root 8 ene  1  1970 /home -> var/home`),
   and this session's `TMPDIR=/home/rubentxu/.jcode/scratch` puts all
   `tempfile::NamedTempFile` artefacts under that symlink.
4. `InputValidator::validate_path` (in
   `crates/cognicode-core/src/interface/mcp/security.rs` around line 273)
   walks every parent component of the resolved path and rejects any of
   them whose `symlink_metadata().file_type().is_symlink()` is true. The
   `/home` symlink is therefore treated as an attacker-controlled symlink,
   even though it's a system-level link the user did not create.

This is a real defect, but it is **out of scope** for e92 (which targets
only `cognicode_ide_adapter.rs`) and it pre-dates the cycle. Carried
forward as **DEBT-SDDK-007** with the recommendation that the next cycle
either (a) loosen the validator to skip symlinks whose canonical target is
also under the workspace root, or (b) configure the test suite's `TMPDIR`
to a non-symlinked location. The diagnostic `eprintln!` was reverted
before commit.

## Decision

**PASS** — the cycle's scope (the three originally-red integration tests in
`cognicode_ide_adapter.rs`) is fully verified. The carried-forward
`cognicode-core` symlink interaction is documented and tagged
DEBT-SDDK-007 for the next planning round.

## Refs

- Test binary: `crates/cognicode-cli/tests/cognicode_ide_adapter.rs`
- Production code (unchanged): `crates/cognicode-cli/src/cmd/ide.rs`
- Manifest schema: `crates/cognicode-cli/src/cmd/bundle_manifest.rs`
- Layout: `crates/cognicode-cli/src/cmd/layout.rs`
- Carried-forward debt: DEBT-SDDK-007 (cognicode-core symlink/parent-walk)
