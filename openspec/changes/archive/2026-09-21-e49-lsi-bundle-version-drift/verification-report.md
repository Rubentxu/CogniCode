# Verification Report — cycle e49 — bundle version drift (root-cause fix)

## Summary

| Item | Value |
|------|-------|
| Cycle | e49 — bundle version drift (root-cause fix) |
| Path | A-min |
| Branch | `main` |
| Base HEAD | `dcca25ea` (post-e48) |
| Type | production + test-infra |
| Verify verdict | **PASS** |

## Root causes addressed

| ID | Defect | Fix |
|----|--------|-----|
| RC-1 | Embedded bundle path hard-coded to `v0.94.14` while version is `0.94.15` → clean-home install aborts with a spurious version mismatch | Derive the path from `CARGO_PKG_VERSION` at compile time; add `bundles/v0.94.15/bundle.yaml` |
| RC-2 | `cogh_bin()` hard-codes `<workspace>/target/debug/cogh`, which under a custom `CARGO_TARGET_DIR` resolved to a stale (2026-08-16) binary | Derive the binary from `std::env::current_exe()` |

## Verification commands executed

### V-1 — Unit + integration suite, serial

```
cargo test -p cognicode-cli -- --test-threads=1
```

Result:

```
unit (cogh bin):  101 passed; 0 failed; 1 ignored
tests/cogh_cli:     7 passed; 0 failed
tests/portable_skill_bundle: 7 passed; 0 failed
tests/cognicode_lifecycle:   7 passed; 0 failed
tests/cognicode_plugin:      5 passed; 0 failed
tests/cognicode_ide_adapter: 8 passed; 0 failed
```

**All green.** Pre-e49 serial was `98 passed; 2 failed` (the two
deterministic version-mismatch failures). The +1 test count is the new
`embedded_bundle_version_matches_pkg_version` regression test.

### V-2 — Previously failing tests, targeted

```
cargo test -p cognicode-cli test_clean_home_install
cargo test -p cognicode-cli test_install_with_ide_and_profile_dispatches_both
```

Both: **ok** (previously deterministic `version mismatch` failures).

### V-3 — New regression test

```
cargo test -p cognicode-cli embedded_bundle_version_matches_pkg_version
```

Result: **ok**. Parses the embedded manifest and asserts
`manifest.version == env!("CARGO_PKG_VERSION")` and
`assert_pkg_version().is_ok()`.

### V-4 — Compile-time drift guard (proof)

Temporarily bumped the workspace version to `0.94.16` (no matching
bundle) and built:

```
cargo build -p cognicode-cli
error: couldn't read `.../bundles/v0.94.16/bundle.yaml`:
       No such file or directory (os error 2)
```

**Confirmed:** drift is now a loud compile error, not a silent runtime
failure. Version reverted to `0.94.15` immediately after.

### V-5 — `ide::tests` unaffected

```
cargo test -p cognicode-cli ide::tests
```

Result: **20 passed; 0 failed**.

## Regression / no-regression

- `cargo build -p cognicode-cli` → succeeds.
- `cargo fmt -p cognicode-cli --check` → the changed files
  (`installer_transaction.rs`, `lifecycle.rs`) are clean. (Pre-existing
  fmt drift remains in `tests/cogh_cli.rs` and
  `tests/cognicode_ide_adapter.rs`; out of scope, see below.)
- `cargo clippy -p cognicode-cli --all-targets` → no new errors
  (pre-existing unused-variable warnings only).

## Known residual (out of scope)

Under **default parallel** execution the suite still shows 3-4 variable
failures:

```
install_lock::tests::lock_acquire_and_release
install_lock::tests::lock_guard_releases_on_drop
lifecycle::tests::test_cogh_list_shows_installed
ide::tests::integrate_opencode_writes_mcp_entry
```

Every one of these passes in isolation and in serial. They are the
residual process-global-state race documented by e48: `serial_test`'s
`#[serial]` only serialises the annotated tests among themselves; tests
that mutate `HOME` / `COGNICODE_HOME` / the lock path and are *not*
annotated still run concurrently with them. This is orthogonal to the
version drift fixed here and is a candidate for cycle e50.

Also noted: `tests/cogh_cli.rs` and `tests/cognicode_ide_adapter.rs`
have pre-existing `cargo fmt --check` drift (unrelated to this cycle).

## Spec compliance

| Requirement | Status |
|-------------|--------|
| Embedded bundle path tracks the crate version | ✅ (V-4) |
| A bundle exists for the current crate version | ✅ (`bundles/v0.94.15/bundle.yaml`, V-3) |
| Lifecycle subprocess tests exercise the freshly built binary | ✅ (RC-2 fix, V-2) |
| Clean-home install no longer aborts on version mismatch | ✅ (V-2) |

## Conclusion

Both root causes are fixed and verified. The CLI test suite is fully
green under serial execution (101 passed; 0 failed), previously-failing
tests pass, and the drift can no longer occur silently. The remaining
parallel-execution flakiness is the pre-existing race, explicitly out of
scope.
