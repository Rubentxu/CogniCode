# CogniCode IDE Adapter

## Purpose

Per-IDE adapter plugins that translate portable skill bundles
(`portable-skill-bundle` spec) into the IDE-specific configuration
format. Each supported IDE (OpenCode, ZCode, Claude Code, Codex,
future) has its own adapter plugin that knows how to:

1. Patch the IDE's MCP config file with the CogniCode server entry
2. Copy skill bundles into the IDE's skill directory
3. Optionally install IDE commands (slash commands, etc.)
4. Reverse-clean-up on uninstall

See ADR-036 for the design rationale.

## Requirements

### Requirement: Each IDE is a separate `cogh` plugin

Each IDE adapter is shipped as a separate `cogh` plugin:
- `opencode` — OpenCode (already have skills there)
- `zcode` — ZCode (have config.json mcp section)
- `claude` — Claude Code (have `claude_desktop_config.json`)
- `codex` — Codex (have `~/.codex/config.json`)

Adapters are discoverable via the standard plugin registry (default =
GitHub Releases). Users install with `cogh plugin add <ide>`.

#### Scenario: 4 IDE adapters ship in v1

- GIVEN `cogh init` runs
- AND the bundled plugins are registered
- THEN `~/.cognicode/plugins/opencode/plugin.yaml` exists
- AND `~/.cognicode/plugins/zcode/plugin.yaml` exists
- AND `~/.cognicode/plugins/claude/plugin.yaml` exists
- AND `~/.cognicode/plugins/codex/plugin.yaml` exists

### Requirement: Adapter manifest declares integrate / uninstall steps

```yaml
apiVersion: cognicode/v1
kind: IdeAdapter
name: opencode
description: "OpenCode IDE integration"

detect:                          # how to detect if the IDE is installed
  - file: ~/.config/opencode/opencode.json
  - shell: which opencode

integrate:                       # how to install (apply on `cogh install --ide opencode`)
  - id: skills
    kind: copy
    source: skills/              # relative to plugin dir
    target: ~/.config/opencode/skills/cognicode-$VERSION/
  - id: mcp
    kind: merge_json
    target: ~/.config/opencode/opencode.json
    merge_path: mcp.cognicode-mcp
    template: |
      {
        "command": ["~/.cognicode/shims/cognicode-mcp"],
        "enabled": true,
        "type": "stdio"
      }
  - id: commands
    kind: copy_glob
    source: commands/cognicode-*.md
    target: ~/.config/opencode/commands/

uninstall:                       # how to clean up (apply on `cogh uninstall --ide opencode`)
  - id: skills
    kind: rm_rf
    target: ~/.config/opencode/skills/cognicode-$VERSION/
  - id: mcp
    kind: remove_from_json
    target: ~/.config/opencode/opencode.json
    merge_path: mcp.cognicode-mcp
  - id: commands
    kind: rm_glob
    target: ~/.config/opencode/commands/cognicode-*.md
```

#### Scenario: `cogh install --ide opencode` applies integrate steps

- GIVEN the `opencode` adapter plugin is installed
- AND `~/.config/opencode/opencode.json` exists
- WHEN `cogh install --ide opencode` runs
- THEN the `skills` step copies the skill bundles
- AND the `mcp` step merges the MCP entry into `opencode.json`
- AND `mcp.cognicode-mcp.command[0]` equals the absolute shim path
- AND the `commands` step copies the IDE commands

#### Scenario: opencode integrate writes absolute shim path on a fresh entry

- GIVEN `~/.cognicode/shims/cognicode-mcp` exists
- AND `~/.cognicode/shims/` is NOT on `PATH`
- AND `~/.config/opencode/opencode.json` has no `mcp.cognicode-mcp`
- WHEN the opencode integrate step runs
- THEN `mcp.cognicode-mcp.command[0]` equals the absolute shim path
- AND `mcp.cognicode-mcp.type` equals `"stdio"`
- AND no other `mcp.*` entries are touched

#### Scenario: opencode integrate overwrites stale entry with resolved shim

- GIVEN `opencode.json` has `mcp.cognicode-mcp.command = ["/bin/cognicode-mcp"]`
- WHEN the opencode integrate step runs
- THEN `mcp.cognicode-mcp.command` is replaced with the resolved shim entry
- AND no other `mcp.*` entries are touched

### Requirement: MCP config patching is JSON-merge, not overwrite

The `merge_json` integrate step MUST:
1. Read the existing target file
2. Parse it as JSON
3. Apply the template at the specified `merge_path` (JSON path)
4. If the path already exists, REPLACE that path
5. Preserve all other JSON keys
6. Write back atomically (tmp file + rename)

The merge path is dot-notation: `mcp.cognicode-mcp` means the JSON
key `mcp.cognicode-mcp` (treating dots as object keys, not as
nested paths). This is the convention OpenCode uses.

#### Scenario: `merge_json` preserves existing MCP servers

- GIVEN `opencode.json` has `mcp.chronos` and `mcp.bastion`
- AND `cogh install --ide opencode` runs
- THEN after the install:
  - `mcp.chronos` is preserved
  - `mcp.bastion` is preserved
  - `mcp.cognicode-mcp` is added
  - The JSON file is still valid

#### Scenario: `merge_json` updates an existing entry

- GIVEN `opencode.json` has `mcp.cognicode-mcp` pointing to an old path
- AND `cogh install --ide opencode` runs
- THEN `mcp.cognicode-mcp` is updated to the new path
- AND other MCP entries are preserved

### Requirement: Skill bundles are copied as a unit

The `copy` step for skills MUST copy the entire portable skill
bundle directory tree to the IDE's skill location. The target
directory MAY include the cogh version (`$VERSION` placeholder is
resolved).

#### Scenario: `copy` resolves `$VERSION` placeholder

- GIVEN the current tracker version is `0.92.0`
- WHEN the integrate step copies skills
- THEN the target directory is `~/.config/opencode/skills/cognicode-0.92.0/`

### Requirement: `remove_from_json` cleanly removes the MCP entry

The `remove_from_json` uninstall step MUST:
1. Read the existing target file
2. Remove the key at `merge_path`
3. Write back atomically

#### Scenario: `remove_from_json` preserves other MCP entries

- GIVEN `opencode.json` has `mcp.cognicode-mcp` and `mcp.chronos`
- AND `cogh uninstall --ide opencode` runs
- THEN `mcp.cognicode-mcp` is removed
- AND `mcp.chronos` is preserved
- AND the JSON file is still valid

### Requirement: Adapter plugin is a self-contained Rust binary

The adapter's `integrate` and `uninstall` steps are implemented in
the cogh CLI itself (not as a separate plugin binary). The plugin
manifest declares the steps; cogh runs them.

This keeps the plugin author surface small: just a YAML manifest
and a directory of assets.

### Requirement: Adapter declares a `detect` heuristic

The adapter's `detect` block tells cogh whether the IDE is present
on the system. `cogh install --ide <name>` checks `detect` first
and refuses to install if all detectors fail.

#### Scenario: `detect` checks for OpenCode config

- GIVEN `~/.config/opencode/opencode.json` does NOT exist
- AND `opencode` is NOT on PATH
- WHEN `cogh install --ide opencode` runs
- THEN output is "OpenCode not detected; install OpenCode first"
- AND no install happens

#### Scenario: `detect` allows the install when IDE is found

- GIVEN `~/.config/opencode/opencode.json` exists
- WHEN `cogh install --ide opencode` runs
- THEN the install proceeds

### Requirement: Each IDE adapter has a unique JSON path

Each IDE has its own JSON config shape. The adapter specifies the
`merge_path` for the MCP entry:

| IDE | Config file | Merge path |
|---|---|---|
| OpenCode | `~/.config/opencode/opencode.json` | `mcp.cognicode-mcp` |
| ZCode | `~/.zcode/v2/config.json` | `mcp.cognicode-mcp` |
| Claude Code | `~/.claude/mcp/cognicode-mcp.json` | (one JSON file per server) |
| Codex | `~/.codex/config.toml` | `[mcp_servers.cognicode-mcp]` (TOML table) |

#### Scenario: ZCode adapter patches `mcp` section

- GIVEN `~/.zcode/v2/config.json` has `mcp.<other-name>` entries
- AND `cogh install --ide zcode` runs
- THEN `mcp.cognicode-mcp` is added with the stdio command
- AND other MCP entries are preserved

#### Scenario: Claude Code adapter writes per-server JSON file

- GIVEN `~/.claude/mcp/` directory exists
- AND `cogh install --ide claude` runs
- THEN `~/.claude/mcp/cognicode-mcp.json` is created with the MCP entry
- AND other MCP server files in `~/.claude/mcp/` are preserved

#### Scenario: Codex adapter patches TOML `mcp_servers` table

- GIVEN `~/.codex/config.toml` has `[[mcp_servers]]` entries
- AND `cogh install --ide codex` runs
- THEN `[mcp_servers.cognicode-mcp]` table is added with command and args
- AND other `[[mcp_servers]]` entries are preserved

## Cross-references

- ADR-036 — `IDE-abstraction-portable-skills-per-ide-adapters`
- `docs/specs/cognicode-cli/spec.md`
- `docs/specs/portable-skill-bundle/spec.md`

## Implementation Log

- **2026-08-10 (E32-C plan)**: Spec drafted. Schema for `IdeAdapter`
  mirrors the `mcp-server` plugin manifest but with `integrate` /
  `uninstall` step lists.
