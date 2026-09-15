# Tasks — e47 — cognicode-ide-adapter conformance

> Change: `e47-lsi-cognicode-ide-adapter-conformance` | Phase: tasks | Date: 2026-09-15

## Work units

### WU-1 — Create `crates/cognicode-cli/tests/cognicode_ide_adapter.rs`

RED-first test file covering the 7 scenarios documented in `spec.md`.

The crucial technique is the **wrapper-script HOME redirect**:
each test writes a tiny shell wrapper at setup time
(`export HOME="$1"; shift; exec "$@"`) and invokes it via
`Command::new(wrapper).arg(home).arg(cogh).args(...)`. This isolates
`$HOME` to the subprocess tree so the IDE adapter's `~/.config/...`
lookups don't touch the developer's real `$HOME`.

Tests planned:

| # | Test | REQ scenario |
|---|------|--------------|
| 1 | `cogh_ide_detect_lists_opencode_when_config_present` | detect checks for OpenCode config |
| 2 | `cogh_ide_detect_lists_no_ides_on_empty_home` | detect returns empty when no IDE is installed |
| 3 | `cogh_init_includes_four_ide_plugins` | 4 IDE adapters ship in v1 |
| 4 | `cogh_plugin_list_shows_ide_plugins_with_manifests` | IDE adapters are listed after init |
| 5 | `cogh_ide_install_opencode_writes_mcp_entry_preserving_existing` | merge_json preserves existing MCP servers |
| 6 | `cogh_ide_uninstall_opencode_removes_mcp_entry` | remove_from_json preserves other MCP entries |
| 7 | `cogh_ide_install_zcode_writes_zcode_specific_path` | ZCode adapter patches mcp section |

Implementation notes:

- Helper `wrapper_script(home, args)` builds the wrapper invocation.
- For tests 1, 5, 6, 7: pre-populate `<home>/.config/...` with stub
  JSON before invoking `cogh ide ...`.
- For tests 5 and 6: also run `cogh init` against `--home` first.
- For tests 1, 5, 6: assert on the opencode config file under the
  tempdir (read it back via std::fs).

### WU-2 — Verify locally

```
cargo test -p cognicode-cli --test cognicode_ide_adapter
```

Expected: 7 passed; 0 failed.

### WU-3 — Update conformance evidence_map

Add an entry for `cognicode-ide-adapter`:

```yaml
cognicode-ide-adapter:
  status: verified
  evidence_path: crates/cognicode-cli/tests/cognicode_ide_adapter.rs + crates/cognicode-cli/src/cmd/ide.rs tests
  evidence_note: 'e47 cognicode-ide-adapter conformance: 7 subprocess integration tests covering REQ #1 (4 IDE plugins ship via cogh init), REQ #2 (cogh ide install applies integrate steps), REQ #3 (JSON-merge preserves existing mcp entries), REQ #5 (cogh ide uninstall removes the cognicode-mcp entry while preserving others), REQ #7 (cogh ide detect enumerates installed IDEs), REQ #8 (each IDE has its own unique config path). The 20 unit tests in ide.rs cover the same scenarios at the module level; this cycle adds parallel-safe subprocess coverage via HOME redirection through a wrapper shell script. REQ #6 (self-contained binary) is a build-time concern and out of scope.'
```

Re-run `python3 sandbox/scripts/openspec_conformance.py` and confirm `pct_verified` lifts from `98.3%` to `≥ 99.8%`.

## Sequencing

Apply WU-1, verify WU-2, then apply WU-3 (doc-only).

## Acceptance gate

- `cargo test -p cognicode-cli --test cognicode_ide_adapter` passes 7/7.
- `openspec_conformance.py` reports `pct_verified ≥ 99.8%`.
- No production code change (test-only cycle).
