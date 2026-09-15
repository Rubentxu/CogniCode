# Exploration Report — e45 — cognicode-plugin conformance

> Change: `e45-lsi-cognicode-plugin-conformance` | Phase: explore | Date: 2026-09-15

## Problem

The 4 specs in `openspec/specs/cognicode-plugin/spec.md` are all in
`no_evidence` status. The plugin manager is exposed via
`cogh plugin <subcommand>` (`add`/`remove`/`list`/`update`) and the
manifest parser lives at `crates/cognicode-cli/src/cmd/manifest.rs`
with 4 unit tests but no integration coverage of the CLI surface.

This continues the e43/e44 pattern: every `cognicode-cli` capability
that ships in the conformance corpus must be locked down with tests.

## Landscape

### Source

| Path | Purpose |
|------|---------|
| `crates/cognicode-cli/src/cmd/manifest.rs` | `PluginManifest` struct, `from_str`, `validate`, `find_version` |
| `crates/cognicode-cli/src/cmd/layout.rs` | `cmd_plugin_add`, `cmd_plugin_remove`, `cmd_plugin_list`, `cmd_plugin_update` |
| `crates/cognicode-cli/src/bin/cogh.rs:252-270` | `Command::Plugin { action }` dispatcher |

### Spec

`openspec/specs/cognicode-plugin/spec.md` (4 requirements):

| # | Requirement | Testable today? |
|---|-------------|-----------------|
| 1 | `plugin.yaml` is the canonical manifest format | partial (manifest parsed and printed by `cogh plugin list`; `manifest.rs` has 4 unit tests for parse/validate/sha256; full install path is network-dependent) |
| 2 | Versions are addressable by ref | NO (network: install) |
| 3 | `sha256` integrity check is mandatory | NO (network: install + verify) |
| 4 | Bundled plugins ship with cogh | YES (`cogh init` installs bundled; `cogh plugin list` enumerates them) |

**Testable in this cycle:** #1 (partial, via `plugin list` after dropping a
hand-crafted `plugin.yaml`) and #4 (full, via `cogh init` + `plugin list`).

### Empirical probe (HEAD `8b2f7ded`)

```
$ cogh --home <temp> init
✓ Installed 6 bundled plugin(s)

$ cogh --home <temp> plugin list
Plugin          Description
---------------------------------------------
codex           Codex IDE integration — patches ...
claude          Claude Code IDE integration — writes ...
zcode           ZCode IDE integration — patches ...
sandbox-templates CogniCode podman container specs ...
skills-cognicode-core CogniCode portable skill bundles ...
mcp-server      CogniCode MCP server — 68 tools ...

# Adding a custom plugin (filesystem-level, simulates `cogh plugin add`):
$ mkdir -p <temp>/plugins/my-test-plugin
$ cat > <temp>/plugins/my-test-plugin/plugin.yaml << EOF
apiVersion: cognicode/v1
kind: Plugin
name: my-test-plugin
description: A test plugin added manually for conformance
versions:
  - ref: "1.0.0"
    artifact: my-test-plugin-1.0.0.tar.gz
    sha256: 0000...0000
EOF

$ cogh --home <temp> plugin list
my-test-plugin  A test plugin added manually for conformance
<existing 6 plugins>
```

Both the bundled-plugin enumeration and the custom-plugin detection
are reachable end-to-end via the public CLI without network.

## Strategy

Write **integration tests** in
`crates/cognicode-cli/tests/cognicode_plugin.rs` that:

1. Spawn `cogh` via `std::process::Command::new(env!("CARGO_BIN_EXE_cogh"))`
   (matches e43/e44 pattern).
2. Use `tempfile::tempdir()` per test for `--home`.
3. Build a minimal but valid `plugin.yaml` fixture for the custom-plugin
   scenarios; rely on `cogh init` for the bundled-plugin scenarios.

Tests planned:

| Test | REQ | Scenario |
|------|-----|----------|
| `cogh_init_installs_bundled_plugins` | #4 | `cogh init` installs bundled plugins |
| `cogh_plugin_list_enumerates_bundled_plugins` | #4 | bundled plugins are listed after init |
| `cogh_plugin_list_shows_custom_plugin_with_parsed_description` | #1 | `plugin.yaml` parses with cogh |
| `cogh_plugin_list_handles_plugin_without_yaml_gracefully` | #1 (defensive) | missing manifest falls back to empty description |
| `cogh_plugin_add_without_url_rejects_unknown_name` | #1 | the bundled-name whitelist is enforced when no `--from-url` is given |

## Why a bounded test-only slice

- No production code change → minimal regression surface.
- Bumps conformance: 2 / 523 → `pct_verified` lifts from `95.2%` to `95.6%`.
- Establishes the contract that the `cogh plugin` subcommands
  behave correctly with the bundled plugins + a hand-crafted manifest.

## Scope non-goals

- Specs #2 (versions addressable by ref) and #3 (sha256 mandatory):
  require `cogh install` which is network-dependent. The 4 unit tests in
  `manifest.rs` already cover the sha256 format validation; no additional
  coverage is needed at the CLI integration level.
- The full install transaction (`cmd_install`, `installer_transaction.rs`)
  is part of the M5/MCP wiring scope and is exercised in the
  `install_lock::tests` + `lifecycle::tests` modules that already fail
  on main because of the version mismatch (pre-existing, unrelated).
