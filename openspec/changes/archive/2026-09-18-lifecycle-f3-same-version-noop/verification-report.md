# lifecycle-F3 — Verification Report (CLOSED)

## Contract
Same-version `update` (tracker == resolved version) must be a **semantic
lifecycle no-op with zero lifecycle mutation**, guarded by a minimal
coherence check so corruption is repaired, not hidden.

## Implementation
- `cmd_update` decision point (crates/cognicode-cli/src/cmd/layout.rs):
  short-circuit when `tracker == resolved.version` AND
  `active_install_is_coherent(home, version)` (version tree exists,
  manifest parses, every declared component has its directory).
- Broken installs fall through to the real pipeline (repair semantics).
- No changes to `InstallerTransaction`: it still means "real transition".
- Commit `04eed537`.

## Verification
| Test | What it pins | Result |
|---|---|---|
| f3_t1_same_version_update_is_zero_mutation | sha256 snapshot of journal/tracker/manifest/tree identical across no-op update (was RED before: journal `committed_at_unix` mutated) | PASS |
| f3_t2 (rollback after no-op) | rollback applies the ORIGINAL transition; first-install no-op rollback cleans the pin | PASS |
| f3_t3_real_version_transition_still_transitions | A→B still transitions: tracker=B, tree B, journal B with previous_tracker=A | PASS |
| f3_t4_broken_same_version_install_is_repaired_not_hidden | damaged component dir → no-op refused, real install repairs | PASS |
| f3_t5_noop_reports_decision | repeated no-op remains zero mutation; decision pinned | PASS |

- cogh bin suite: 286 passed / 0 failed / 1 ignored (two consecutive greens).
- `just lint`: PASS (after clearing pre-existing core clippy debt, `7ddd3251`).
- `cargo fmt --check`: PASS.
- Known-failures baseline: `check_known_failures.py` — observed set matches (39 entries).

## Pre-existing debt cleared en route
- `just lint` was failing on clean `06b6a318` (25 clippy warnings in
  cognicode-core). Fixed mechanically in `7ddd3251` (feature gating of
  `boundary_tests`/`run_dfg`, collapsible_if, split_once, unused imports,
  doc continuation, 2 targeted too_many_arguments allows).

## Release
No release for F3 alone; travels in the next significant release.
