# e86.1 — cogh pinned-bug remediation

## Why this cycle

The e86 followup (`openspec/changes/archive/2026-09-17-e86-followup-live-update/`)
shipped three regressions, all **pinned, not fixed**. The followup
verification report explicitly says: "**This trio must be fixed
before e86 can ship.**" All three reproduce on `HEAD`.

This cycle fixes all three and tightens the tests to assert the
correct behaviour.

## What this cycle delivers

1. **Bug 1 (rollback populated-dir)**: change `CreatedDir` reversal
   in `rollback_journal.rs` to `remove_dir_all`. Make it
   **idempotent**: if a sibling reversal already removed the path,
   the reversal is a no-op (the install-dir is recorded twice —
   once for the install dir during extracting, once for the
   manifest parent during writing-manifest — and LIFO reverses the
   second entry first, sweeping the first entry's path).

2. **Bug 2 (stale shim)**: in `LinuxAdapter::install_shim` and
   `MacOsAdapter::install_shim`, detect the existing link before
   `symlink`. If the existing link points at the same target,
   no-op. If it points elsewhere (or is not a link), remove and
   re-link. The `WindowsAdapter` was already idempotent (uses
   `fs::copy` which overwrites).

3. **Bug 3 (zero-component profile)**: in `installer_transaction::run`,
   detect the empty-filtered-component case BEFORE the commit path
   and return a new `InstallerError::EmptyInstall { profile,
   version }` variant. `cmd_update` matches on this and refuses
   to write the tracker, the lifecycle journal, or the install
   manifest.

## Bounded scope

- No new features.
- No architecture changes.
- No new external dependencies.
- One new spec variant: `REQ-LJ-04` (empty-profile error).
- Spec delta: `lifecycle-journal-populated-dir-empty-profile/spec.md`
  adds `REQ-LJ-04..08` (5 requirements).

## Non-goals

- e85 (real artifact publishing).
- e87 (external channels).
- e88 (real Linux install UAT).
- Control Plane / Backstage.
- New MCP tools.
- Skill install (`cogh skill install`).
- Architecture execution (e77).

## Verification

- `cmd_rollback_after_live_install` becomes a strict `expect()`.
- `cmd_update_sequential_installs_overwrite_cleanly` (the
  conditional match) is kept for diagnostic value.
- `cmd_update_sequential_installs_succeed_cleanly` is added as
  a strict-success tripwire (fails loudly if Bug 2 ever
  resurfaces).
- `cmd_update_zero_component_profile_does_not_pin_tracker`
  (`#[ignore]`d) is replaced by
  `cmd_update_zero_component_profile_returns_empty_install_error`
  with strict `expect_err` and `#[ignore]` dropped.
- New unit tests `test_rollback_reverses_populated_created_dir`
  and `test_rollback_is_idempotent_for_double_created_dir` in
  `rollback_journal.rs`.

cogh count: 180 + 2 ignored → 184 + 1 ignored (the
`test_cogh_install_runs_successfully` bundle version mismatch
that already existed).

## Risks

- The `remove_dir_all` change in Bug 1 is a semantic shift. Any
  test that depends on the empty-only behaviour would break.
  Pre-audit: 12 rollback_journal tests + 3 cmd_rollback tests;
  all pass on `HEAD` with the new code.
- The idempotent shim fix changes the journal semantics: a no-op
  re-link records the same `CreatedSymlink` as before. Rollback
  is unchanged.
- The new `InstallerError::EmptyInstall` variant is exhaustive.
  Audit callers before merging.

## Out of scope (carried forward, not addressed)

- The pre-existing `boundary_tests.rs` fmt drift (`14a3a721`, e79).
- The pre-existing CLI help mis-descriptions (Class C from e84.1 WU19).
- The `cmd_update` real download path (today dry-run + plumbing).
- The `ResolverFixture` shared fixture refactor.
- Clippy `-D warnings` (pre-existing).
- The pre-existing `cogh install` `#[ignore]`d test (bundle
  version mismatch).
