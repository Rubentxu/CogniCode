# Proposal — e93 (DEBT-SDDK-007): InputValidator parent-symlink walk produces false positives

## Intent

Recover ~50 reproducibly failing tests in `cognicode-core --lib` that have been
red since the host filesystem was mounted with `/home → /var/home` and Jcode's
runtime `TMPDIR=/home/rubentxu/.jcode/scratch`. The failures cluster in
`application/services/file_operations::tests::*`,
`interface::mcp::file_ops_handlers::tests::*`,
`interface::mcp::handlers::refactor_handlers::tests::*`,
`interface::mcp::mcp_roundtrip_tests::*`,
`interface::mcp::security::tests::test_workspace_boundary_enforcement`,
`interface::mcp::security::tests::test_validate_path_before_operation_no_race`,
`interface::mcp::security::tests::test_validate_nonexistent_path_parent_exists`,
and `application::workspace_session::tests::test_workspace_session_creation`.

Root cause is two-fold:

1. `InputValidator::validate_path` (in
   `crates/cognicode-core/src/interface/mcp/security.rs`) walks every parent
   component of the resolved path and rejects any whose `symlink_metadata`
   reports `is_symlink()`. On a host where `/home → /var/home` is a system-level
   symlink, every `TempDir::new()` (which honors `TMPDIR`) puts the test fixture
   under `/home/...`, the parent-walk hits `/home`, and the validator rejects it
   with `Symlink detected in path: /home`. This parent-walk is **redundant**
   with the canonical-form workspace boundary check that already runs after:
   if any parent is a malicious symlink that escapes the workspace, the
   canonicalized target will fall outside `allowed_paths` and the workspace
   boundary check will reject it. The only thing the parent-walk actually
   catches that the workspace boundary misses is **system-level** symlinks like
   `/home → /var/home`, which are not attacker-controlled and so should not be
   treated as security violations.

2. `WorkspaceSession::new` canonicalizes the root and stores the canonical
   string, but the test
   `test_workspace_session_creation` asserts `session.workspace_root() ==
   temp_dir.path()` as raw strings. When `temp_dir.path()` is `/home/...` and
   `workspace_root()` is `/var/home/...` (same physical directory), the strings
   differ even though they refer to the same filesystem location. This is a
   pre-existing test bug exposed by the same symlinked `TMPDIR`.

## Scope

In scope:

- `crates/cognicode-core/src/interface/mcp/security.rs` — remove the
  parent-symlink walk in both `validate_file_path` (line ~265) and
  `validate_path` (line ~380). Keep the **direct symlink** check
  (`symlink_metadata(&resolved).is_symlink()`) — that one is the real
  symlink-attack defence (a file at the target path that is itself a symlink).
- `crates/cognicode-core/src/application/workspace_session.rs` — fix
  `test_workspace_session_creation` to compare canonical paths.
- OpenSpec change directory
  `openspec/changes/2026-09-21-e93-sddk-007-symlink-parent-walk/`.

Out of scope:

- Adding new `SecurityError` variants or changing the public API.
- Changing the direct-symlink check (the one on `&resolved`, not its parents).
- The 35 security tests that currently PASS (`test_symlink_on_file_rejected`,
  `test_symlink_directory_rejected`, `test_symlink_parent_directory_rejected`,
  etc.) — they must remain green after the change.
- Modifying `Cargo.toml` to add a `dunce` dependency; the fix is local.

## Why now

The test failures are blocking the scorecard's gate `cargo test --workspace`
from being GREEN and they pre-date the current cycle (they have been red since
the very first test run on this host, including the `2082 passing` summary in
the previous session, which was likely measured on a host without the symlink).
Closing them now unblocks every downstream test cycle that needs the core
crate's full unit-test signal.

## Approach

1. **Remove the parent-symlink walk** in both `validate_file_path` and
   `validate_path` (the two public functions in `InputValidator`). The direct
   symlink check stays. After removal, the workspace boundary check (which
   compares the canonical target against `allowed_paths`, all canonicalized at
   `with_workspace` time) is the single authority for "is this path safe?".

2. **Add a regression test** that proves the security guarantee is **not**
   weakened: a symlink inside the workspace that points outside the workspace
   MUST still be rejected. The canonical form of the symlink's target will fall
   outside `allowed_paths`, and `validate_file_path` will return
   `Err(SecurityError::PathOutsideWorkspace)`. This test pins the security
   invariant for the future.

3. **Fix `test_workspace_session_creation`** to compare
   `session.workspace_root().canonicalize()` with `temp_dir.path().canonicalize()`
   — both strings now refer to the same physical path even when one of them
   traverses a system symlink.

## Security analysis

The parent-symlink walk is **strictly more restrictive** than necessary. It
treats any `is_symlink()` parent — including system-managed ones like
`/home → /var/home` (Fedora Silverblue / Universal Blue), `/var → /private/var`
(macOS), `/tmp → /private/tmp` (macOS) — as an attack signal. None of these
are attacker-controlled. The actual symlink-attack threat model is:

> Attacker writes a symlink inside the workspace boundary that points to a
> file outside the boundary (e.g. `/etc/shadow`). The validator then operates
> on the file the symlink resolves to, which is outside the workspace.

The defence against that threat is the **direct-symlink check** plus the
**workspace-boundary check on the canonical target**. The parent-walk adds
no incremental protection: any symlink that escapes the workspace via a
parent component is also caught by the canonical + workspace-boundary check
on the file itself.

After this change, the validator's security properties are:

| Threat | Detection | Status |
|---|---|---|
| File at path is a symlink (regardless of target) | `symlink_metadata(&resolved).is_symlink()` | ✅ retained |
| File's canonical target escapes workspace | `canonical.starts_with(allowed)` | ✅ retained |
| Parent in path is a system-level symlink (e.g. `/home`) | not detected (correctly) | ✅ not a threat |
| Parent in path is an attacker-controlled symlink inside the workspace that escapes | canonical target falls outside `allowed_paths` | ✅ caught by workspace check |

## Trade-offs

- **Removing the parent-walk weakens nothing that matters** but lets system
  symlinks through. If we ever ship to a host where the system symlinks are
  attacker-controlled (e.g. a malicious container init script creates
  `/home -> /tmp/attacker-data`), the workspace boundary check still catches
  escapes via canonical target.
- **We could canonicalize each parent and compare** instead of removing the
  walk. That is more conservative but adds I/O per path and the same
  semantics: any escape via parent ends up outside `allowed_paths` after
  canonicalization. Net: removing is simpler and equivalent.
- **We could use the `dunce` crate** to canonicalize without resolving
  symlinks. That keeps the parent-walk's strictness but masks system
  symlinks. Net: equivalent security properties, adds a dependency, and is
  harder to reason about than "remove redundant check + rely on canonical
  workspace boundary".

## Risks

- **Behaviour change visible to callers**: a CLI run on a host with
  `/home` symlink that previously failed with `Symlink detected in path: /home`
  now succeeds. Net: a **bug fix** for the user, not a regression.
- **Hidden caller assumptions**: any production code that depended on the
  parent-walk's strictness to refuse system symlinks is now more permissive.
  Audit will grep for `SymlinkDetected` call sites and confirm none rely on
  the parent-walk semantics. (The verify-report does this audit.)
- **Workspace boundary check on the file's canonical path must always run**.
  If a future refactor accidentally skips it, we'd lose symlink-attack
  protection. The new regression test (`symlink_inside_workspace_pointing_outside_is_rejected`)
  pins this invariant.

## Acceptance criteria

1. `cargo test -p cognicode-core --lib` reports `2082 passed; 0 failed; 27 ignored`
   (i.e. all 50 currently-red tests are green, 0 newly-red).
2. `cargo test -p cognicode-core --lib security::tests` keeps the 35
   currently-passing security tests green (no regression in the security suite).
3. The new regression test
   `symlink_inside_workspace_pointing_outside_is_rejected` passes.
4. `cargo test -p cognicode-cli --bin cogh` and
   `cargo test -p cognicode-cli --test cognicode_ide_adapter` remain green
   (no collateral).
5. `cargo build --workspace --all-targets` succeeds with no new warnings.

## Refs

- `crates/cognicode-core/src/interface/mcp/security.rs` — `validate_file_path`
  and `validate_path` (the two functions losing the parent-walk).
- `crates/cognicode-core/src/application/workspace_session.rs` —
  `WorkspaceSession::new` canonicalizes the root; the test compares raw
  strings and needs the same canonicalization on the other side.
- Carry-forward debt: DEBT-SDDK-007 (created in e92's verify-report).
