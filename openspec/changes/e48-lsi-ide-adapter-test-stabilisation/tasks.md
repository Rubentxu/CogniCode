# Tasks — e48 — IDE adapter test stabilisation

> Change: `e48-lsi-ide-adapter-test-stabilisation` | Phase: tasks | Date: 2026-09-15

## Work units

### WU-1 — Add `serial_test` dev-dep

In `crates/cognicode-cli/Cargo.toml`, add `serial_test = "3"` to the
`[dev-dependencies]` block (next to `tempfile = "3"`).

### WU-2 — Annotate HOME-mutating tests

Annotate each test that mutates the process `HOME` env var with
`#[serial]` so they are forced to run sequentially, one at a time.

In `crates/cognicode-cli/src/cmd/ide.rs`:

1. Add `use serial_test::serial;` near the top of the `mod tests`
   block (next to the existing `use` statements).
2. Add `#[serial]` directly above each of the 8 test functions listed
   in the spec.

In `crates/cognicode-cli/src/cmd/lifecycle.rs`:

1. Add `use serial_test::serial;` near the top of the `mod tests` block.
2. Add `#[serial]` above each of the 8 lifecycle test functions that
   call `setup_temp_home` / `setup_clean_home` (which mutate `HOME`
   via `unsafe { std::env::set_var("HOME", ...) }`):
   - `init_creates_layout_and_bundled_plugins`
   - `install_opencode_ide_patches_config_and_skills`
   - `install_codex_ide_patches_toml_config`
   - `install_zcode_ide_patches_config_and_skills`
   - `install_claude_ide_patches_config_and_skills`
   - `test_clean_home_install`
   - `tracker_write_and_read_version_roundtrip`
   - `test_install_with_ide_and_profile_dispatches_both`

### WU-3 — Verify parallel execution

```
cargo test -p cognicode-cli ide::tests
cargo test -p cognicode-cli
```

Expected after this cycle:

- `ide::tests`: 20 passed; 0 failed.
- Full CLI suite under default parallelism: 96+ passed; ≤ 4 failed.
  The remaining failures are pre-existing bugs (version mismatch between
  bundled plugin manifests and `CARGO_PKG_VERSION`) unrelated to thread
  safety. They were documented before this cycle began.

Before this cycle: 89 passed; 11 failed under default parallelism.
After this cycle: ~97 passed; ~3 failed under default parallelism.
That's a **−8 failure reduction** from the `#[serial]` annotations alone.

## Sequencing

Apply WU-1 → WU-2 → WU-3 in one commit (the change is conceptually
atomic: add dev-dep + annotate tests + verify).

## Acceptance gate

- `cargo test -p cognicode-cli ide::tests` reports `20 passed; 0 failed`.
- `cargo test -p cognicode-cli` under default parallelism reports
  ≤ 4 failed (down from 11 pre-cycle). The remaining failures are
  pre-existing version-mismatch bugs unrelated to this cycle.
- `crates/cognicode-cli/Cargo.toml` includes `serial_test = "3"` in dev-deps.
- All target test functions in `ide::tests` (8) and `lifecycle::tests` (8) carry `#[serial]`.
- No production code change (test infrastructure only).
