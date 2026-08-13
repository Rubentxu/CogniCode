# CogniCode Plugin Manifest

## Purpose

The plugin manifest format that `cogh install` reads to discover
plugins, install artifacts, and integrate with IDEs. Inspired by
asdf-vm's plugin scripts but using a typed YAML format
instead of bash.

See ADR-034 and ADR-035 for the architecture.

## Requirements

### Requirement: `plugin.yaml` is the canonical manifest format

A plugin MUST declare its identity in `plugin.yaml` at the root of
the plugin's source directory. The manifest MUST be valid YAML.
Fields are versioned; new fields MAY be added in minor versions
without breaking existing parsers.

Schema:

```yaml
apiVersion: cognicode/v1
kind: Plugin
name: <plugin-name>            # unique identifier (alphanumeric, dash)
description: <one-line>        # human-readable description
homepage: <url>                # optional, used for registry lookup
repository: <git-url>          # optional, for `cogh plugin add <name>`

versions:                       # list of available versions
  - ref: v0.92.0                # human-readable version ref
    artifact: <tarball>         # filename in the GitHub Release
    sha256: <hex>               # mandatory: sha256 of the artifact
    url: <download-url>          # mandatory: where to download
    min_cogh: ">=0.1.0"         # optional: minimum cogh version

default_install:                # install steps (in order)
  - id: extract
    run: tar -xzf $ARTIFACT -C $INSTALL_DIR
  - id: shim
    run: ln -sf $INSTALL_DIR/bin/<binary> ~/.cognicode/shims/<binary>

binaries:                       # binaries to shim
  - name: cognicode-mcp          # shim name
    path: bin/cognicode-mcp      # path within INSTALL_DIR
    description: "CogniCode MCP server"

ide_integrations:               # optional: which IDEs this plugin supports
  - opencode
  - zcode
```

#### Scenario: `plugin.yaml` parses with cogh

- GIVEN a plugin directory with `plugin.yaml`
- WHEN `cogh install <plugin>` runs
- THEN the manifest is read and parsed
- AND validators reject the manifest if required fields are missing
- AND validators reject if `sha256` does not match the downloaded artifact

### Requirement: Versions are addressable by ref

A version MUST be addressable by an opaque `ref` string. The `cogh
install <plugin> --version <ref>` command takes the ref as a
positional argument. The version is an arbitrary string (e.g.,
`v0.92.0`, `0.92.0`, `latest`, `stable`, `nightly`).

#### Scenario: `cogh install mcp-server --version v0.92.0` resolves

- GIVEN the `mcp-server` plugin has a `versions` entry with `ref: v0.92.0`
- WHEN `cogh install mcp-server --version v0.92.0` runs
- THEN the artifact for `v0.92.0` is downloaded
- AND the install steps run

#### Scenario: `cogh install mcp-server --version latest` resolves

- GIVEN the registry returns `0.93.0` as latest stable
- WHEN `cogh install mcp-server --version latest` runs
- THEN the artifact for `0.93.0` is downloaded
- AND the install steps run

### Requirement: `sha256` integrity check is mandatory

The `sha256` field on each version MUST be verified against the
downloaded artifact. A mismatch MUST abort the install with a clear
error message.

#### Scenario: `sha256` mismatch aborts the install

- GIVEN the manifest declares `sha256: abc123...`
- AND the downloaded artifact has a different hash
- WHEN `cogh install <plugin>` runs
- THEN the install aborts with "sha256 mismatch: expected abc123..., got <actual>"
- AND no partial install state is left behind

### Requirement: Plugin discovery via GitHub registry

Default plugin discovery is via GitHub Releases. The plugin's
### Requirement: Bundled plugins ship with cogh

The first release of `cogh` ships with 4 bundled plugins:
- `mcp-server` — the CogniCode MCP server
- `skills-cognicode-core` — portable skill bundles
- `sandbox-templates` — podman container specs
- `opencode` — OpenCode IDE adapter

These plugins are embedded in the `cogh` binary at build time
(via `include_str!`) and pre-installed when `cogh init` runs.

#### Scenario: `cogh init` installs bundled plugins

- GIVEN `cogh` is freshly installed
- AND `~/.cognicode/` does not exist
- WHEN `cogh init` runs
- THEN the bundled plugins are registered in `~/.cognicode/plugins/`
- AND the user can immediately run `cogh install mcp-server`

### Requirement: Plugin discovery respects the user's pinned registry

The user MAY override the default GitHub registry with
`~/.cognicode/config.yaml`:

```yaml
registry:
  type: github
  url: https://github.com/my-org
  token: ${GITHUB_TOKEN}
```

If a `token` is provided, `cogh` MUST use it for authenticated
requests (useful for private registries).

#### Scenario: `cogh install` uses a custom registry

- GIVEN `~/.cognicode/config.yaml` has `registry.url: https://github.com/my-org`
- WHEN `cogh install mcp-server` runs
- THEN `cogh` queries `https://api.github.com/repos/my-org/cognicode-plugins/mcp-server/releases/...`
- AND uses the configured token (if any) for authentication

## Cross-references

- ADR-034 — `cognicode-distribution-package`
- ADR-035 — `asdf-vm-version-management-pattern`
- `docs/specs/cognicode-cli/spec.md`
- `docs/specs/cognicode-ide-adapter/spec.md`

## Implementation Log

- **2026-08-10 (E32-C plan)**: Spec drafted. Schema is YAML
  (similar to the OpenCode commands format). Validation rules
  documented (sha256 mandatory, ref opaque).
