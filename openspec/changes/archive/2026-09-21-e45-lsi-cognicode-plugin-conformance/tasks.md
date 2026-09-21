# Tasks — e45 — cognicode-plugin conformance

> Change: `e45-lsi-cognicode-plugin-conformance` | Phase: tasks | Date: 2026-09-15

## Work units

### WU-1 — Create `crates/cognicode-cli/tests/cognicode_plugin.rs`

RED-first test file covering the 5 scenarios documented in `spec.md`:

| # | Test | REQ scenario |
|---|------|--------------|
| 1 | `cogh_init_installs_bundled_plugins` | `cogh init` installs bundled plugins |
| 2 | `cogh_plugin_list_enumerates_bundled_plugins_after_init` | bundled plugins are listed after init |
| 3 | `cogh_plugin_list_shows_custom_plugin_with_parsed_description` | `plugin.yaml` parses with cogh |
| 4 | `cogh_plugin_list_handles_plugin_without_yaml_gracefully` | Missing plugin manifest falls back gracefully |
| 5 | `cogh_plugin_add_unknown_name_without_url_is_rejected` | `cogh plugin add` rejects unknown non-bundled names |

Implementation notes:

- Reuse the e43/e44 subprocess pattern: `Command::new(env!("CARGO_BIN_EXE_cogh"))` + `--home <temp>`.
- Helper `run_cogh(home, args)` to centralise dispatch.
- Helper `write_plugin(home, name, yaml)` to drop a plugin.yaml into `<home>/plugins/<name>/`.
- Constant `SAMPLE_PLUGIN_YAML` with the minimal valid manifest shape.
- For test 4, create `<home>/plugins/empty-dir/` with no files.

### WU-2 — Verify locally

```
cargo test -p cognicode-cli --test cognicode_plugin
```

Expected: 5 passed; 0 failed.

### WU-3 — Update conformance evidence_map

Add an entry for `cognicode-plugin`:

```yaml
cognicode-plugin:
  status: verified
  evidence_path: crates/cognicode-cli/tests/cognicode_plugin.rs + crates/cognicode-cli/src/cmd/manifest.rs tests
  evidence_note: 'e45 cognicode-plugin conformance: 5 CLI integration tests locking down REQ #4 (cogh init installs bundled plugins, cogh plugin list enumerates them) and REQ #1 (plugin.yaml parses end-to-end, missing manifest falls back gracefully, plugin add rejects unknown non-bundled names); the 4 unit tests in manifest.rs already cover REQ #2 (version ref parsing) and REQ #3 (sha256 format validation); the full install path is network-dependent and out of scope.'
```

Re-run `python3 sandbox/scripts/openspec_conformance.py` and confirm `pct_verified` lifts from `95.2%` to `≥ 95.6%`.

## Sequencing

Apply WU-1, verify WU-2, then apply WU-3 (doc-only).

## Acceptance gate

- `cargo test -p cognicode-cli --test cognicode_plugin` passes 5/5.
- `openspec_conformance.py` reports `pct_verified ≥ 95.6%`.
- No production code change (test-only cycle).
