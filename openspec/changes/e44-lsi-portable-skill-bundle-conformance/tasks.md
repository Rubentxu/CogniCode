# Tasks — e44 — portable-skill-bundle conformance

> Change: `e44-lsi-portable-skill-bundle-conformance` | Phase: tasks | Date: 2026-09-15

## Work units

### WU-1 — Create `crates/cognicode-cli/tests/portable_skill_bundle.rs`

RED-first test file covering the four REQ scenarios documented in `spec.md`:

| # | Test | REQ scenario |
|---|------|--------------|
| 1 | `portable_skill_bundle_valid_directory_passes_validation` | Skill bundle loads |
| 2 | `portable_skill_bundle_non_directory_path_is_rejected` | Non-directory path is rejected |
| 3 | `portable_skill_bundle_manifest_yaml_required_and_parsed` | `manifest.yaml` declares bundle metadata |
| 4 | `portable_skill_bundle_missing_manifest_is_rejected` | Missing `manifest.yaml` is rejected |
| 5 | `portable_skill_bundle_missing_skill_md_is_rejected` | Missing `SKILL.md` is rejected |
| 6 | `portable_skill_bundle_invalid_maturity_is_rejected` | Invalid `maturity` is rejected |
| 7 | `portable_skill_bundle_ide_specific_compatibility_field_rejected` | Portable bundle is IDE-agnostic |
| 8 | `portable_skill_bundle_referenced_script_must_exist` | Reference files are copied recursively (pre-condition) |

Implementation notes:

- Use `tempfile::tempdir()` to build isolated bundle fixtures.
- Helper `write_bundle(dir, manifest_yaml, skill_md)` that writes both files atomically.
- Helper `write_script(dir, path)` that creates a referenced script on disk so the "valid" path can declare it.
- Spawn `cogh` via `Command::new(env!("CARGO_BIN_EXE_cogh"))` (matches e43 pattern).
- Pass `--home <isolated tempdir>` so the home never touches `~/.cognicode`.

### WU-2 — Verify locally

```
cargo test -p cognicode-cli --test portable_skill_bundle
```

Expected: 8 passed; 0 failed.

### WU-3 — Update conformance evidence_map

Add an entry for `portable-skill-bundle`:

```yaml
portable-skill-bundle:
  status: verified
  evidence_path: crates/cognicode-cli/tests/portable_skill_bundle.rs
  evidence_note: 'e44 portable-skill-bundle conformance: 8 integration tests covering REQ #1 (bundle loads + non-directory rejection), #2 (manifest required + maturity validated), #3 (no IDE-specific compatibility), #4 (manifest parsed), #7 (referenced scripts must exist); REQ #5 (versioning with MCP server) and #6 (requires cascading) are network-dependent and out of scope.'
```

Re-run `python3 sandbox/scripts/openspec_conformance.py` and confirm `pct_verified` lifts from `93.7%` to ≥ `94.7%`.

## Sequencing

Apply WU-1 (file write + run), verify WU-2, then apply WU-3 (doc-only).

## Acceptance gate

- `cargo test -p cognicode-cli --test portable_skill_bundle` passes 8/8.
- `openspec_conformance.py` reports `pct_verified ≥ 94.7%`.
- No production code change (test-only cycle).
