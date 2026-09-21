# Tasks — e46 — cognicode-lifecycle conformance

> Change: `e46-lsi-cognicode-lifecycle-conformance` | Phase: tasks | Date: 2026-09-15

## Work units

### WU-1 — Create `crates/cognicode-cli/tests/cognicode_lifecycle.rs`

RED-first test file covering the 7 scenarios documented in `spec.md`:

| # | Test | REQ scenario |
|---|------|--------------|
| 1 | `cogh_doctor_reports_healthy_on_initialised_home` | doctor PASS on healthy install |
| 2 | `cogh_doctor_reports_uninitialised_home_on_fresh_dir` | doctor FAIL on broken home |
| 3 | `cogh_doctor_warns_when_tracker_version_missing` | doctor warns when tracker missing |
| 4 | `cogh_reshim_emits_recognisable_message_on_current_implementation` | reshim recreates shims (contract) |
| 5 | `cogh_uninstall_emits_recognisable_message_for_known_plugin` | uninstall preserves other versions (contract) |
| 6 | `cogh_list_reports_installed_plugins_after_init` | list shows installed + available (lifecycle hook) |
| 7 | `cogh_current_reports_unpinned_state_on_initialised_home_without_tracker` | current shows pinned version (lifecycle hook) |

Implementation notes:

- Reuse the e43/e44/e45 subprocess pattern: `Command::new(env!("CARGO_BIN_EXE_cogh"))` + `--home <temp>`.
- Helper `run_cogh(home, args)` (mirrors e45).
- For tests 1, 6, 7: run `cogh init --home <dir>` first to populate the home.
- For test 3: init then `rm -f <dir>/tracker/version` to force the warning.

### WU-2 — Verify locally

```
cargo test -p cognicode-cli --test cognicode_lifecycle
```

Expected: 7 passed; 0 failed.

### WU-3 — Update conformance evidence_map

Add an entry for `cognicode-lifecycle`:

```yaml
cognicode-lifecycle:
  status: verified
  evidence_path: crates/cognicode-cli/tests/cognicode_lifecycle.rs
  evidence_note: 'e46 cognicode-lifecycle conformance: 7 integration tests covering REQ #4 (uninstall emits recognised descriptor — contract), REQ #7 (doctor healthy + uninitialised + tracker-warn), REQ #8 (reshim emits recognised message — contract), REQ #9 (current reports unpinned state — lifecycle hook), REQ #10 (list reports installed plugins — lifecycle hook); the 5 network-dependent specs (#1 idempotency, #2 atomicity, #3 update reversibility, #5 lockfile pinning, #6 update respects lock) are out of scope; the unimplemented doctor checks (broken shim + plugin manifest validity) are deferred.'
```

Re-run `python3 sandbox/scripts/openspec_conformance.py` and confirm `pct_verified` lifts from `96.1%` to `≥ 97.5%`.

## Sequencing

Apply WU-1, verify WU-2, then apply WU-3 (doc-only).

## Acceptance gate

- `cargo test -p cognicode-cli --test cognicode_lifecycle` passes 7/7.
- `openspec_conformance.py` reports `pct_verified ≥ 97.5%`.
- No production code change (test-only cycle).
