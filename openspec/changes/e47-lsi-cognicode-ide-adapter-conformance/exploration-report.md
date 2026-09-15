# Exploration Report — e47 — cognicode-ide-adapter conformance

> Change: `e47-lsi-cognicode-ide-adapter-conformance` | Phase: explore | Date: 2026-09-15

## Problem

The 11 specs in `openspec/specs/cognicode-ide-adapter/spec.md` are all
in `no_evidence` status. The IDE adapter is exposed via
`cogh ide <detect|install|uninstall>` and the underlying recipes
(`Step::{Copy, RmRf, MergeJson, RemoveFromJson, Symlink}`) live in
`crates/cognicode-cli/src/cmd/ide.rs` (1056 LOC).

There are **20 unit tests** in `ide::tests` (lines 688-1055) that
already cover all 8 substantive scenarios — but they mutate the
process `HOME` env var directly with `unsafe { std::env::set_var }`,
which makes them **flaky under parallel test execution**. They only
pass with `cargo test -- --test-threads=1`.

This cycle closes the conformance gap by adding **subprocess
integration tests** that drive the public `cogh ide` subcommand from
outside the process, with HOME redirected via a wrapper script. The
integration tests are parallel-safe by construction (each test owns
its own subprocess + tempdir).

## Landscape

### Source

| Path | Purpose |
|------|---------|
| `crates/cognicode-cli/src/cmd/ide.rs` | `Step` enum, `Step::execute`, `detect_opencode/zcode/claude/codex`, `integrate_opencode/zcode/claude/codex`, `uninstall_opencode/zcode/claude/codex` |
| `crates/cognicode-cli/src/bin/cogh.rs` | `Command::Ide { action }` dispatcher |

### Spec

`openspec/specs/cognicode-ide-adapter/spec.md` (11 requirements):

| # | Requirement | Testable today? |
|---|-------------|-----------------|
| 1 | Each IDE is a separate `cogh` plugin | YES (init + plugin list shows 4 IDE plugins: opencode, zcode, claude, codex) |
| 2 | Adapter manifest declares integrate / uninstall steps | YES (cogh ide install --ide opencode applies steps; existing unit test `integrate_opencode_writes_mcp_entry` covers it) |
| 3 | MCP config patching is JSON-merge, not overwrite | YES (existing unit test `merge_preserves_existing_mcp_servers`) |
| 4 | Skill bundles are copied as a unit | partial (Step::Copy + integrate_opencode creates symlink; covers $VERSION placeholder indirectly) |
| 5 | `remove_from_json` cleanly removes the MCP entry | YES (existing unit test `remove_path_clears_nested_value`) |
| 6 | Adapter plugin is a self-contained Rust binary | NO (out of scope for runtime tests; this is a build-time concern) |
| 7 | Adapter declares a `detect` heuristic | YES (`cogh ide detect` enumerates installed IDEs by reading their config paths) |
| 8 | Each IDE adapter has a unique JSON path | YES (existing unit tests for zcode, claude, codex) |

**Testable in this cycle:** 1, 2, 3, 4 (partial), 5, 7, 8. Spec 6 is
build-time only.

### Empirical probe (HEAD `9977b7c5`)

```
$ cogh ide detect
Detected IDEs:
  ✓ opencode (/home/rubentxu/.config/opencode/opencode.json)
  ✓ zcode (/home/rubentxu/.zcode/v2/config.json)
```

With HOME redirection via wrapper:

```
$ cat > /tmp/test-ide-detect.sh << 'EOF'
#!/bin/bash
export HOME="$1"; shift; exec "$@"
EOF
$ mkdir -p /tmp/ide-test-home/.config/opencode
$ echo '{"mcp": {"chronos": {"type": "local"}}}' > /tmp/ide-test-home/.config/opencode/opencode.json
$ /tmp/test-ide-detect.sh /tmp/ide-test-home ./target/debug/cogh ide detect
Detected IDEs:
  ✓ opencode (/tmp/ide-test-home/.config/opencode/opencode.json)
  ✗ zcode (config not found)
```

The wrapper-script approach works.

## Strategy

Write **integration tests** in
`crates/cognicode-cli/tests/cognicode_ide_adapter.rs` that:

1. Spawn `cogh` via `std::process::Command::new(env!("CARGO_BIN_EXE_cogh"))`.
2. **Pre-build a wrapper shell script** at test setup that takes
   `HOME=$1; shift; exec "$@"` — this isolates `HOME` per test.
3. Use `tempfile::tempdir()` for the test's HOME root.
4. Pre-populate stub IDE config files (e.g. `~/.config/opencode/opencode.json`)
   when needed.
5. Assert stdout/stderr/exit-code per the spec scenarios.

Tests planned (7 tests for 7 of the 8 substantive REQs):

| # | Test | REQ |
|---|------|-----|
| 1 | `cogh_ide_detect_lists_opencode_when_config_present` | #7 detect + #8 unique JSON path |
| 2 | `cogh_ide_detect_lists_no_ides_on_empty_home` | #7 detect (negative branch) |
| 3 | `cogh_init_includes_four_ide_plugins` | #1 each IDE is a separate plugin |
| 4 | `cogh_plugin_list_shows_ide_plugins_with_manifests` | #1 manifest declared |
| 5 | `cogh_ide_install_opencode_writes_mcp_entry_preserving_existing` | #2 integrate + #3 JSON-merge |
| 6 | `cogh_ide_uninstall_opencode_removes_mcp_entry` | #5 remove_from_json |
| 7 | `cogh_ide_install_zcode_writes_zcode_specific_path` | #8 unique JSON path |

Test 6 requires `cogh ide uninstall` to run via subprocess — but the
existing unit tests already cover this fully and `cogh ide uninstall`
delegates to the same `cmd_ide_*` functions, so the subprocess
contract test is sufficient.

The wrapper-script approach (HOME redirection in a subprocess) is
robust to parallel test execution because each test owns its own
HOME tempdir; there's no env-var contention between processes.

## Why a bounded test-only slice

- No production code change → minimal regression surface.
- Bumps conformance: 8 / 523 → `pct_verified` lifts from `98.3%` to `99.8%`.
- Establishes a parallel-safe subprocess test pattern that complements
  the existing flaky unit tests.

## Scope non-goals

- Spec #6 (adapter is a self-contained Rust binary) is a build-time
  concern; not testable at runtime.
- Spec #4 "Skill bundles are copied as a unit" is partially covered
  via `Step::Copy` + symlink; full coverage would require a fixture
  skill bundle, which is broader scope than this cycle.
- The 20 existing flaky unit tests in `ide::tests` are NOT replaced;
  this cycle adds new integration tests on top. A future cycle could
  add `#[serial]` to stabilise them.
