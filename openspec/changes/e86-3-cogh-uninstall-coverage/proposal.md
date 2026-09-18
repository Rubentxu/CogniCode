# e86.3 — cogh Uninstall Coverage (proposal)

> Program: cognicode-distribution (umbrella: e84 contract + e85 release + e86 lifecycle)
> Milestone: M2.3 — close the unit-coverage gap on `cogh uninstall`
> Phase: propose | Date: 2026-09-18
> Delivery: bounded cycle, single slice, GATED by E86.2 receipt (no real-bug fired yet we proceed; if E86.2 surfaces a bug, this list absorbs it)

## Intent

e86.1 remediated three pinned bugs in **install** + **rollback**:
  1. rollback populated `CreatedDir` (uses `remove_dir_all`, idempotent)
  2. stale shim on sequential install (LinuxAdapter/MacOsAdapter install_shim
     detects existing symlink and no-ops on match)
  3. zero-component profile silent install (`InstallerError::EmptyInstall`)

The `cogh uninstall` command (declared in `crates/cognicode-cli/src/bin/cogh.rs`,
delegates to `layout::cmd_uninstall`, has ONE test
`uninstall_opencode_ide_removes_entry_and_skills` in `crates/cognicode-cli/src/cmd/lifecycle.rs:276`)
was NOT covered with the equivalent regression suite. There is no guarantee
that the same three bug patterns cannot fire in `cmd_uninstall`'s parallel
side-effect paths.

This cycle writes that suite so we can claim uninstall coverage parity with
install/rollback.

## Scope

IN scope:
- 5 new unit tests under `crates/cognicode-cli/src/cmd/lifecycle.rs`
  (or a new file `crates/cognicode-cli/src/cmd/uninstall_regression.rs`
  if test-suite size warrants):
  1. `cmd_uninstall_with_populated_install_dir_does_not_panic` —
     installs into a populated `~/.cognicode/`, then uninstalls; expect all
     stale entries removed, no `IO_ERROR` on a non-empty dir.
  2. `cmd_uninstall_is_idempotent_for_double_uninstall` — uninstall twice,
     second call is a clean no-op (doesn't crash on already-removed files).
  3. `cmd_uninstall_after_stale_shim_via_sequential_installs_recovers_cleanly` —
     installs v1 (creates shim), installs v2 (overwrites shim), uninstall v2;
     shim removed (not pointing at v1's now-removed binary).
  4. `cmd_uninstall_zero_component_profile_returns_empty_install_error` —
     triggered in install phase so already returns `EmptyInstall` BEFORE any
     pin/tracker/journal write occurs; uninstall is then a no-op.
  5. `cmd_uninstall_writes_rollback_journal_and_can_rollback_itself` —
     ensure the uninstall sequence is a reversible transaction (mirror
     `cmd_rollback_after_live_install`'s tripwire) so a botched uninstall
     restores pre-uninstall state.
- One lifecycle test (the existing template in `lifecycle.rs:635`) for
  end-to-end subprocess UAT mirroring the e86.1 + e86-followup pattern.
- Spec delta: REQ-LJ-11..15 added to `lifecycle-journal-populated-dir-empty-profile/spec.md`
  mirroring REQ-LJ-04..10.

OUT of scope:
- Removing the existing `uninstall_opencode_ide_removes_entry_and_skills`
  test (it stays; the new tests are additive).
- Changes to `cmd_uninstall`'s contract (REMOVE semantics stay; only test
  coverage is added).
- Real-PC UAT (E86.2 owns that).
- Rollback-to-version support (E86.4).

## Approach

Read first:
  - `crates/cognicode-cli/src/cmd/installer_transaction.rs` (the install
    pipeline that e86.1 hardened — most patterns mirror to uninstall).
  - `crates/cognicode-cli/src/cmd/rollback_journal.rs::rollback()` (the
    shared reverse machinery).
  - `crates/cognicode-cli/src/cmd/layout.rs::cmd_uninstall` (the seam we
    are covering).
  - `crates/cognicode-cli/src/cmd/lifecycle.rs:635` (test harness template).
  - `openspec/specs/cognicode-lifecycle/spec.md` for existing REQ conventions.

Write the 5 unit tests RED-first (proven fail before the matching test
fixture is added to the harness if any is missing). Then apply the same
harness augmentation e86.1 used (TempDir per test, install fixture
mutation kept off `/tmp`).

If any test reveals an actual bug (not just missing coverage), spawn a
follow-up cycle (E86.3.1) the same way e86.1 was spawned.

## Acceptance contract

| REQ | Observable |
|---|---|
| REQ-UC-01 | The 5 unit tests above pass; each has a RED-first proof in the apply phase receipt |
| REQ-UC-02 | The existing `uninstall_opencode_ide_removes_entry_and_skills` test still passes (no regression) |
| REQ-UC-03 | All 184 existing cogh tests still pass (post-e86.1 baseline + 5 new = 189 expected) |
| REQ-UC-04 | Spec delta REQ-LJ-11..15 added under `lifecycle-journal-populated-dir-empty-profile/spec.md` and consumed by the new tests |
| REQ-UC-05 | If a real bug is found, it becomes REQ-UC-06 with a reproducer; the bug fix is shipped in this cycle (not a follow-up) |
| REQ-UC-06 | Workspace lint, fmt, known-failures checker stay green |

## Risks

- **The uninstall path may not have the same bug surface as install** —
  if a test scenario is structurally inapplicable (e.g. zero-component
  profile is install-only), the test must be marked `// not-applicable:
  see proposal` and skipped, with a one-line justification in the
  proposal. Do NOT delete the test; deletion is a separate decision.
- **Subprocess tests are slow** — limit to ONE e2e lifecycle test, mirror
  `cmd_rollback_after_live_install`, drive it via `LocalRelease` +
  `TempBaseUrl` (already used in e86-followup) not against the real
  network.
- **Test count budget** — the e86.1 baseline was 147 cogh tests; the
  budget for this cycle is +5 (REQs UC-01..02), +1 (UC-03 e2e). Anything
  beyond +6 needs an explicit reason in the apply report.

## Deliverables

1. `crates/cognicode-cli/src/cmd/lifecycle.rs` — +6 new tests (5 unit + 1 e2e).
2. `openspec/specs/cognicode-lifecycle/spec.md` — REQ-LJ-11..15.
3. `openspec/changes/e86-3-cogh-uninstall-coverage/specs/lifecycle-journal-uninstall-coverage/spec.md` —
   mirror cycle spec for traceability.
4. Apply receipt + verification report (the e86 family pattern).

## Non-goals

- Generalising `cmd_uninstall` to be a "full reverse of any install"
  (that is partial overlap with E86.4's rollback-to-version work — keep
  them separate or E86.4 swallows E86.3).
- Removing/cleaning `dev-bundle.yaml` seam (out of scope since e85).

## Sequencer

Independently runnable IF E86.2 has not surfaced a bug that needs E86.3's
test to be redesigned. Safe ordering: E86.2 first (1 day), then E86.3 in
parallel with E86.4 design.
