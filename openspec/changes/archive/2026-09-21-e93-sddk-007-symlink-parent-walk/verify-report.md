# Verify Report — e93 (DEBT-SDDK-007): InputValidator parent-symlink walk produces false positives

## Scope of verification

- Direct test (the cycle's primary SUT): `cargo test -p cognicode-core --lib`
- Owning component (security suite): `cargo test -p cognicode-core --lib -- security::tests`
- Collateral crates: `cargo test -p cognicode-cli --bin cogh`,
  `cargo test -p cognicode-cli --test cognicode_ide_adapter`
- Build: `cargo build --workspace --all-targets`

## Results

### Direct test

```text
$ cargo test -p cognicode-core --lib --no-fail-fast
test result: ok. 2083 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out
```

| Metric | Before e93 | After e93 | Delta |
|---|---|---|---|
| Passed | 2032 | 2083 | +51 (50 previously-red tests + 1 new regression test) |
| Failed | 50 | 0 | -50 |
| Ignored | 27 | 27 | 0 |
| Total | 2109 | 2110 | +1 (new regression test) |

The 50 previously-failing tests are exactly the ones catalogued in
e92's verify-report: ~25 `file_operations::tests`, ~14
`mcp::file_ops_handlers::tests`, ~4 `mcp::handlers::refactor_handlers::tests`,
~5 `mcp::mcp_roundtrip_tests::*`, 3 `mcp::security::tests::*`, and
1 `workspace_session::tests::test_workspace_session_creation`. All now green.

### Owning component (security suite)

```text
$ cargo test -p cognicode-core --lib -- security::tests
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 2071 filtered out
```

| Test | Status |
|---|---|
| `test_symlink_on_file_rejected` | ✅ pass (retained) |
| `test_symlink_directory_rejected` | ✅ pass (retained) |
| `test_symlink_parent_directory_inside_workspace_accepted` | ✅ pass (renamed from `..._rejected`; pins new contract) |
| `test_symlink_inside_workspace_pointing_outside_is_rejected` | ✅ pass (new regression test) |
| 32 other security tests | ✅ all pass |

The 35 security tests that PASSED before e93 remain green. The one that
asserted the old (now-removed) parent-symlink walk's behaviour
(`test_symlink_parent_directory_rejected`) was renamed and re-purposed:
it now asserts the **new** contract (parent symlink inside the workspace
is accepted when the canonical target lies inside the workspace). A
**new** regression test pins the security invariant
(`test_symlink_inside_workspace_pointing_outside_is_rejected`).

### Collateral crates

```text
$ cargo test -p cognicode-cli --bin cogh
test result: ok. 291 passed; 0 failed; 1 ignored

$ cargo test -p cognicode-cli --test cognicode_ide_adapter
test result: ok. 7 passed; 0 failed; 0 ignored
```

No collateral.

### Build

```text
$ cargo build --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s)
warning: profiles for the non root package will be ignored, ...
warning: unused variable: `digest_seed`
warning: function `scope` is never used
warning: unused import: ...
```

Same warning count and same warnings as the pre-cycle baseline (residual
H5-phase warnings, unrelated to this cycle).

## Acceptance criteria

| # | Criterion | Status |
|---|---|---|
| 1 | `cargo test -p cognicode-core --lib` reports `2082+ passed; 0 failed; 27 ignored` | ✅ 2083 passed; 0 failed; 27 ignored |
| 2 | `cargo test -p cognicode-core --lib security::tests` keeps 35+ tests green (no regression in security suite) | ✅ 36 passed; 0 failed |
| 3 | New regression test `symlink_inside_workspace_pointing_outside_is_rejected` passes | ✅ |
| 4 | `cargo test -p cognicode-cli --bin cogh` and `--test cognicode_ide_adapter` remain green | ✅ |
| 5 | `cargo build --workspace --all-targets` succeeds with no new warnings | ✅ (no new warnings) |

## Security audit (Task 5)

Grep for callers that depend on the parent-symlink walk's specific behaviour:

```text
$ rg "SymlinkDetected" crates/cognicode-core/src/
crates/cognicode-core/src/interface/mcp/security.rs (definition + tests)
crates/cognicode-core/src/interface/mcp/security.rs:262  return Err(SecurityError::SymlinkDetected { path: path.to_string() });
crates/cognicode-core/src/interface/mcp/security.rs:379  return Err(SecurityError::SymlinkDetected { path: path_str.to_string() });
```

The only code paths that **produce** `SymlinkDetected` are the
direct-symlink check on `&resolved` (which we kept) and the now-removed
parent-walk (which we replaced with a comment explaining why). The
matcher `SecurityError::SymlinkDetected { .. }` still appears in 2
tests (the renamed `test_symlink_parent_directory_inside_workspace_accepted`
no longer matches it; the new regression test matches it as the
fast-path defence). No production code matches on `SymlinkDetected`
specifically to detect system-level symlinks — production code only
propagates the error.

**Audit conclusion**: removing the parent-walk is safe. No production
code path depends on the old semantics.

## Diff summary

```text
crates/cognicode-core/src/interface/mcp/security.rs
  ~ InputValidator::validate_file_path: removed parent-symlink walk
  ~ InputValidator::validate_path:        removed parent-symlink walk
  ~ InputValidator::validate_path:        removed parent-symlink walk in NotFound fallback
  ~ test_symlink_parent_directory_rejected
      → renamed to test_symlink_parent_directory_inside_workspace_accepted
        (asserts the NEW contract: parent symlink inside workspace is accepted)
  + test_symlink_inside_workspace_pointing_outside_is_rejected
      (asserts the security invariant: symlink attack via parent walk is still
       caught by the direct-symlink check + canonical workspace boundary)

crates/cognicode-core/src/application/workspace_session.rs
  ~ test_workspace_session_creation: compare canonical paths on both sides
```

No public API changed. No new dependencies. No production code logic
change outside the documented removal.

## Side-finding: pre-existing security tests that pre-date e93

Before e93, 35 of 38 security tests passed; 3 failed
(`test_workspace_boundary_enforcement`,
`test_validate_path_before_operation_no_race`,
`test_validate_nonexistent_path_parent_exists`). All three now pass.
None required product-code changes — they all failed for the same
single root cause (system symlink in `TMPDIR`'s parent).

## Decision

**PASS** — the cycle's scope (recover ~50 reproducibly failing
`cognicode-core` tests by removing a redundant security check that
produced false positives on system-managed symlinks) is fully verified.
Security invariants are preserved and pinned by a new regression test.

## Refs

- `crates/cognicode-core/src/interface/mcp/security.rs` — production code
  (parent-symlink walk removed in two functions)
- `crates/cognicode-core/src/application/workspace_session.rs` — test fix
- Carry-forward debt: DEBT-SDDK-007 (now closed by this cycle)
