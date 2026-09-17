# e86.1 — Verification Report

> e86.1 WU8. Operational authority is `state.yaml`.

## Summary

The e86 followup's three "pinned, not fixed" regressions all
reproduce on `HEAD` and have been fixed. All three are now
asserted with strict tests; the `#[ignore]` is dropped.

## Evidence

### Test suite (cogh)

```text
$ cargo test -p cognicode-cli --bin cogh
test result: ok. 184 passed; 0 failed; 1 ignored; 0 measured
```

Was 180 + 2 ignored in the e86-followup archive. +4 new tests,
-1 ignored, no regressions.

### Breakdown of the +4 tests

| Test | Outcome |
|---|---|
| `cmd_update_zero_component_profile_returns_empty_install_error` (renamed from `..._does_not_pin_tracker`, `#[ignore]` dropped) | NEW (was `#[ignore]`d) |
| `cmd_update_sequential_installs_succeed_cleanly` (strict tripwire) | NEW |
| `test_rollback_reverses_populated_created_dir` (rollback_journal.rs unit test) | NEW |
| `test_rollback_is_idempotent_for_double_created_dir` (rollback_journal.rs unit test) | NEW |

### Cargo fmt / check / clippy / known-failures

- `cargo fmt -p cognicode-cli --check` exits 0 (touched code is
  fmt-clean).
- `cargo check --workspace` exits 0 (warnings only, no errors).
- `just check-known-failures` baseline intact (41 entries).

### Live smoke

```
$ /var/home/rubentxu/cargo-targets/debug/cogh --version
cogh 0.95.0

$ /var/home/rubentxu/cargo-targets/debug/cogh --help | head -5
CogniCode version manager

Usage: cogh [OPTIONS] <COMMAND>

Commands:
  install    Install a plugin at a specific version
```

## Observations / honesty

### Bug 2 (stale-shim) DOES reproduce on HEAD

I initially misread `cmd_update_sequential_installs_overwrite_cleanly`
as passing via the `Ok` arm. It actually passes via the `Err` arm
(the conditional match accepts both outcomes, and the Err arm
asserts the message contains "shim install error" + "symlink" +
shim_path_str — exactly what it gets). A debug `eprintln`
revealed this. So Bug 2 reproduces.

### Test count summary

| | Before e86.1 | After e86.1 | Δ |
|---|---|---|---|
| `cargo test -p cognicode-cli --bin cogh` | 180 passed | 184 passed | +4 |
| `#[ignore]`d | 2 | 1 | -1 |

The +1 ignored test that remains
(`test_cogh_install_runs_successfully`) is the pre-existing
bundle-version-mismatch pinning (touches
`crate::lifecycle::tests::test_cogh_install_runs_successfully`).
It is unrelated to this cycle.

## Out-of-scope findings (carried forward)

- Pre-existing `boundary_tests.rs` fmt drift (`14a3a721`, e79).
- Pre-existing CLI help mis-descriptions (Class C from e84.1 WU19).
- The `cmd_update` real download path.
- The `ResolverFixture` shared fixture refactor.
- Clippy `-D warnings` (pre-existing).
- The `cogh install` pre-existing `#[ignore]`d test (bundle
  version mismatch).

## Exit gate verdict

| Gate | Status |
|---|---|
| Bug 1 fix: rollback reverses populated `CreatedDir` | PASS |
| Bug 2 fix: `install_shim` is idempotent on re-install | PASS |
| Bug 3 fix: zero-component profile returns `Err(EmptyInstall)` | PASS |
| `cmd_rollback_after_live_install` strict-success | PASS |
| `cmd_update_zero_component_profile_returns_empty_install_error` strict-error | PASS |
| `cmd_update_sequential_installs_succeed_cleanly` strict-success | PASS |
| `test_rollback_reverses_populated_created_dir` (new unit) | PASS |
| `test_rollback_is_idempotent_for_double_created_dir` (new unit) | PASS |
| `cargo fmt -p cognicode-cli --check` | PASS |
| `cargo check --workspace` | PASS |
| `just check-known-failures` | PASS (41 intact) |
| `python3 scripts/validate_skills.py` | PASS |
| `bash scripts/verify-skills.sh` | PASS |
| `cogh --version` smoke | PASS (0.95.0) |

## Commits

- TBD at archive time.

## Carried forward

None from this cycle. (The pre-existing items in the table above
were already carried forward; this cycle does not add new
carry-forward items.)
