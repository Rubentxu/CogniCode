# Tasks — e93 (DEBT-SDDK-007): InputValidator parent-symlink walk produces false positives

## Task 1: Remove parent-symlink walk in `validate_file_path`

- **File**: `crates/cognicode-core/src/interface/mcp/security.rs`
- **Location**: in `InputValidator::validate_file_path`, after the direct-symlink check on `&resolved` (line ~265).
- **Description**: Delete the `while let Some(parent) = current.parent()` loop that walks every parent component and rejects any whose `symlink_metadata().is_symlink()` is true. Keep the direct-symlink check on `&resolved`. Keep the workspace boundary check that runs after canonicalization (line ~308).
- **Acceptance**: the loop is gone; `cargo check -p cognicode-core` compiles.
- **Status**: TO DO

## Task 2: Remove parent-symlink walk in `validate_path`

- **File**: `crates/cognicode-core/src/interface/mcp/security.rs`
- **Location**: in `InputValidator::validate_path`, after the direct-symlink check (line ~380).
- **Description**: Same as Task 1 but on the `validate_path` function (which is a parallel API to `validate_file_path`).
- **Acceptance**: the loop is gone; `cargo check -p cognicode-core` compiles.
- **Status**: TO DO

## Task 3: Add regression test for symlink-attack defence

- **File**: `crates/cognicode-core/src/interface/mcp/security.rs` (test module)
- **Description**: Add `fn test_symlink_inside_workspace_pointing_outside_is_rejected`:
  1. Create a `TempDir` for the workspace.
  2. Construct an `InputValidator` with `allowed_paths = [workspace.path()]`.
  3. Create a symlink at `<workspace>/escape -> /etc/passwd`.
  4. Assert `validator.validate_file_path("<workspace>/escape")` returns
     `Err(SecurityError::PathOutsideWorkspace)`.
- **Acceptance**: new test passes after Task 1+2; would have also passed before, but pins the invariant for future refactors.
- **Status**: TO DO

## Task 4: Fix `test_workspace_session_creation` to compare canonical paths

- **File**: `crates/cognicode-core/src/application/workspace_session.rs`
- **Description**: Change the assertion from
  `assert_eq!(session.workspace_root(), temp_dir.path());`
  to
  `assert_eq!(session.workspace_root().canonicalize().unwrap(), temp_dir.path().canonicalize().unwrap());`
- **Acceptance**: `test_workspace_session_creation` passes; no other test in the module is affected.
- **Status**: TO DO

## Task 5: Audit `SymlinkDetected` callers

- **Files**: all callers of `validate_file_path` / `validate_path`.
- **Description**: Grep for any production code that branches on
  `SecurityError::SymlinkDetected` specifically because of the parent-walk
  semantics (i.e. expects a `SymlinkDetected` error for system-level
  symlinks). The expectation is zero callers — the parent-walk was an
  internal defence, not a documented API contract — but verify.
- **Acceptance**: the verify-report documents the audit result.
- **Status**: TO DO (run during verify)

## Task 6: Verify

- **Description**: Run the full acceptance-criteria battery:
  1. `cargo test -p cognicode-core --lib` — must report `2082 passed; 0 failed; 27 ignored`
  2. `cargo test -p cognicode-core --lib security::tests` — must report `36 passed; 0 failed` (35 existing + the new regression test)
  3. `cargo test -p cognicode-cli --bin cogh` and `--test cognicode_ide_adapter` — must remain green
  4. `cargo build --workspace --all-targets` — must compile without new warnings
- **Acceptance**: all four pass; recorded in verify-report.
- **Status**: TO DO
