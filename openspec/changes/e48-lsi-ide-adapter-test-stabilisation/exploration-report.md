# Exploration Report — e48 — IDE adapter test stabilisation

> Change: `e48-lsi-ide-adapter-test-stabilisation` | Phase: explore | Date: 2026-09-15

## Problem

`crates/cognicode-cli/src/cmd/ide.rs` has 20 unit tests, 8 of which
mutate the process `HOME` env var via `unsafe { std::env::set_var("HOME", ...) }`.
The mutation is **not thread-safe** with other tests that read `HOME`
in parallel, so the test suite only passes with
`cargo test -- --test-threads=1`. This is a real CI hazard: any
contributor who runs `cargo test` on the crate in parallel mode will
see flaky failures, and the CI matrix that doesn't already pin
`--test-threads=1` will report false negatives.

The 8 affected tests are listed below (all live in `ide::tests`):

| Line | Test |
|------|------|
| 763 | `integrate_opencode_writes_mcp_entry` |
| 808 | `uninstall_opencode_removes_entry` |
| 849 | `integrate_zcode_creates_mcp_section` |
| 877 | `uninstall_zcode_removes_entry` |
| 912 | `integrate_claude_writes_mcp_file` |
| 934 | `uninstall_claude_removes_mcp_file` |
| 959 | `integrate_codex_inserts_mcp_server` |
| 993 | `uninstall_codex_removes_entry` |

All 8 read `prev_home`, set `HOME` to a per-test tempdir, run the
production `cmd_*` function, then restore `HOME`. The mutation is
process-global, so the only safe way to run them is one-at-a-time.

## Landscape

### Source

| Path | Purpose |
|------|---------|
| `crates/cognicode-cli/src/cmd/ide.rs:688-1055` | 20 unit tests (8 mutate HOME) |
| `crates/cognicode-cli/src/cmd/lifecycle.rs:142-1056` | additional 8 tests that mutate HOME via `setup_temp_home`/`setup_clean_home` helpers |
| `crates/cognicode-cli/Cargo.toml` | dev-dependencies (currently `tempfile = "3"`) |
| `crates/cognicode-ladybug/Cargo.toml` | reference for `serial_test = "3"` dev-dep usage |

### Existing infrastructure

- `serial_test = "3"` is already a dev-dep of `cognicode-ladybug` and
  used via `#[serial]` attribute on 3 of its tests.
- No production code change is required — this is a pure test-infrastructure
  change (dev-dep + test attributes).

### Discovery during execution

While implementing this cycle, it became apparent that
`crates/cognicode-cli/src/cmd/lifecycle.rs::tests` ALSO mutates `HOME`
via `setup_temp_home` / `setup_clean_home` helpers (which call
`unsafe { std::env::set_var("HOME", ...) }`). These tests were
previously failing intermittently under parallel execution for the
same thread-safety reason as the `ide::tests` set. This cycle
extends the annotation to **both** test modules (8 tests in `ide.rs`
+ 8 tests in `lifecycle.rs` = 16 tests total), giving a net
**−8 failure reduction** under default parallelism.

## Strategy

1. Add `serial_test = "3"` to `crates/cognicode-cli/Cargo.toml`'s
   `[dev-dependencies]` block (next to `tempfile`).
2. Add `use serial_test::serial;` to the `mod tests` block in `ide.rs`.
3. Annotate each of the 8 HOME-mutating tests with `#[serial]` so they
   are forced to run sequentially, one at a time.
4. The 12 HOME-independent tests remain `#[test]` (parallel-safe).
5. Verify that `cargo test -p cognicode-cli ide::tests` passes under
   default (parallel) execution.

### Alternative considered (rejected)

**Alternative**: refactor the production code to take a `home: &Path`
parameter instead of reading `std::env::var("HOME")` internally. This
would be cleaner but requires production code change — out of scope for
this housekeeping cycle (which is test-only).

## Why a bounded test-only slice

- No production code change → minimal regression surface.
- Establishes parallel-safe execution for the entire `ide::tests` module.
- Removes a CI hazard (flaky tests under `--test-threads > 1`).
- The new `serial_test` dev-dep is shared infrastructure that future
  cycles can reuse if other tests need env-var isolation.

## Scope non-goals

- Refactoring production code to accept `home: &Path`. Future cycle.
- Annotating the 8 install/lifecycle unit tests that fail because of
  the `version mismatch` error (CARGO_PKG_VERSION 0.94.15 vs bundle
  0.94.14). Those failures are unrelated to thread safety and require
  either a bundle rebuild or a test fixture to mock the version check.
