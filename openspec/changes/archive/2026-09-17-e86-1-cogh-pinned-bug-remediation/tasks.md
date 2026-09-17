# e86.1 — Tasks

> Operational authority is `state.yaml`.

| WU | Task | Outcome |
|---|---|---|
| WU0 | Audit the 3 pinned regressions on `HEAD`; confirm which reproduce | DONE — All 3 reproduce |
| WU1 | Fix Bug 1: `CreatedDir` reversal uses `remove_dir_all` + idempotent | DONE — `rollback_journal.rs:262` |
| WU2 | Fix Bug 2: `LinuxAdapter`/`MacOsAdapter::install_shim` idempotent on re-install | DONE — `platform_adapter.rs` |
| WU3 | Fix Bug 3: `InstallerError::EmptyInstall` variant + early return in `installer_transaction::run` | DONE — `error.rs` + `installer_transaction.rs` |
| WU4 | Tighten `cmd_rollback_after_live_install` to strict `expect` | DONE — `layout.rs:1434` |
| WU5 | Drop `#[ignore]` on `cmd_update_zero_component_profile_does_not_pin_tracker`, rename to `..._returns_empty_install_error`, assert strict `expect_err` | DONE — `layout.rs:1226` |
| WU6 | Add `cmd_update_sequential_installs_succeed_cleanly` (strict tripwire) | DONE — `layout.rs:1379` |
| WU7 | Add unit tests for new `CreatedDir` reversal semantics | DONE — `test_rollback_reverses_populated_created_dir` + `test_rollback_is_idempotent_for_double_created_dir` |
| WU8 | Gate sweep | DONE — 184 cogh tests pass; cargo check exits 0; cargo fmt -p cognicode-cli exits 0; validator PASS |

## Out of scope (carried forward)

- `boundary_tests.rs` fmt drift (pre-existing e79).
- CLI help mis-descriptions (Class C from e84.1 WU19).
- `cmd_update` real download path.
- `ResolverFixture` shared fixture refactor.
- Clippy `-D warnings` (pre-existing).
