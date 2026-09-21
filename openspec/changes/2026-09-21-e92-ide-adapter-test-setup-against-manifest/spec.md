# Spec — e92: IDE-adapter integration tests must stand up a bundle manifest

## Requirement

The CLI integration test binary `cognicode_ide_adapter` (in
`crates/cognicode-cli/tests/cognicode_ide_adapter.rs`) MUST plant a valid
v2 bundle manifest at `<cogh_home>/versions/<version>/manifest.yaml`
before invoking `cogh ide install` / `cogh ide uninstall`, because the
production code introduced in cycle `arch(debt2)` (commit `9752cc52`,
2026-09-18) resolves skill bundles from that manifest and errors out
otherwise.

## Scenarios

### Scenario 1: opencode install against `latest`

**Given** a `cogh_home` initialized via `cogh init` (creates the directory tree)
**And** a v2 bundle manifest planted at `<cogh_home>/versions/latest/manifest.yaml`
declaring the `core` profile, the `cognicode-mcp` component, and at least one
`skill_bundles[]` entry (`id: skills-for-test`) whose physical directory
`<cogh_home>/versions/latest/skills/skills-for-test/` exists with a `SKILL.md`
placeholder
**When** the user runs `cogh ide install opencode --plugin mcp-server`
(equivalent to running `cogh ide install opencode` with default `--version latest`)
**Then** the command exits 0
**And** the opencode config (`~/.config/opencode/opencode.json`) gains a
`cognicode-mcp` MCP entry that preserves any pre-existing entries.

### Scenario 2: opencode uninstall against a specific version

**Given** a `cogh_home` initialized via `cogh init`
**And** a v2 bundle manifest planted at `<cogh_home>/versions/latest/manifest.yaml`
**And** a v2 bundle manifest planted at `<cogh_home>/versions/0.94.15/manifest.yaml`
with the same shape as Scenario 1
**When** the user runs `cogh ide uninstall opencode --version 0.94.15`
**Then** the command exits 0
**And** the opencode config no longer carries a `cognicode-mcp` entry
**And** any other MCP entries (e.g. `chronos`) are preserved.

### Scenario 3: zcode install against `latest`

**Given** a `cogh_home` initialized via `cogh init`
**And** a v2 bundle manifest planted at `<cogh_home>/versions/latest/manifest.yaml`
with the same shape as Scenario 1
**And** a stub zcode config at `~/.zcode/v2/config.json`
**When** the user runs `cogh ide install zcode`
**Then** the command exits 0
**And** the zcode config gains a `cognicode-mcp` MCP entry under the
zcode-specific path (`mcp_servers.cognicode-mcp.command` per the zcode v2 schema).

### Scenario 4: manifest version mismatch (negative)

**Given** a `cogh_home` initialized via `cogh init`
**And** a manifest at `<cogh_home>/versions/latest/manifest.yaml` whose
`version:` field is the literal string `latest` (not a semver)
**When** the user runs `cogh ide install opencode`
**Then** the command exits non-zero
**And** the stderr mentions
`version must match ^\d+\.\d+\.\d+(-.*)?$, got latest`.

This pins the schema's semver invariant in the test suite so a future
loosening of the regex is caught.

## Refs

- `crates/cognicode-cli/src/cmd/ide.rs::cmd_ide_install_opencode` /
  `cmd_ide_install_zcode` / `cmd_ide_uninstall_opencode`
- `crates/cognicode-cli/src/cmd/bundle_manifest.rs::declared_skill_bundle_dirs`
- `crates/cognicode-cli/src/cmd/layout.rs::CogniCodeHome::skills_root`
