# Delta for e32i_opencode_self_apply

## MODIFIED Requirements

### Requirement: `cogh install` registers MCP server with IDEs

When `cogh install --ide <name>` is run for a registered IDE, `cogh` MUST dispatch to the IDE adapter for the `--ide` value. The dispatched adapter MUST:
1. Locate the IDE's config file (e.g. `~/.config/opencode/opencode.json`)
2. Patch the `mcp` section to add the CogniCode server entry
3. Symlink or copy skill bundles to the IDE's skill directory
4. Preserve any other MCP servers the user has configured
5. Write `mcp.cognicode-mcp.command[0]` as the absolute resolved shim path (`~/.cognicode/shims/cognicode-mcp`), not the bare binary name

When `--profile` is also set, the atomic bundle install MUST run first, then the IDE adapter MUST dispatch for any `--ide` value.

(Previously: `cogh install --ide <name>` silently dropped `--ide` when `--profile` was set; the working entry point was `cogh ide install <name>`.)

#### Scenario: `cogh install --ide opencode` dispatches and patches opencode.json

- GIVEN `~/.config/opencode/opencode.json` exists with an existing `mcp` section
- WHEN `cogh install --ide opencode` runs
- THEN `cogh` dispatches to the opencode adapter
- AND the existing `mcp` section is preserved
- AND `mcp.cognicode-mcp.command[0]` equals the absolute shim path (`~/.cognicode/shims/cognicode-mcp`)
- AND the JSON file is still valid

#### Scenario: `cogh install --ide zcode` patches zcode config

- GIVEN `~/.zcode/v2/config.json` exists
- WHEN `cogh install --ide zcode` runs
- THEN the MCP section is patched (per ZCode's specific config shape)

#### Scenario: `cogh install --ide claude` creates per-server JSON file

- GIVEN `~/.claude/mcp/` directory is accessible
- WHEN `cogh install --ide claude` runs
- THEN `~/.claude/mcp/cognicode-mcp.json` is created with the MCP entry
- AND skills are copied to `~/.claude/skills/cognicode-$VERSION/`

#### Scenario: `cogh install --ide codex` patches TOML config

- GIVEN `~/.codex/config.toml` exists
- WHEN `cogh install --ide codex` runs
- THEN the `[mcp_servers.cognicode-mcp]` table is added with command and args
- AND skills are copied to `~/.codex/skills/cognicode-$VERSION/`

#### Scenario: `cogh install --ide all` configures every registered IDE

- GIVEN OpenCode + ZCode + Claude Code are installed
- WHEN `cogh install --ide all` runs
- THEN all three IDEs are configured with the same MCP server + skills
- AND the user can invoke CogniCode from any IDE

### Requirement: Adapter manifest declares integrate / uninstall steps

Each adapter plugin manifest declares an `integrate` step list (applied on `cogh install --ide <name>`) and an `uninstall` step list (applied on `cogh uninstall --ide <name>`). The OpenCode adapter's `integrate` MUST resolve the MCP command to the absolute shim path `~/.cognicode/shims/cognicode-mcp` and write that value as `opencode.json` → `mcp.cognicode-mcp.command[0]` (alongside `"type": "stdio"` as a sibling field, per OpenCode's MCP schema). The adapter MUST NOT hardcode the bare binary name `cognicode-mcp`, because the MCP process spawner does not inherit `~/.cognicode/shims/` on `PATH`. This contract mirrors the `integrate_zcode`, `integrate_claude`, and `integrate_codex` adapters.

(Previously: opencode `integrate` hardcoded the bare binary name in `command`, so the MCP server only started when `~/.cognicode/shims/` happened to be on `PATH`.)

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
