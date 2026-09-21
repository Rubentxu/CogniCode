# Apply Receipt — e86-3-cogh-uninstall-coverage

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/e86-3-cogh-uninstall-coverage` |
| Path | a-min (propose → apply → verify → archive) |
| Date | 2026-09-21 |
| Actor | jcode-orchestrator |
| Status | **APPLIED — all 5 unit tests green at HEAD `1fefc3d5`** |

## Outcome

The 5 unit tests called for in `proposal.md` REQ-UC-01 (REQ-LJ-11..15)
were already present in `crates/cognicode-cli/src/cmd/lifecycle.rs`
under `tests::t_e86_3_*` and pass at HEAD. This cycle closes the
formal-spec + verification-report + archive gap so the e86-3 cycle
reaches `archive-closed-with-receipt` parity with e86-1 (which is
fully archived at `openspec/changes/archive/2026-09-17-e86-1-...`).

No code changes were required. The cycle is documentation-only:
1. Spec delta added (this directory: `specs/lifecycle-journal-uninstall-coverage/spec.md`).
2. `cognicode-lifecycle/spec.md` was NOT mutated; the new requirements
   live in the cycle's own spec file (consistent with the e86-1 pattern
   of spec deltas living under `changes/archive/.../specs/` rather than
   being merged into the canonical spec immediately — that is reserved
   for the broader umbrella sync).
3. Receipt + verification + apply evidence is filed in this cycle's
   directory; `sddk-archive` will move the whole tree to
   `archive/2026-09-21-e86-3-.../` after verification.

## Acceptance verdicts

| REQ | Status | Evidence |
|---|---|---|
| REQ-LJ-11 | ✅ PASS | `t_e86_3_uninstall_errors_on_uninitialized_home` GREEN at HEAD `1fefc3d5` |
| REQ-LJ-12 | ✅ PASS | `t_e86_3_uninstall_idempotent_second_call` GREEN at HEAD `1fefc3d5` |
| REQ-LJ-13 | ✅ PASS | `t_e86_3_uninstall_opencode_handles_missing_config_file` GREEN at HEAD `1fefc3d5` |
| REQ-LJ-14 | ✅ PASS | `t_e86_3_uninstall_without_ide_prints_helpful_message` GREEN at HEAD `1fefc3d5` |
| REQ-LJ-15 | ✅ PASS | `t_e86_3_uninstall_unknown_ide_errors_cleanly` GREEN at HEAD `1fefc3d5` |
| REQ-UC-02 | ✅ PASS | `uninstall_opencode_ide_removes_entry_and_skills` GREEN (existing test, unchanged) |
| REQ-UC-03 | ✅ PASS | All 287 cogh tests pass (282 baseline + 5 new); see verification-report.md |
| REQ-UC-04 | ✅ PASS | REQ-LJ-11..15 documented in `specs/lifecycle-journal-uninstall-coverage/spec.md` |
| REQ-UC-05 | ⚪ N/A | No real bug fired in the 5 tests; the proposal's "if a real bug is found" branch did not trigger |
| REQ-UC-06 | ✅ PASS | `cargo fmt --check` exit 0, `cargo clippy --workspace --all-targets -- -D warnings` exit 0 (with the established per-file baseline allows), `scripts/check_known_failures.py` exit 0 |

## Tests added (verbatim, no diff vs current main)

```rust
#[test] #[serial] fn t_e86_3_uninstall_errors_on_uninitialized_home()
#[test] #[serial] fn t_e86_3_uninstall_idempotent_second_call()
#[test] #[serial] fn t_e86_3_uninstall_opencode_handles_missing_config_file()
#[test] #[serial] fn t_e86_3_uninstall_without_ide_prints_helpful_message()
#[test] #[serial] fn t_e86_3_uninstall_unknown_ide_errors_cleanly()
```

All five live in `crates/cognicode-cli/src/cmd/lifecycle.rs` under
`#[cfg(test)] mod tests`.

## Why no code changes

The 5 tests pin `cmd_uninstall`'s behaviour against a path that
already exists in `crates/cognicode-cli/src/cmd/layout.rs` (lines
247-329). The path was hardened in e86.1 for rollback + install but
not for the 5 uninstall-specific failure modes above. The tests
were added when the implementation already supported the desired
behaviour — i.e. this cycle is a "pin the contract" cycle, not a
"fix the bug" cycle.

The risk called out in `proposal.md` §Risks ("the uninstall path
may not have the same bug surface as install") did materialise in
mild form: REQ-LJ-13 (missing config file) and REQ-LJ-14 (no IDE)
test structural-applicability rather than specific bug patterns,
because the install-side bug patterns (CreatedDir idempotency,
shim re-install, empty-profile) are install-only and have no
uninstall-side analogue. The tests still pin the structural
contract and the proposal's "marked // not-applicable" escape
hatch was not needed.

## Sequencer

E86.2 (closed archive-partial) → **E86.3 (closed here)** → E86.4
(next). E86.3's pattern of "tests already present, spec added,
archive closed" mirrors E86.1's pattern. E86.4 will need real code
work (journal upgrade + CLI surface), so it is a heavier cycle.

## Cross-references

- Spec: `specs/lifecycle-journal-uninstall-coverage/spec.md`
- Proposal: `../proposal.md`
- Verification: `../verification-report.md`
- Test source: `crates/cognicode-cli/src/cmd/lifecycle.rs` (lines
  1270, 1308, 1377, 1410, 1430)
