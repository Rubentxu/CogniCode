# Archive Manifest — e93 (DEBT-SDDK-007): InputValidator parent-symlink walk produces false positives

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/2026-09-21-e93-sddk-007-symlink-parent-walk` |
| Path | a-min (security-critical change to canonical validator) |
| Date | 2026-09-21 |
| Actor | jcode-orchestrator |
| Status | **CLOSED — `archive-closed-with-commit`** |
| Closes | DEBT-SDDK-007 (parent-symlink walk producing false positives) |
| Severity | security-critical (validator hot path) |
| Files touched | 2 production / 2 test (security.rs, workspace_session.rs) |

## Outcome

CLOSED. The cycle recovered 50 reproducibly-failing tests in
`cognicode-core --lib` that had been red since the very first test run
on this host (where `/home → /var/home` is a system-managed symlink
and Jcode's runtime `TMPDIR=/home/rubentxu/.jcode/scratch`).

Root cause: `InputValidator::validate_file_path` and
`InputValidator::validate_path` walked every parent component of the
resolved path and rejected any whose `symlink_metadata` reported
`is_symlink()`. On a host with `/home` as a system symlink, every
`TempDir::new()` puts test fixtures under `/home/...`, the parent-walk
hits `/home`, and the validator rejects it with
`Symlink detected in path: /home`.

That parent-walk is **redundant** with the canonical-form workspace
boundary check that already runs after: any attacker-controlled parent
symlink that escapes the workspace would cause the canonicalized
target to fall outside `allowed_paths`, and the workspace boundary
check already rejects that. The only thing the parent-walk actually
caught that the workspace check missed was system-level symlinks, which
are not attacker-controlled.

## Security analysis (preserved in `proposal.md`)

| Threat | Detection | Status |
|---|---|---|
| File at path is a symlink (regardless of target) | `symlink_metadata(&resolved).is_symlink()` | ✅ retained |
| File's canonical target escapes workspace | `canonical.starts_with(allowed)` | ✅ retained |
| Parent in path is a system-level symlink (e.g. `/home`) | not detected (correctly) | ✅ not a threat |
| Parent in path is an attacker-controlled symlink inside the workspace that escapes | canonical target falls outside `allowed_paths` | ✅ caught by workspace check |

## Acceptance verdicts

| REQ | Status | Evidence |
|---|---|---|
| REQ-E93-01 (`cargo test -p cognicode-core --lib` ≥2082 passed; 0 failed; 27 ignored) | ✅ PASS | 2083 passed; 0 failed; 27 ignored (+51 from baseline) |
| REQ-E93-02 (security suite ≥35 passed; 0 failed) | ✅ PASS | 36 passed; 0 failed (35 retained + 1 new regression) |
| REQ-E93-03 (regression test `symlink_inside_workspace_pointing_outside_is_rejected` passes) | ✅ PASS | New test passes; pins the security invariant |
| REQ-E93-04 (CLI collateral: cogh + ide_adapter green) | ✅ PASS | cogh 291/0/1; ide_adapter 7/0/0 |
| REQ-E93-05 (`cargo build --workspace --all-targets` no new warnings) | ✅ PASS | Same warning set as pre-cycle baseline |

## Commits produced

| Commit | Description |
|---|---|
| `f902be2f` | `fix(security): e93 / DEBT-SDDK-007 — remove redundant parent-symlink walk` |

## Diff summary

```text
crates/cognicode-core/src/interface/mcp/security.rs
  ~ InputValidator::validate_file_path: removed parent-symlink walk
  ~ InputValidator::validate_path (main path): removed parent-symlink walk
  ~ InputValidator::validate_path (NotFound fallback): removed parent-symlink check
  ~ test_symlink_parent_directory_rejected
      → renamed to test_symlink_parent_directory_inside_workspace_accepted
        (asserts the NEW contract: parent symlink inside workspace is accepted)
  + test_symlink_inside_workspace_pointing_outside_is_rejected
      (regression test for the security invariant)

crates/cognicode-core/src/application/workspace_session.rs
  ~ test_workspace_session_creation: compare canonical paths on both sides
```

No public API changed. No new dependencies. No production code logic
change outside the documented removal.

## Audit

- **Security invariant pinned by `test_symlink_inside_workspace_pointing_outside_is_rejected`**:
  a symlink inside the workspace that escapes it is still rejected by
  the direct-symlink check (fast path) and would also be caught by the
  canonical workspace boundary check if the direct check were bypassed.
- **No production code depends on the parent-walk's semantics** — grep
  for `SymlinkDetected` matches only the direct-symlink check that was
  retained, and the renamed/new tests. No `match` arm in production code
  is keyed to a parent-walk-only signal.
- **35 of 38 pre-cycle security tests retained as green**; the 3 that
  were red (workspace-boundary-enforcement, validate-path-before-op,
  validate-nonexistent) are now green because their fixture setup
  (`TempDir::new()` under TMPDIR) no longer hits the parent-walk.

## Decision

**PASS — CLOSED.** The cycle's scope (recover ~50 reproducibly failing
`cognicode-core` tests by removing a redundant security check that
produced false positives on system-managed symlinks) is fully verified
and committed. Security invariants are preserved and pinned by a new
regression test. DEBT-SDDK-007 is closed.
