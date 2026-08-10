---
title: "ADR-035 — asdf-vm version-management pattern"
slug: "ADR-035"
status: accepted
date: 2026-08-10
deciders: Maintainer
related:
  - "[[ADR-034-cognicode-distribution-package]]"
---

# ADR-035 — asdf-vm version-management pattern

## Context

asdf-vm is a tool version manager written in Go (~5K LOC). It manages
multiple language runtimes (Node, Ruby, Python, etc.) from a single
CLI. The user (this cycle) explicitly asked for an asdf-vm-inspired
distribution system for CogniCode.

We need to understand asdf-vm's design deeply because:

1. It solves the same problem: one CLI, multiple plugin-managed tools,
   per-project version pinning
2. Its plugin-API is small (~7 commands) and well-documented
3. Its shim approach is the cleanest way to make "anywhere-runnable"
   binaries coherent across versions
4. The `.tool-versions` per-project file is familiar to most devs

## Investigation findings

### asdf-vm architecture (studied 2026-08-10)

```
~/.asdf/
├── bin/asdf                            # 5K LOC Go binary
├── shims/                              # auto-generated symlinks per binary
│   ├── python -> ../installs/python/3.12.0/bin/python
│   └── ruby -> ../installs/ruby/3.3.0/bin/ruby
├── installs/<tool>/<version>/         # versioned, immutable
├── plugins/<tool>/                     # plugin scripts (bash)
│   ├── bin/install
│   ├── bin/list-all
│   ├── bin/latest-stable
│   └── bin/post-plugin-update
├── tmp/                                # build artifacts
├── downloads/                          # cached tarballs
└── (legacy) shims and completions
```

**Plugin API** (each plugin implements these exec scripts):

| Script | Purpose | Returns |
|---|---|---|
| `bin/install` | install a version | exit 0 on success |
| `bin/list-all` | list available versions | printable table |
| `bin/latest-stable` | current latest stable | version string |
| `bin/latest-pre` | current latest pre-release | version string |
| `bin/list-legacy-filenames` | legacy filename handling | filename |
| `bin/parse-legacy-file` | legacy config parsing | version |
| `bin/postinstall` | hook after install | exit 0 |
| `bin/post-plugin-update` | hook after plugin update | exit 0 |
| `bin/uninstall` | uninstall a version | exit 0 |

**Core commands** (`asdf <cmd>`):

| Command | Purpose |
|---|---|
| `asdf plugin add <name> [<git-url>]` | register a plugin |
| `asdf plugin list [--urls] [--refs]` | list installed plugins |
| `asdf plugin remove <name>` | unregister |
| `asdf plugin update --all` | git pull on plugin repos |
| `asdf install <tool> <version>` | install a version |
| `asdf install <tool> latest` | install latest stable |
| `asdf uninstall <tool> <version>` | remove a version |
| `asdf current [<tool>]` | show current version |
| `asdf latest <tool>` | show latest stable |
| `asdf list <tool>` | list installed versions |
| `asdf list-all <tool>` | list all available versions |
| `asdf reshim <tool> <version>` | regenerate shims |
| `asdf global <tool> <version>` | set global default |
| `asdf local <tool> <version>` | set per-project |

**Per-project file**: `.tool-versions` (default location) — simple
whitespace-separated `tool version` lines:

```
nodejs 20.0.0
ruby 3.3.0
python 3.12.0
terraform 1.7.0
```

**Shim mechanism**: every binary in a tool's `bin/` directory gets a
shim symlink in `~/.asdf/shims/`. The shim is a small script that
delegates to the version-specific binary. Prepending `~/.asdf/shims`
to PATH makes every version's binaries addressable.

## Decision

**Adopt the asdf-vm pattern 1:1** for CogniCode, with the following
mapping:

| **asdf** | **cogh** |
|---|---|
| Single binary (Go) | Single binary (Rust) |
| `~/.asdf/...` | `~/.cognicode/...` |
| `bin/asdf` | `bin/cogh` |
| Plugins (bash scripts) | Plugins (TOML + static binary) |
| `installs/<tool>/<ver>/` | `versions/<ver>/<plugin>/` |
| `.tool-versions` (per-project) | `.cognicode.lock` (per-project) |
| Shims symlinks | Shims symlinks |
| `asdf` (no namespace) | `cogh` (short, mnemonically "cognicode home") |

### Why 1:1 instead of "novel"

- Lower cognitive load for users who already know asdf
- Mitigates risk by reusing a proven design (asdf has 10+ years of
  edge-case hardening)
- Smaller spec surface → faster implementation
- Easier to write docs and tutorials

### Deviations from asdf

1. **Plugin manifests are TOML/YAML**, not bash scripts. asdf plugins
   are bash because asdf needs to be portable across shells. cogh
   is a single Rust binary that reads `plugin.yaml` — easier to
   version, validate, and sign.
2. **`.cognicode.lock` is JSON**, not `.tool-versions` text. Project
   lockfile style matches `Cargo.lock` / `package-lock.json` — more
   expressive (deps + hashes + provenance).
3. **`cogh` includes a registry client** (default = GitHub Release
   artifacts). asdf assumes you have a git-cloned plugin; cogh
   assumes plugins can be hosted as static release artifacts.
4. **cogh is opinionated about IDE integration**. asdf doesn't know
   about editors; cogh has explicit `install --ide <name>` because
   most CogniCode users want MCP + skills + config in one shot.

## Consequences

### Positive

- Lower learning curve for asdf users
- Battle-tested design (10+ years of edge cases)
- Smaller cogh source (probably 3K LOC vs 10K if we invented from scratch)
- Plugin author boilerplate is minimal (1 `plugin.yaml` + 1 tarball)

### Negative

- asdf's plugin model has some quirks (e.g. legacy file support)
  that cogh inherits
- The asdf community may mistake cogh for an asdf competitor
- The asdf name itself is also a Rust tool `rtx` (now `mise`) — we
  avoid that naming clash by using `cogh` not `cog`

## Implementation

See ADR-034. The asdf-pattern is fairly direct to implement; the
novel work is **plugin manifest format** and **IDE adapter plugins**.

## References

- asdf-vm: https://asdf-vm.com
- asdf-vm source: https://github.com/asdf-vm/asdf (Go, ~5K LOC)
- ADR-034: `[[ADR-034-cognicode-distribution-package]]`
- `docs/specs/cognicode-cli/spec.md` (OpenSpec)

## Implementation Log

- **2026-08-10 (E32-C plan)**: ADR written. Decision: 1:1 mapping
  with deviations (TOML manifests, JSON lockfile, IDE opinionated).
