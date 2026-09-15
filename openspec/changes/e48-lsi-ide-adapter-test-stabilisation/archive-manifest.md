# Archive Manifest — e48 — IDE adapter & lifecycle test stabilisation

> Cycle: housekeeping | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e48-lsi-ide-adapter-test-stabilisation` |
| Path | B-direct (housekeeping) |
| Phases completed | explore → spec → tasks → apply (3 WUs) → verify |
| Final status | **ARCHIVED** |
| Base SHA | `0ed56c7f` (post-e47 archive) |
| Diff stat | **+20 / -0** across **3 files** (1 commit, test infrastructure only) |
| Verify verdict | **PASS** (core gate met; residual work documented) |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e48-lsi-ide-adapter-test-stabilisation/exploration-report.md` |
| Spec | `openspec/changes/e48-lsi-ide-adapter-test-stabilisation/spec.md` |
| Tasks | `openspec/changes/e48-lsi-ide-adapter-test-stabilisation/tasks.md` |
| Verification report | `openspec/changes/e48-lsi-ide-adapter-test-stabilisation/verification-report.md` |
| Implementation | commit (this) — `test(cognicode-cli): e48 serialise HOME-mutating tests` |

## What was closed

The 8 thread-safety failures in `ide::tests` (caused by
`unsafe { std::env::set_var("HOME", ...) }` racing under default
parallel execution) are now race-free. While implementing this
cycle, a related set of 8 HOME-mutating tests in
`lifecycle::tests` was also annotated, eliminating the same class
of race in that module.

**Total: 16 `#[serial]` annotations** (8 in `ide.rs` + 8 in `lifecycle.rs`).

| File | Change |
|------|--------|
| `crates/cognicode-cli/Cargo.toml` | +1 line (`serial_test = "3"` in dev-deps) |
| `crates/cognicode-cli/src/cmd/ide.rs` | +9 lines (`use serial_test::serial;` + 8 `#[serial]`) |
| `crates/cognicode-cli/src/cmd/lifecycle.rs` | +9 lines (`use serial_test::serial;` + 8 `#[serial]`) |

## Verification outcome

| Metric | Pre-cycle | Post-cycle | Δ |
|--------|-----------|------------|---|
| `cargo test -p cognicode-cli ide::tests` (parallel) | 20 pass; 0 fail (in isolation, race-prone in full suite) | 20 pass; 0 fail (race-free) | ✅ stable |
| `cargo test -p cognicode-cli` (parallel, default) | 89 pass; 11 fail | 91-97 pass; 3-9 fail (variable) | −2 to −8 failures |
| `cargo test -p cognicode-cli -- --test-threads=1` | (not measured) | 98 pass; 2 fail | n/a |

The 2-7 remaining failures are pre-existing bugs:
- 2 deterministic version-mismatch failures (`test_clean_home_install`,
  `test_install_with_ide_and_profile_dispatches_both`) — bundle manifest
  version `0.94.14` vs `CARGO_PKG_VERSION 0.94.15`.
- Up to 5 residual race failures in `lifecycle::tests` / `install_lock::tests`
  that share process-global resources beyond `HOME` (e.g.,
  `~/.config/cogh/lock.json`).

Both are out of scope for this housekeeping cycle. They are candidates
for a future cycle (e.g., e49: workspace version bump + extended serial
coverage).

## Why no tag

The user explicitly froze v1.0.0 tag cuts (`nada de tag 1.0.0, tenemos
que acabar el roadmap`). This housekeeping cycle is not a release:
- No public API change.
- No production code change.
- Only test infrastructure (dev-dep + test attributes).

A tag here would add noise without information.

## Why no delta-spec

This cycle is pure test infrastructure. It does not add or modify any
spec capability. The `openspec/specs/cognicode-cli/spec.md` and
`openspec/specs/cognicode-ide-adapter/spec.md` REQs that this
addresses are already at `verified` status (per the e43-e47 conformance
work). No delta-spec is needed.

## Verification command

```
cargo test -p cognicode-cli ide::tests
```

Returns `20 passed; 0 failed` under default parallelism. ✅
