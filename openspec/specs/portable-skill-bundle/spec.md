# Portable Skill Bundle

## Purpose

The portable skill bundle format that is portable across agentic IDEs
(OpenCode, ZCode, Claude Code, Codex, future). The same skill content
(SKILL.md body + scripts) is delivered through every supported IDE
via the per-IDE adapter (`cognicode-ide-adapter` spec).

See ADR-036 for the design rationale.

## Requirements

### Requirement: Skill bundle is a portable directory tree

A portable skill bundle is a directory:

```
<skill-name>/
├── SKILL.md              # required: portable content
├── README.md              # optional: human-readable description
├── manifest.yaml          # required: cogh-parsable metadata
├── references/            # optional: scripts, schema files
│   └── *.sh
│   └── *.py
└── assets/                # optional: data files, images
```

The `SKILL.md` is the primary content. The `manifest.yaml` declares
cog-managed metadata. The `references/` and `assets/` directories
support the skill.

#### Scenario: Skill bundle loads

- GIVEN a portable skill bundle directory `cognicode-core/`
- AND `~/.cognicode/versions/0.92.0/skills/cognicode-core/` exists
- WHEN `cogh list --skills` runs
- THEN the bundle is listed with its name + description

### Requirement: `SKILL.md` has portable YAML frontmatter

```yaml
---
name: cognicode-core
description: Core CogniCode workflow — analyze, graph, navigate.
license: MIT
metadata:
  version: "1.0.0"
  maturity: stable
  author: CogniCode Team
  homepage: https://github.com/Rubentxu/CogniCode
---

# Skill body in Markdown
...
```

The `name`, `description`, `license`, and `metadata.version` fields are
mandatory. The `metadata.maturity` is one of `experimental`,
`beta`, `stable`, `deprecated`.

#### Scenario: `SKILL.md` frontmatter parses

- GIVEN a `SKILL.md` with YAML frontmatter
- WHEN `cogh inspect <skill>` runs
- THEN the frontmatter is parsed and printed
- AND the body is shown as raw Markdown

#### Scenario: Missing mandatory fields fail

- GIVEN `SKILL.md` is missing `name`
- WHEN `cogh install --ide opencode` runs
- THEN the install fails with "skill missing required field: name"

### Requirement: Portable bundle has NO IDE-specific fields

The portable bundle MUST NOT contain fields like:
- `compatibility: opencode` (this is IDE-specific)
- Hardcoded paths to `~/.config/opencode/`
- References to `opencode.json` config keys

The bundle is rendered **IDE-agnostic**. The IDE adapter plugin is
responsible for translating the portable form into the IDE-specific
form.

#### Scenario: Portable bundle is IDE-agnostic

- GIVEN a portable skill bundle at `~/.cognicode/skills/cognicode-core/`
- WHEN the bundle is validated
- THEN the `SKILL.md` has no `compatibility: opencode` field
- AND no file paths in the bundle reference `~/.config/opencode`
- AND no config keys reference `opencode.json`

### Requirement: `manifest.yaml` declares cogh metadata

```yaml
apiVersion: cognicode/v1
kind: SkillBundle
name: cognicode-core                    # unique identifier
description: Core CogniCode workflow
version: "1.0.0"                         # bundle version (semver)
maturity: stable                          # experimental|beta|stable|deprecated
homepage: https://github.com/Rubentxu/CogniCode/skills
authors:
  - CogniCode Team

requires:                                  # optional: required plugins
  - mcp-server

ide_compatibility:                        # optional: which IDEs this bundle supports
  - opencode
  - zcode
  - claude
  - codex
```

The `manifest.yaml` is the cogh-side metadata. The `SKILL.md`
frontmatter is the IDE-side metadata (used by the IDE to display
the skill).

#### Scenario: `manifest.yaml` declares bundle metadata

- GIVEN a portable skill bundle with `manifest.yaml`
- WHEN `cogh inspect <skill>` runs
- THEN the manifest is parsed and printed
- AND maturity is one of the 4 valid values

### Requirement: Skill bundles are versioned with the CogniCode version

`cogh install mcp-server --version 0.92.0` installs the MCP server
at `0.92.0` AND the bundled skills at `0.92.0`. The skill bundle
version is tied to the CogniCode version.

#### Scenario: Skills are versioned with the MCP server

- GIVEN `cogh install mcp-server --version 0.92.0` runs
- AND the `mcp-server` plugin has bundled skills
- THEN the skills are installed at `~/.cognicode/versions/0.92.0/skills/`
- AND the IDE adapter copies them to `~/.config/opencode/skills/cognicode-0.92.0/`

### Requirement: Skills with `requires` plugins are installed together

If a portable skill bundle declares `requires: [mcp-server]`, `cogh
install <skill>` MUST also install the required plugins.

#### Scenario: Skill install cascades to required plugins

- GIVEN a portable skill bundle `cognicode-graph` with `requires: [mcp-server]`
- AND `mcp-server` is NOT installed
- WHEN `cogh install cognicode-graph` runs
- THEN `cogh install mcp-server` runs first
- AND the skill is installed after

### Requirement: `references/` and `assets/` are copied recursively

When the IDE adapter plugin copies a skill bundle, it MUST copy
all files in `references/` and `assets/` recursively. The relative
subdirectory structure is preserved.

#### Scenario: Reference files are copied recursively

- GIVEN a portable skill bundle with `references/scripts/analyze.sh`
- AND `references/scripts/helpers/validate.sh`
- AND `assets/schema.json`
- WHEN the IDE adapter copies the bundle
- THEN the target directory has:
  - `references/scripts/analyze.sh`
  - `references/scripts/helpers/validate.sh`
  - `assets/schema.json`

### Requirement: Release manifest declares shipped skill bundles (DEBT-2)

The release `bundle.yaml` (`cognicode.bundle/v2`) MUST carry an
optional, additive `skill_bundles[]` section declaring every portable
skill bundle the release ships:

```yaml
skill_bundles:
  - id: skills-for-claude      # SkillBundleId (unique within the manifest)
    version: "0.95.0"          # must equal the bundle version
    profiles: [core]           # profiles that ship the bundle
```

The `SkillBundleId` is an **explicit declaration**. It MUST NOT be
derived from a `ComponentId`, a `BinaryName`, a `PluginId`, or a
directory scan. Manifests without the field keep their exact
pre-DEBT-2 meaning (the field defaults to empty).

Validation invariants:
- ids are non-empty and unique within the manifest;
- each bundle's `version` equals the bundle version (lockstep);
- every referenced profile is declared in `profiles[]`.

#### Scenario: Manifest without skill_bundles parses unchanged

- GIVEN a valid `cognicode.bundle/v2` manifest with no `skill_bundles`
- WHEN the manifest is parsed
- THEN parsing succeeds and `skill_bundles` is empty

#### Scenario: Declared SkillBundleId is resolved verbatim

- GIVEN a manifest declaring `skill_bundles: [{id: skills-for-claude, ...}]`
- AND `skills-for-claude` differs from every `components[].name`
- WHEN the skill bundles for a profile are resolved
- THEN the id `skills-for-claude` is returned verbatim
- AND no id is ever inferred from a component or binary name

#### Scenario: Duplicate ids or version drift are rejected

- GIVEN a manifest with two skill bundles sharing an id
- OR a skill bundle whose version differs from the bundle version
- WHEN the manifest is parsed
- THEN validation fails loudly

### Requirement: Consumers resolve skill bundles from the manifest, never from a directory scan

Install-time and IDE-integration code paths MUST resolve skill
bundle sources through the canonical helper
(`bundle_manifest::declared_skill_bundle_dirs`) against the version's
canonical `skills/` root. The retired heuristics — "first directory
under `skills_root`" (`read_dir().next()`) and
`<root>/versions/<v>/<plugin>/skills/` — MUST NOT be reintroduced.

#### Scenario: Declared-and-present bundle is integrated

- GIVEN a manifest declaring skill bundle `skills-for-claude` for `core`
- AND `versions/<v>/skills/skills-for-claude/` exists
- WHEN an IDE integration runs for profile `core`
- THEN the bundle at that manifest-derived path is copied/linked

#### Scenario: Declared-but-missing bundle fails loudly

- GIVEN a manifest declaring skill bundle `skills-for-claude`
- AND `versions/<v>/skills/skills-for-claude/` does not exist
- WHEN an IDE integration or install runs
- THEN the operation fails naming the id
- AND no substitute directory is guessed

#### Scenario: Non-declaring manifest skips integration

- GIVEN a manifest with no skill bundles for the active profile
- WHEN an IDE integration runs
- THEN the integration is skipped with an explicit notice
- AND no on-disk directory is picked up as a fallback

## Cross-references

- ADR-036 — `IDE-abstraction-portable-skills-per-ide-adapters`
- `docs/specs/cognicode-cli/spec.md`
- `docs/specs/cognicode-ide-adapter/spec.md`

## Implementation Log

- **2026-08-10 (E32-C plan)**: Spec drafted. Portable format
  documented (`SKILL.md` + `manifest.yaml` + `references/` +
  `assets/`). Constraints on IDE-agnostic content documented.
