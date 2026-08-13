---
title: "ADR-034 — Distribution package: cognicode-cli + skill-bundles + IDE adapters"
slug: "ADR-034"
status: accepted
date: 2026-08-10
deciders: Maintainer
context:
  - "asdf-vm architecture (https://asdf-vm.com)"
  - "OpenCode, ZCode, Claude Code, Codex IDE integration patterns"
  - "Current local-only CI (E31-B5) + sandbox distribution via `just recipes`"
---

# ADR-034 — Distribution package: cognicode-cli + skill-bundles + IDE adapters

## Context

CogniCode's runtime artifacts (MCP server, sandbox containers, skills,
agents) are currently distributed via three mechanisms:

1. **Source build**: `cargo build` + manual skill copy to
   `~/.config/opencode/skills/`
2. **Justfile recipes**: `just ci-t6`, `just scorecard-nightly` etc.,
   local-only per E31-B5 user directive
3. **None**: no pre-built binaries, no `npm install`, no `pip install`,
   no `cargo install cognicode`

This works for a single developer iterating on the codebase but blocks:

- **Distribution**: cannot give the binary to a colleague without
  a 30-minute build
- **Versioning**: there is no concept of "CogniCode 0.92.0" as a
  user-facing version — only `git tag` semantics
- **Lifecycle**: no `install`, `update`, `uninstall`, `list` commands
- **Multi-IDE**: skills are copied by hand to one IDE; Claude Code
  and Codex would need a separate copy path
- **Skill packaging**: 4 skills (sense-and-adapt, recent refactor,
  recent rust, test-pyramid) are opencode-specific; the same logic
  could be reused in other IDEs if extracted as a portable "skill bundle"

The user (this cycle) explicitly asked for an asdf-vm-inspired
distribution system, automation for OpenCode and ZCode, and an
abstraction layer for future agentic IDEs (Claude Code, Codex).

## Decision

Build a **`cognicode`** CLI tool (codename `cogh`) — a single binary
that:

1. **Installs** the CogniCode MCP server binary + sandbox container
   spec + skills bundle at `~/.cognicode/`
2. **Versions** per release (e.g. `0.92.0`, `0.93.0`) with a
   `~/.cognicode/versions/<version>/` layout (asdf style)
3. **Manages** via plugins: `cogh plugin add mcp-server`,
   `cogh plugin add skills-cognicode`, `cogh plugin add sandbox-templates`
4. **Reshimms** (regenerates integration scripts) on every install
5. **Dispatches** to IDEs via adapter plugins:
   `cogh install --ide opencode`, `cogh install --ide claude`,
   `cogh install --ide codex`
6. **Updates** in-place: `cogh update` resolves the latest version
   compatible with the current `cogh.lock`
7. **Uninstalls** cleanly: `cogh uninstall` removes binaries,
   shims, and IDE-specific config snippets

The cognicode CLI itself is a **small static binary** (single-file
release, no dynamic linking) — written in Rust, ships as a single
executable.

## Architecture

```
~/.cognicode/                          # COGNICODE_HOME (mirrors asdf ~/.asdf)
├── bin/
│   └── cogh                           # the CLI itself (small static binary)
├── versions/                          # co-versioned installs (asdf-style)
│   ├── 0.92.0/
│   │   ├── bin/
│   │   │   ├── cognicode-mcp          # the MCP server
│   │   │   ├── cognicode-explorer-api # the API server
│   │   │   └── cognicode-cli          # the standalone CLI
│   │   ├── skills/                    # portable skill bundles
│   │   │   ├── cognicode-core/
│   │   │   │   ├── SKILL.md
│   │   │   │   └── scripts/
│   │   │   ├── cognicode-mcp-driven/
│   │   │   └── cognicode-sandbox/
│   │   ├── containers/
│   │   │   └── cognicode-{rust,ts,py,go,java}.container
│   │   ├── manifests/
│   │   │   └── e31b4rollup_tier1_ts_py_closure.yaml
│   │   └── VERSION                    # version stamp
│   └── 0.93.0/...                     # future
├── plugins/                           # IDE adapters (one per IDE)
│   ├── opencode/
│   │   ├── bin/install-skills         # writes to ~/.config/opencode/
│   │   └── bin/install-mcp            # patches opencode.json
│   ├── zcode/
│   │   ├── bin/install-skills         # writes to ~/.zcode/...
│   ├── claude/
│   │   └── bin/install-skills         # writes to ~/.claude/...
│   └── codex/
│       └── bin/install-skills         # writes to ~/.codex/...
├── shims/                             # symlinks to current version
│   ├── cognicode-mcp -> ../versions/0.92.0/bin/cognicode-mcp
│   ├── cognicode-explorer-api -> ../versions/0.92.0/bin/cognicode-explorer-api
│   └── ...
├── tracker/                           # current version pin (asdf .tool-version style)
│   └── version                        # "0.92.0"
├── locks/                             # per-project lock
│   └── cognicode.lock                 # note: per-project is .cognicode.lock
└── cache/
    └── downloads/                     # tarball cache
```

**Per-project**: `.cognicode.lock` (JSON or TOML) declares the version
that project requires. `cogh install` reads it and selects the right
version.

## Asdf-vm pattern adaptation

| **asdf-vm concept** | **cogh equivalent** |
|---|---|
| `~/.asdf/` | `~/.cognicode/` |
| `~/.asdf/bin/asdf` | `~/.cognicode/bin/cogh` |
| `~/.asdf/shims/` | `~/.cognicode/shims/` |
| `~/.asdf/installs/<tool>/<ver>/` | `~/.cognicode/versions/<ver>/` |
| `~/.asdf/plugins/<tool>/` | `~/.cognicode/plugins/<plugin>/` |
| `~/.tool-versions` (per-project) | `.cognicode.lock` (per-project) |
| `asdf plugin add <name>` | `cogh plugin add <plugin>` |
| `asdf install <tool> <ver>` | `cogh install <plugin> <ver>` |
| `asdf current` | `cogh current` |
| `asdf latest` | `cogh latest` |
| `asdf uninstall <tool> <ver>` | `cogh uninstall <plugin> <ver>` |
| `asdf reshim` | `cogh reshim` |
| `asdf list` | `cogh list` |
| `asdf global <tool> <ver>` | `cogh global <plugin> <ver>` |

## CLI surface

```bash
cogh --version                          # cogh 0.1.0 (managing CogniCode 0.92.0)
cogh install <plugin> [--version X.Y.Z] [--ide <name>...]
cogh uninstall <plugin> [--version X.Y.Z]
cogh list [--installed] [--available]
cogh current
cogh latest [<plugin>]
cogh update [<plugin>]
cogh use [--global|--local] <plugin> <version>
cogh reshim
cogh plugin add <name> [<git-url>]
cogh plugin list
cogh plugin remove <name>
cogh doctor                            # verify all integrations
cogh where <binary>                    # path to current version
cogh version                            # shows both cogh + CogniCode version
```

### Plugin manifest (`~/.cognicode/plugins/<name>/plugin.yaml`)

```yaml
name: mcp-server
description: "CogniCode MCP server — 68 tools, rust binary"
homepage: https://github.com/Rubentxu/CogniCode
versions:
  - ref: v0.92.0
    artifact: cognicode-mcp-0.92.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "<digest>"
    url: https://github.com/Rubentxu/CogniCode/releases/download/v0.92.0/cognicode-mcp-0.92.0-x86_64-unknown-linux-gnu.tar.gz
  - ref: v0.91.1
    artifact: cognicode-mcp-0.91.1-x86_64-unknown-linux-gnu.tar.gz
    sha256: "<digest>"
    url: https://github.com/Rubentxu/CogniCode/releases/download/v0.91.1/cognicode-mcp-0.91.1-x86_64-unknown-linux-gnu.tar.gz
install:
  - id: extract
    run: tar -xzf $ARTIFACT -C $INSTALL_DIR
  - id: shim
    run: ln -sf $INSTALL_DIR/bin/cognicode-mcp ~/.cognicode/shims/cognicode-mcp
ide_integrations:
  - template: opencode-mcp
    requires: mcp
  - template: zcode-mcp
    requires: mcp
```

### IDE adapter plugin (`~/.cognicode/plugins/opencode/plugin.yaml`)

```yaml
name: opencode
description: "OpenCode IDE integration"
install_assets:
  - source: skills/
    target: ~/.config/opencode/skills/cognicode-$PLUGIN_VERSION/
  - source: mcp.json
    target: ~/.config/opencode/opencode.json
    merge: true    # patch, not overwrite
  - source: agents/cognicode-*.md
    target: ~/.config/opencode/agents/
```

Each IDE adapter is a **separate plugin** so they can be installed
independently. The `cogh install --ide opencode` command:
1. Resolves the IDE plugin
2. Reads its `install_assets`
3. Writes skills to the IDE's skill directory
4. Patches the IDE's MCP config to add the CogniCode server
5. Creates shims that the IDE can `PATH` to

## Plugin discovery

For the first release, we ship **bundled plugins** with cogh:
- `mcp-server` (the binary)
- `skills-cognicode-core` (the 4 portable skills)
- `sandbox-templates` (the podman container specs)
- `opencode` (the IDE adapter)
- `zcode`, `claude`, `codex` (future IDE adapters)

## Why this is the right shape

1. **Asdf-aligned**: developers already know `asdf plugin add`,
   `asdf install`, `.tool-versions`. cogh is a small twist on a
   familiar pattern.
2. **Single binary**: `cogh` is a static Rust binary with no dynamic
   dependencies. Easy to ship, easy to curl-install.
3. **Per-project lock**: `.cognicode.lock` mirrors `Cargo.lock` /
   `package-lock.json` — version pinning is familiar and expected.
4. **IDE-agnostic**: skills are portable (`SKILL.md` format is close
   to neutral). The IDE adapter plugins translate the portable skill
   into the IDE-specific config snippet.
5. **OpenCode + ZCode today; Claude + Codex tomorrow**: the adapter
   plugin interface makes new IDEs a 200 LOC plugin, not a fork.
6. **Distribution via GitHub Releases**: pre-built binaries have
   shasums in `plugin.yaml` — no third-party registry needed.

## Effects

### Positive

- One command (`cogh install`) replaces ~15 minutes of manual setup
- Version pinning via `.cognicode.lock` enables reproducible reviewer
  environments
- IDE-agnostic design means new agentic IDEs are a plugin, not a fork
- Lifecycle management (install/update/uninstall) is formalized
- Adheres to the existing E31-B5 directive (local control via `cogh`
  CLI, no remote registry dependency)

### Negative

- New CLI to maintain (cogh itself is a Rust project)
- Pre-built binaries require CI/CD infrastructure (GitHub Releases)
- Plugin discovery requires a registry (initially GitHub org)
- Per-IDE skill translation adds maintenance overhead

### Neutral

- Sandbox containers remain in podman (per E31-B5)
- Local-only CI remains the default (per E31-B5)
- MCP server connection (stdio vs remote) per-IDE adapter

## Implementation plan

| Sub-cycle | Asunto | PR |
|---|---|---|
| E32 | cognicode CLI binary (`cogh`) with asdf-style commands | TBD |
| E33 | plugin manifest + `~/.cognicode/` layout | TBD |
| E34 | mcp-server plugin + bundled skills | TBD |
| E35 | opencode IDE adapter | TBD |
| E36 | zcode IDE adapter | TBD |
| E37 | install / uninstall / update lifecycle tests | TBD |

The first 3 sub-cycles (E32-E34) are needed for the `cogh install`
end-to-end demo. E35-E37 are follow-ups.

## Cross-references

- ADR-035 (asdf-vm version-management pattern)
- ADR-036 (IDE-abstraction pattern)
- `docs/specs/cognicode-cli/spec.md` (OpenSpec)
- `docs/specs/cognicode-plugin/spec.md` (OpenSpec)
- `docs/specs/cognicode-ide-adapter/spec.md` (OpenSpec)
- `docs/adr/E32-cognicode-distribution.md` (sub-cycle plan)

## Implementation Log

- **2026-08-10 (E32-C plan)**: ADR written based on asdf-vm architecture
  study + OpenCode MCP config pattern. Decision: `cogh` CLI as
  single static binary, `~/.cognicode/` layout, per-project
  `.cognicode.lock`, IDE adapter plugins.
