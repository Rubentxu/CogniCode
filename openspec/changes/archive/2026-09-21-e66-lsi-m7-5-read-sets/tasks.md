# e66 Tasks — M7.5 Read-Set Foundation

Single work unit, single commit, single verify.

## WU1: ReadSet domain type + recorder + invalidation + tests

| Step | Task | Acceptance |
|------|------|------------|
| 1 | Verify `domain/mod.rs` has `ports/` module — add if missing | `grep -q 'pub mod ports' src/domain/mod.rs` |
| 2 | Create `src/domain/ports/mod.rs` exporting `read_set_recorder` | file exists, `pub mod read_set_recorder;` |
| 3 | Create `src/domain/readset.rs` with `FactId` + `ReadSet` + `ReadSetError` | file exists, no warnings |
| 4 | Create `src/domain/ports/read_set_recorder.rs` with trait + impl | file exists, compiles |
| 5 | Add `pub mod readset;` to `src/domain/mod.rs` | grep returns match |
| 6 | Add test module `#[cfg(test)] mod tests` with 3 tests | all pass |
| 7 | Run `cargo test --package cognicode-core --lib readset` | 3/3 PASS |
| 8 | Run `cargo clippy --package cognicode-core --all-targets -- -D warnings` | no warnings |
| 9 | Run `cargo fmt --package cognicode-core` | no diff |
| 10 | Commit: `feat(lsi): add ReadSet + ReadSetRecorder + InvalidationQuery (UAT-U61)` | 1 commit on feat/e66-lsi-m7-5-read-sets |

## Out of work-unit scope

- Behavior authority runtime integration
- Cache integration
- Other UAT milestones
- Documentation (will be in a follow-up commit if needed)

## Acceptance gate (verify phase)

- `cargo test --package cognicode-core --lib readset` → 3/3 PASS
- `cargo clippy -p cognicode-core --all-targets -- -D warnings` → PASS
- `cargo fmt --check -p cognicode-core` → PASS
- Working tree diff is restricted to ~5 files
