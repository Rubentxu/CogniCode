---
title: "ADR-036 — IDE abstraction: portable skills + per-IDE adapter plugins"
slug: "ADR-036"
status: accepted
date: 2026-08-10
deciders: Maintainer
related:
  - "[[ADR-034-cognicode-distribution-package]]"
  - "[[ADR-035-asdf-vm-version-management-pattern]]"
---

# ADR-036 — IDE abstraction: portable skills + per-IDE adapter plugins

## Context

CogniCode currently has 4 skills committed to
`~/.config/opencode/skills/` (sense-and-adapt, recent refactor,
recent rust, test-pyramid). These are tied to OpenCode's structure
(`SKILL.md` format with frontmatter + references to OpenCode-specific
paths).

The user (this cycle) wants:

1. **Skill portability** — the same skill logic should work on
   OpenCode, ZCode, Claude Code, Codex, and future agentic IDEs
2. **IDE-agnostic installation** — one `cogh install` command
   should configure all of them
3. **Future-proofing** — new IDEs should be a plugin, not a fork

This ADR scopes the **IDE abstraction layer** that sits on top of
ADR-034's distribution system.

## Investigation findings

### Current IDE skill placement

| IDE | Config root | Skill dir | MCP config |
|---|---|---|---|
| **OpenCode** | `~/.config/opencode/` | `skills/<name>/SKILL.md` | `opencode.json` mcp section |
| **ZCode** | `~/.zcode/` | `cli/plugins/.../skills/` | `config.json` mcp section |
| **Claude Code** | `~/.claude/` | `commands/`, `skills/`, `agents/` | `claude_desktop_config.json` |
| **Codex** | `~/.codex/` | `agents/`, `commands/` | `codex_config.json` |

(Note: ZCode, Claude Code, and Codex skill paths are partially inferred
from observed file structure; precise locations may differ across versions.)

### Skill format today

OpenCode's `SKILL.md` is a Markdown file with YAML frontmatter:

```yaml
---
name: architecture-discovery
description: Reverse-engineer a repository into the canonical architecture graph.
license: MIT
compatibility: opencode
metadata:
  version: "1.0.0"
  maturity: stable
  output-schema: c4-discover-report-v1
---
# Objective
...
# Required process
1. Resolve the project:
   ```bash
   archctl project resolve --cwd <dir>
   ```
...
```

The `SKILL.md` body is mostly **portable** — describe what the skill does
in natural language, plus bash/Python/shell commands. The `references/`
directory can include scripts. The `compat: opencode` field is the
only IDE-specific piece.

### MCP config formats

Each IDE stores MCP server config in a different JSON file with
different field names:

```json
// OpenCode
"mcp": {
  "cognicode-mcp": {
    "command": ["~/.cognicode/shims/cognicode-mcp"],
    "enabled": true,
    "type": "stdio"
  }
}

// Claude Code (claude_desktop_config.json)
"mcpServers": {
  "cognicode-mcp": {
    "command": "~/.cognicode/shims/cognicode-mcp",
    "args": []
  }
}

// Codex (assumed — varies)
"mcp_servers": [...]  // array form
```

The **name of the field** (`mcp` vs `mcpServers` vs `mcp_servers`) and
the **shape** (object vs array) differ across IDEs.

## Decision

**Portable skill bundles** + **per-IDE adapter plugins**.

### Portable skill bundle (`~/.cognicode/versions/<ver>/skills/<bundle>/`)

```yaml
---
name: cognicode-core
description: ...
license: MIT
metadata:
  version: 1.0.0
  maturity: stable
---

# Skill body (portable Markdown)
```

The skill bundle is **IDE-agnostic** — no `compatibility: opencode`
field, no IDE-specific paths. The skill is referenced by **content**
(name, description, body) and is portable across all supported IDEs.

A sketch of the portable layer:

```rust
// No new code; the existing cognicode-core skill stays opencode-flavored
// but the portable layer just stores copies:

~//.cognicode/versions/0.92.0/skills/cognicode-core/
├── SKILL.md          # portable content (no `compatibility` field)
├── README.md         # describes the skill
├── manifest.yaml     # cogh-parsable metadata
└── references/       # scripts, schema files
```

The 4 existing opencode skills (`sense-and-adapt`, `recent-refactor`,
`recent-rust`, `test-pyramid`) would be **re-packaged** as portable
bundles, dropping the `compatibility: opencode` field.

### Per-IDE adapter plugin (`~/.cognicode/plugins/<ide>/`)

```yaml
name: opencode
description: "OpenCode IDE integration"
install_assets:
  - source: skills/cognicode-core/        # source: relative to plugin dir
    target: ~/.config/opencode/skills/cognicode-core/
  - source: skills/cognicode-mcp-driven/
    target: ~/.config/opencode/skills/cognicode-mcp-driven/
  - source: mcp_config.json               # template with $COGNICODE_HOME
    target: ~/.config/opencode/opencode.json
    merge: true    # JSON merge (not overwrite)
    merge_path: "mcp.cognicode-mcp"   # JSON path to patch
mcp_template: |
  {
    "command": ["$COGNICODE_HOME/shims/cognicode-mcp"],
    "enabled": true,
    "type": "stdio"
  }
```

The IDE adapter:
1. **Reads** the IDE's existing config file
2. **Merges** the MCP template at the specified path
3. **Writes** back the merged config (preserves user's other MCP servers)
4. **Copies** skills to the IDE's skill directory

### Per-IDE adapter interface

Each IDE adapter is a **plugin** with a standard interface:

```yaml
name: <ide-name>
description: <human-readable>
detect:                       # how to detect if the IDE is installed
  - shell: code --version
  - file: ~/.config/<ide>/<config-file>
install:                      # how to install
  - kind: skills
    source: skills/
    target: ~/.config/<ide>/skills/cognicode/
  - kind: mcp
    template: mcp_config.json
    merge_path: mcp.cognicode-mcp
  - kind: commands
    source: commands/
    target: ~/.config/<ide>/commands/
uninstall:                    # how to clean up
  - remove: ~/.config/<ide>/skills/cognicode/
  - remove: ~/.config/<ide>/commands/cognicode-*.md
  - remove_from_json: ~/.config/<ide>/opencode.json:mcp.cognicode-mcp
```

The CLI handles the merge / write / remove logic; the adapter just
declares **what to do**.

### Adapter plugin directory

```
~/.cognicode/plugins/opencode/
├── plugin.yaml            # manifest above
├── skills/                # portable skill bundles
│   ├── cognicode-core/
│   └── cognicode-mcp-driven/
├── templates/
│   └── mcp_config.json
├── commands/
│   └── cognicode-status.md
└── README.md
```

### Identity abstraction

Each adapter declares an `ide` identifier. The CLI accepts:

```bash
cogh install --ide opencode
cogh install --ide zcode
cogh install --ide claude
cogh install --ide codex
cogh install --ide all
```

`--ide all` runs the install for every registered IDE adapter.

### Forward compatibility

A new IDE adapter (e.g. `cursor`, `windsurf`, `zed`) is a single
plugin: write `plugin.yaml` + bundle the assets. The user installs
it with `cogh plugin add cursor <git-url>` and `cogh install --ide cursor`.

## Codecbase layout

Adapters live in the CogniCode repo at `ide-adapters/<ide>/plugin.yaml`
and are bundled into the cogh distribution at build time. This way
the IDE adapters are versioned alongside the MCP server itself.

```
crates/cognicode-cli/                 # cogh binary
├── src/
│   ├── main.rs
│   ├── cli/
│   │   ├── install.rs
│   │   ├── uninstall.rs
│   │   ├── list.rs
│   │   ├── current.rs
│   │   ├── latest.rs
│   │   ├── update.rs
│   │   ├── plugin.rs
│   │   ├── use.rs
│   │   ├── reshim.rs
│   │   ├── doctor.rs
│   │   └── version.rs
│   ├── layout/
│   │   ├── paths.rs       # ~/.cognicode/... path resolution
│   │   ├── home.rs
│   │   └── shims.rs
│   ├── plugin/
│   │   ├── manifest.rs    # plugin.yaml reader
│   │   ├── registry.rs    # GitHub Releases client
│   │   ├── extract.rs     # tarball extraction
│   │   ├── verify.rs      # sha256 + signature
│   │   └── lifecycle.rs
│   ├── lockfile/
│   │   ├── read.rs        # .cognicode.lock parser
│   │   └── write.rs
│   └── adapters/
│       ├── trait.rs        # Adapter trait
│       ├── opencode.rs     # impl Adapter for OpenCode
│       ├── zcode.rs        # impl Adapter for ZCode
│       ├── claude.rs       # impl Adapter for Claude Code
│       └── codex.rs        # impl Adapter for Codex
└── ide-adapters/
    ├── opencode/
    │   ├── plugin.yaml
    │   ├── skills/
    │   ├── templates/
    │   └── commands/
    ├── zcode/
    └── claude/
```

## Why this is the right shape

1. **Single source of truth for skills**: the portable bundle lives
   in `crates/cognicode-cli/ide-adapters/<ide>/skills/`. The IDE
   adapter just copies it to the IDE-specific location.
2. **Reuse existing opencode skill format**: the portable bundle
   drops the `compatibility: opencode` field — that's the only
   change.
3. **Bidirectional**: skills can be re-extracted from any IDE back
   into the portable bundle (future direction).
4. **JSON merge is the right primitive**: most IDEs use JSON config.
   Versioned JSON merge (preserving user's other config) is the
   fragile bit — handled by cogh, not the adapter.
5. **Plugin discovery is decentralized**: a community adapter ships
   in its own git repo, registered via `cogh plugin add <git-url>`.

## Consequences

### Positive

- One `cogh install --ide all` configures everything
- New IDEs are a plugin, not a fork
- Portable skills can be re-targeted to any IDE
- Cleaner separation of concerns (skill content vs IDE mechanics)

### Negative

- JSON merge logic is fragile (user-modified config files)
- Adapter plugins can have bugs (the IDE-specific config shape may
  change across IDE versions)
- One more skill location to maintain

### Neutral

- The existing opencode skills need re-publication with the
  `compatibility` field removed
- The 4 IDE adapters required at first release: opencode, zcode,
  claude, codex

## Implementation

| Sub-cycle | Asunto | Est. LOC |
|---|---|---|
| E32 | cogh CLI binary core | 2K |
| E33 | plugin manifest + lockfile | 1K |
| E34 | opencode adapter | 0.5K |
| E35 | zcode adapter | 0.5K |
| E36 | claude adapter | 0.5K |
| E37 | codex adapter | 0.5K |
| E38 | portable skill re-publication | 0.2K |

## Cross-references

- ADR-034: `[[ADR-034-cognicode-distribution-package]]`
- ADR-035: `[[ADR-035-asdf-vm-version-management-pattern]]`
- `docs/specs/cognicode-ide-adapter/spec.md`
- `docs/specs/portable-skill-bundle/spec.md`

## Implementation Log

- **2026-08-10 (E32-C plan)**: ADR written. Decision: portable skill
  bundles + per-IDE adapter plugins with `trait Adapter` interface.
  4 IDE adapters for first release (opencode, zcode, claude, codex).
