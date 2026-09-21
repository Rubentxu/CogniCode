# Spec — e47 — cognicode-ide-adapter conformance

> Change: `e47-lsi-cognicode-ide-adapter-conformance` | Phase: spec | Date: 2026-09-15

## Purpose

Lock down the user-visible contracts of `cogh ide <subcommand>` and
the 4 bundled IDE plugins (opencode, zcode, claude, codex) for the
scenarios that are exercisable via subprocess with HOME isolation, so
they reach `verified` status in the conformance corpus.

## Requirements

### Requirement: `cogh ide detect` enumerates installed IDEs

`cogh ide detect` MUST print a `Detected IDEs:` header followed by
one line per known IDE. For each IDE, the line MUST be either
`✓ <name> (<config-path>)` if the IDE config file exists under
`$HOME`, or `✗ <name> (config not found)` otherwise.

#### Scenario: `detect` checks for OpenCode config

- GIVEN `$HOME/.config/opencode/opencode.json` exists with valid JSON
- WHEN `cogh ide detect` runs
- THEN stdout contains `Detected IDEs:`
- AND stdout contains `✓ opencode`
- AND stdout references the path `~/.config/opencode/opencode.json`.

#### Scenario: `detect` returns empty when no IDE is installed

- GIVEN `$HOME` is a fresh tempdir (no IDE configs)
- WHEN `cogh ide detect` runs
- THEN stdout contains `Detected IDEs:`
- AND stdout contains `opencode` AND `✗` (config not found).

### Requirement: Each IDE is a separate `cogh` plugin

`cogh init` MUST install 4 bundled IDE plugins (`opencode`, `zcode`,
`claude`, `codex`) into `<home>/plugins/<name>/`. After init,
`cogh plugin list` MUST enumerate them.

#### Scenario: 4 IDE adapters ship in v1

- GIVEN `--home` points to a fresh tempdir
- WHEN `cogh init --home <dir>` runs
- THEN `<dir>/plugins/` contains directories named `opencode`, `zcode`,
  `claude`, and `codex`.

#### Scenario: IDE adapters are listed after init

- GIVEN `--home` is initialised
- WHEN `cogh plugin list --home <dir>` runs
- THEN stdout contains all four plugin names: `opencode`, `zcode`,
  `claude`, `codex`.

### Requirement: Adapter manifest declares integrate / uninstall steps

Each IDE adapter plugin MUST have a `plugin.yaml` that declares the
`integrate` and `uninstall` steps. The file MUST be valid YAML and
MUST be parsed by `cogh plugin list` (the description is rendered).

#### Scenario: `cogh install --ide opencode` applies integrate steps (contract)

- GIVEN the opencode adapter plugin is installed and `$HOME/.config/opencode/opencode.json`
  contains existing content
- WHEN `cogh ide install opencode --plugin mcp-server --home <dir>`
  runs (with the wrapper script setting HOME to a tempdir)
- THEN exit status is `0`
- AND the opencode config file under the tempdir contains the
  `mcp.cognicode-mcp` entry while preserving the pre-existing
  `mcp.chronos` entry.

### Requirement: MCP config patching is JSON-merge, not overwrite

When `cogh ide install` writes the MCP config, it MUST merge into the
existing JSON rather than overwriting it. Pre-existing MCP entries
under the `mcp.*` path MUST be preserved.

#### Scenario: `merge_json` preserves existing MCP servers

- GIVEN an opencode config with `mcp.chronos.type = "local"`
- WHEN `cogh ide install opencode --plugin mcp-server --home <dir>`
  runs against a tempdir containing that config
- THEN the resulting config contains both `mcp.chronos.type = "local"`
  AND `mcp.cognicode-mcp.type = "stdio"`.

### Requirement: `remove_from_json` cleanly removes the MCP entry

`cogh ide uninstall opencode --plugin mcp-server --home <dir>` MUST
remove the `mcp.cognicode-mcp` entry from the opencode config while
preserving other `mcp.*` entries.

#### Scenario: `remove_from_json` preserves other MCP entries

- GIVEN an opencode config containing both `mcp.cognicode-mcp` and
  `mcp.chronos`
- WHEN `cogh ide uninstall opencode --plugin mcp-server --home <dir>`
  runs against a tempdir containing that config
- THEN exit status is `0`
- AND the resulting config does NOT contain `mcp.cognicode-mcp`
- AND the resulting config still contains `mcp.chronos`.

### Requirement: Each IDE adapter has a unique JSON path

The four bundled IDE adapters MUST target four distinct config paths:

- `opencode` → `~/.config/opencode/opencode.json`
- `zcode` → `~/.zcode/v2/config.json`
- `claude` → `~/.claude/mcp/cognicode-mcp.json`
- `codex` → `~/.codex/config.toml`

#### Scenario: ZCode adapter patches `mcp` section

- GIVEN `$HOME/.zcode/v2/config.json` exists with valid JSON
- WHEN `cogh ide detect` runs
- THEN stdout contains `✓ zcode` AND references
  `~/.zcode/v2/config.json`.

## Out of scope (build-time or fixture-bounded)

- Spec #6 "Adapter plugin is a self-contained Rust binary" — build-time
  concern, not runtime-testable.
- Spec #4 "Skill bundles are copied as a unit" — full coverage would
  require a fixture skill bundle directory; partial coverage via
  `Step::Copy` is asserted by the existing unit tests, not in this cycle.
