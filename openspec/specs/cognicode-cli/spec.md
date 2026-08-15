# CogniCode CLI (`cogh`) — Version Management Tool

## Purpose

A single binary CLI (`cogh`) that manages the full lifecycle of
CogniCode's runtime artifacts: MCP server, sandbox containers,
skills, and IDE integration. Implements the `asdf-vm` pattern
(ADR-035) with portable skill bundles (ADR-036) and per-IDE adapter
plugins.

Inspired by `asdf-vm` (https://asdf-vm.com). See ADR-034 for the
distribution architecture and ADR-035 for the version-management pattern.

## Requirements

### Requirement: `cogh` binary is a single static executable

The `cogh` CLI MUST be installed as a single static binary at
`~/.cognicode/bin/cogh`. It MUST have no dynamic library dependencies
beyond libc + libstdc++ (no .so files in the install). It MUST
support Linux x86_64, macOS x86_64 + arm64, and Windows x86_64.
Self-contained binaries enable `curl ... | sh` install.

#### Scenario: `cogh --version` reports the binary version

- GIVEN `cogh` is installed at `~/.cognicode/bin/cogh`
- AND `~/.cognicode/bin` is on `PATH`
- WHEN the user runs `cogh --version`
- THEN output is `cogh 0.1.0`

#### Scenario: `cogh install` works without network for the binary itself

- GIVEN `cogh` is installed once
- AND `cogh install <plugin>` is run multiple times
- THEN each invocation does NOT re-download the `cogh` binary itself
- AND only the plugin artifacts are re-fetched

### Requirement: `~/.cognicode/` layout mirrors `~/.asdf/`

The `cogh install` command MUST create a directory layout:
- `~/.cognicode/bin/cogh` — the CLI itself
- `~/.cognicode/versions/<v>/<plugin>/bin/...` — versioned binaries
- `~/.cognicode/shims/<binary>` — symlinks to current version
- `~/.cognicode/plugins/<plugin>/plugin.yaml` — installed plugins
- `~/.cognicode/tracker/version` — current version pin
- `~/.cognicode/locks/<project-hash>/cognicode.lock` — per-project locks
- `~/.cognicode/cache/downloads/` — cached tarballs

#### Scenario: `cogh install mcp-server` creates the layout

- GIVEN `~/.cognicode/` does not exist
- WHEN `cogh install mcp-server --version 0.92.0` runs
- THEN `~/.cognicode/bin/cogh` exists
- AND `~/.cognicode/versions/0.92.0/mcp-server/bin/cognicode-mcp` exists
- AND `~/.cognicode/shims/cognicode-mcp` is a symlink to the versioned binary
- AND `~/.cognicode/tracker/version` contains `0.92.0`

### Requirement: `~/.cognicode/shims/` regenerates on every install

After every install or uninstall, `cogh` MUST regenerate the shims
directory so that the current version's binaries are exposed as
symlinks. The shim is a small script (or symlink) that resolves to
the version-specific binary.

#### Scenario: `cogh install <plugin>` regenerates shims

- GIVEN `cogh install mcp-server --version 0.92.0` was just run
- AND `cogh install mcp-server --version 0.91.1` runs on top
- THEN `~/.cognicode/versions/0.91.1/mcp-server/bin/` exists
- AND `~/.cognicode/shims/cognicode-mcp` now points to `0.91.1`
- AND `~/.cognicode/versions/0.92.0/...` is still present (not deleted)

#### Scenario: `cogh uninstall <plugin>` regenerates shims

- GIVEN `cogh install mcp-server --version 0.92.0` was run
- AND `cogh uninstall mcp-server --version 0.92.0` runs
- THEN `~/.cognicode/versions/0.92.0/mcp-server/` is removed
- AND `~/.cognicode/shims/cognicode-mcp` is removed

### Requirement: `cogh install` registers MCP server with IDEs

When `cogh install --ide <name>` is run for a registered IDE, `cogh` MUST dispatch to the IDE adapter for the `--ide` value. The dispatched adapter MUST:
1. Locate the IDE's config file (e.g. `~/.config/opencode/opencode.json`)
2. Patch the `mcp` section to add the CogniCode server entry
3. Symlink or copy skill bundles to the IDE's skill directory
4. Preserve any other MCP servers the user has configured
5. Write `mcp.cognicode-mcp.command[0]` as the absolute resolved shim path (`~/.cognicode/shims/cognicode-mcp`), not the bare binary name

When `--profile` is also set, the atomic bundle install MUST run first, then the IDE adapter MUST dispatch for any `--ide` value.

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

#### Scenario: `cogh install --ide opencode --profile core` dispatches to both bundle install and IDE adapter

- GIVEN `cogh install --ide opencode --profile core` runs
- WHEN the command completes
- THEN the atomic bundle install runs first (tracker/version is written)
- AND the opencode adapter dispatches and patches `opencode.json`
- AND `mcp.cognicode-mcp.command[0]` equals the absolute shim path

### Requirement: `cogh list` shows installed plugins and versions

`cogh list` MUST output a table of installed plugins and their
available + installed versions. The table format mirrors `asdf list`:

```
  Plugin          Installed        Available
  mcp-server      0.92.0           0.91.1, 0.92.0, 0.93.0
  skills-core     0.92.0           0.92.0
  opencode        0.1.0            0.1.0
```

#### Scenario: `cogh list` shows installed + available versions

- GIVEN `cogh install mcp-server --version 0.92.0` was run
- AND `cogh install skills-core --version 0.92.0` was run
- WHEN `cogh list` runs
- THEN output shows a table with `mcp-server` (0.92.0) and
  `skills-core` (0.92.0) rows
- AND shows the available versions column with the latest 5 versions

### Requirement: `cogh current` shows the active version pin

`cogh current` MUST read the current version pin from
`~/.cognicode/tracker/version` and print it. The tracker file is
plain text (one version string per line).

#### Scenario: `cogh current` reads the tracker

- GIVEN `~/.cognicode/tracker/version` contains `0.92.0`
- WHEN `cogh current` runs
- THEN output is `0.92.0`

### Requirement: `cogh latest <plugin>` queries the registry

`cogh latest <plugin>` MUST query the plugin registry (default =
GitHub Releases) for the latest stable version. The registry URL is
derived from the plugin manifest's `homepage` field.

#### Scenario: `cogh latest mcp-server` returns the latest version

- GIVEN the registry API responds with version `0.93.0`
- WHEN `cogh latest mcp-server` runs
- THEN output is `0.93.0`

#### Scenario: `cogh latest --all` lists all plugins

- WHEN `cogh latest --all` runs
- THEN output is a table with each plugin name + the latest version

### Requirement: `cogh update` resolves and installs latest

`cogh update [<plugin>]` MUST:
1. Resolve the latest stable version from the registry
2. Compare against the currently installed version
3. If different, install the new version
4. Regenerate shims
5. Update the tracker

If the user's `.cognicode.lock` file pins a specific version, `cogh
update` MUST respect the pin and refuse to update beyond it.

#### Scenario: `cogh update` upgrades to the latest

- GIVEN `cogh install mcp-server --version 0.92.0` was run
- AND the registry reports latest stable is `0.93.0`
- WHEN `cogh update mcp-server` runs
- THEN `~/.cognicode/versions/0.93.0/mcp-server/` exists
- AND `~/.cognicode/shims/cognicode-mcp` points to 0.93.0
- AND the previous 0.92.0 install is preserved (no auto-cleanup)

#### Scenario: `cogh update` respects the lock pin

- GIVEN `.cognicode.lock` pins `mcp-server` at `0.92.0`
- AND the registry reports latest stable is `0.93.0`
- WHEN `cogh update mcp-server` runs
- THEN output is "version 0.92.0 is pinned by .cognicode.lock; refusing to update"
- AND no install happens

### Requirement: `cogh uninstall` removes a version cleanly

`cogh uninstall <plugin> --version <v>` MUST:
1. Remove the version directory
2. Regenerate shims (if the version was the active one)
3. Update the tracker (if the version was the active one)
4. NOT remove other versions

#### Scenario: `cogh uninstall mcp-server --version 0.92.0`

- GIVEN `cogh install mcp-server --version 0.92.0` was run
- AND `cogh install mcp-server --version 0.91.1` was run
- WHEN `cogh uninstall mcp-server --version 0.92.0` runs
- THEN `~/.cognicode/versions/0.92.0/...` is removed
- AND `~/.cognicode/versions/0.91.1/...` is preserved
- AND `~/.cognicode/shims/cognicode-mcp` now points to 0.91.1

### Requirement: `cogh doctor` validates installation

`cogh doctor` MUST run a series of checks and report status:
- `cogh` binary in PATH
- All shims resolve to existing binaries
- `.cognicode.lock` parses correctly
- All installed IDEs are configured (if `--ide` matches)
- Plugin manifests are valid YAML

#### Scenario: `cogh doctor` reports PASS on a healthy install

- GIVEN `cogh install mcp-server --version 0.92.0` was run
- WHEN `cogh doctor` runs
- THEN output shows all checks passing

#### Scenario: `cogh doctor` reports FAIL on broken shims

- GIVEN `~/.cognicode/shims/cognicode-mcp` points to a missing binary
- WHEN `cogh doctor` runs
- THEN output shows the broken shim + remediation suggestion

### Requirement: `cogh` is curl-installable

The README MUST include a single-line install command:

```bash
curl -fsSL https://cognicode.dev/install.sh | sh
```

This installs `cogh` to `~/.cognicode/bin/cogh` and adds it to PATH
for the current shell.

## Cross-references

- ADR-034 — `cognicode-distribution-package`
- ADR-035 — `asdf-vm-version-management-pattern`
- ADR-036 — `IDE-abstraction-portable-skills-per-ide-adapters`
- `docs/specs/cognicode-plugin/spec.md`
- `docs/specs/cognicode-ide-adapter/spec.md`
- `docs/specs/portable-skill-bundle/spec.md`
- `docs/specs/cognicode-lifecycle/spec.md`
- `docs/adr/E32-cognicode-distribution.md`

## Implementation Log

- **2026-08-10 (E32-C plan)**: Spec drafted. Mirrors `asdf-vm` patterns
  with portable-skill + IDE-adapter additions.
