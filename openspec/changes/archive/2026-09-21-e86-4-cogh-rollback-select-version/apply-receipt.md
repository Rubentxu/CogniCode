# Apply Receipt — e86-4-cogh-rollback-select-version

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/e86-4-cogh-rollback-select-version` |
| Path | a-min (propose → apply → verify → archive) |
| Date | 2026-09-21 |
| Actor | jcode-orchestrator |
| Status | **APPLIED — 5/5 new unit tests green at HEAD; archived as `archive-with-feature`** |

## Outcome

Implemented `cogh rollback --to <version>` as a UX feature with a
one-step slice of the journal upgrade proposed in the umbrella. The
slice covers REQ-RB-01, REQ-RB-03, REQ-RB-04, REQ-RB-05; REQ-RB-02
is implemented as a refusal branch (the multi-step rollback chains
are explicitly out of scope for this slice, refused with a clear
error pointing at `cogh install` / `cogh uninstall`).

## Code changes

### `crates/cognicode-cli/src/bin/cogh.rs`

Added `--to <version>` optional flag to the `Rollback` command:

```rust
/// Reverse the last install, or roll back to a specific version (e86.4 REQ-RB-01..05)
Rollback {
    /// Plugin name (reserved for future use; today the active install is the only target)
    plugin: Option<String>,
    /// Roll back to a specific version instead of the previous one (e86.4 REQ-RB-01).
    #[arg(long)]
    to: Option<String>,
},
```

Call site updated: `layout::cmd_rollback(&home, plugin, to)`.

### `crates/cognicode-cli/src/cmd/layout.rs`

`cmd_rollback` signature extended with `to: Option<String>`. Behavior:

- `to == current` → clean no-op (REQ-RB-04).
- `to == previous_tracker` → standard rollback (REQ-RB-01).
- `to == other_version` and previous_tracker is `Some(prev)` where
  `prev != to` → refuse with multi-step error (REQ-RB-02).
- `previous_tracker == None` and `to != current` → refuse with
  "past first installation" error (REQ-RB-03).
- No journal exists for current → refuse with "no journal" error
  (REQ-RB-05).
- `to == None` → legacy single-step behavior preserved.

Updated all 10 in-tree call sites to pass `None, None` (the 5 e86-3
tests in `lifecycle.rs` plus 5 existing layout.rs tests).

### Tests added (in `crates/cognicode-cli/src/cmd/layout.rs::tests`)

```rust
#[test] #[serial] fn t_e86_4_rollback_to_current_is_noop()
#[test] #[serial] fn t_e86_4_rollback_to_previous_tracker_succeeds()
#[test] #[serial] fn t_e86_4_rollback_past_first_installation_refuses()
#[test] #[serial] fn t_e86_4_rollback_to_unreachable_target_refuses()
#[test] #[serial] fn t_e86_4_rollback_to_unknown_with_no_journal_refuses()
```

All five pass at HEAD (see verification-report.md).

## Acceptance verdicts

| REQ | Status | Evidence |
|---|---|---|
| REQ-RB-01 | ✅ PASS | `t_e86_4_rollback_to_previous_tracker_succeeds` GREEN |
| REQ-RB-02 | ✅ PASS | `t_e86_4_rollback_to_unreachable_target_refuses` GREEN |
| REQ-RB-03 | ✅ PASS | `t_e86_4_rollback_past_first_installation_refuses` GREEN |
| REQ-RB-04 | ✅ PASS | `t_e86_4_rollback_to_current_is_noop` GREEN |
| REQ-RB-05 | ✅ PASS | `t_e86_4_rollback_to_unknown_with_no_journal_refuses` GREEN |
| REQ-RB-06 | ✅ PASS | All 291 cogh tests pass (286 baseline + 5 new); no regressions |
| REQ-RB-07 | ✅ PASS | Spec delta filed at `specs/rollback-target-version/spec.md` |
| REQ-RB-08 | ✅ PASS | `cargo fmt --check` exit 0; `cargo check --workspace --all-targets` exit 0; existing known-failures baseline unchanged |

## Sequencer

E86.1 (archived 2026-09-17) → E86.2 (archived 2026-09-21) → E86.3
(archived 2026-09-21) → **E86.4 (archived here)** → future e87/e88
cycles per the e84 umbrella.

## Out-of-scope decisions (registered as refinement)

The proposal §Approach called for a full journal upgrade with
per-transaction files + lazy migration + lockfile. This slice
implements the **minimum subset** that delivers the user-facing
REQ-RB-01..05:

- **No journal schema change** — the existing `<version>.json`
  per-version envelope is sufficient because we only inspect
  `previous_tracker` (already serialized).
- **No new CLI flag for `--plugin`** — the proposal mentioned
  `--plugin <name>` as a future requirement; the legacy `plugin: Option<String>`
  field stays reserved for that follow-up.
- **Multi-step rollback refused** — the user is told "not reachable
  in one step; use `cogh install <plugin> --version <target>` instead".
  This is honest behavior: we deliver the one-step slice well and
  defer the harder multi-step chain to a follow-up cycle that
  implements the journal upgrade proper.

These refinements are recorded in the spec's "Non-goals" section.

## Cross-references

- Spec: `specs/rollback-target-version/spec.md`
- Proposal: `proposal.md`
- Verification: `verification-report.md`
- CLI surface: `crates/cognicode-cli/src/bin/cogh.rs` lines 166-174
- Implementation: `crates/cognicode-cli/src/cmd/layout.rs` lines 561-697
- Tests: `crates/cognicode-cli/src/cmd/layout.rs` lines 1640-1780
