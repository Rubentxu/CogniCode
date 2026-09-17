# Skill productization spec

This spec captures the **requirements** for the skills this cycle
delivers. The full design lives in `../design.md`; the WU-level
results live in `../artifacts/`.

## REQ-84.1-01 — `cognicode` skill (entry point)

A `skills/cognicode/SKILL.md` MUST exist with:

- Frontmatter declaring `name: cognicode`, a non-empty
  `description`, and `license: MIT`.
- A "Step 1 — verify the install" section listing the four
  commands: `cogh version`, `cogh doctor`, `cognicode --version`,
  `cognicode doctor`.
- A "Step 3 — first discovery" section teaching both the CLI
  surface (`cognicode analyze`, `cognicode index build`, etc.) and
  the MCP surface (`project_overview`, `project_insights`,
  `codebase_map`).
- A "Step 5 — when to load the deeper skill" section pointing to
  `cognicode-mcp`.

A `skills/cognicode/manifest.yaml` MUST declare
`apiVersion: cognicode/v1`, `kind: SkillBundle`, and the same
`name: cognicode`.

**Validation:** `python3 scripts/validate_skills.py --skill cognicode`
returns `PASS`.

## REQ-84.1-02 — `cognicode-mcp` skill (workflow guide)

A `skills/cognicode-mcp/SKILL.md` MUST exist with:

- Frontmatter declaring `name: cognicode-mcp`, a non-empty
  description, and `license: MIT`.
- A "Capability domains" table covering DISCOVER/NAVIGATE/GRAPH/
  IMPACT/QUALITY/SAFE CHANGE/CODE IO/INFRASTRUCTURE with at least
  one anchor tool per domain.
- A "Read vs mutate" table classifying each domain as
  READ/ANALYZE or MUTATE.
- Four canonical flows (PROJECT DISCOVERY, ARCHITECTURE DEEP DIVE,
  CHANGE IMPACT, SAFE REFACTOR) with **actual runtime parameter
  names** (`symbol_name`, `source`, `target`, `action`, `file_path`,
  `function_name`, `file_paths`).
- An "Architecture warnings" section stating explicitly that
  `check_architecture` is Tarjan SCC, NOT e77 executable
  architecture.
- A "Prerequisites" section noting the `build_graph` → most
  graph tools dependency, and the workaround for the graph
  persistence quirk.

**Validation:** `python3 scripts/validate_skills.py --skill cognicode-mcp`
returns `PASS` and reports ≥40 validated MCP tool references.

## REQ-84.1-03 — `cognicode-developer` skill

A `skills/cognicode-developer/SKILL.md` MUST exist with:

- Frontmatter declaring `name: cognicode-developer`, a non-empty
  description, and `license: MIT`.
- A "Required setup" section listing Rust, `just`, and a repo clone.
- A "Required reading" section pointing at `docs/adr/README.md`,
  `openspec/`, and `AGENTS.md`.
- A "Required process" section describing the SDDK + OpenSpec
  cycle.
- A "Key development commands" section listing `just build`,
  `just test-unit`, `just check-known-failures`, `just lint`,
  `just dev`, `just run`, `just start`.
- A clear statement that this skill is **NOT** for users of
  CogniCode.

**Validation:** the validator allows internal-only patterns
(`docs/adr/`, `openspec/`, `just` recipes) inside this skill only.

## REQ-84.1-04 — `cognicode-recommended` SET

A `skills/cognicode-recommended/SET.yaml` MUST exist with:

- `apiVersion: cognicode/v1`, `kind: SkillSet`.
- `name: cognicode-recommended`.
- `includes` listing the two user skills.
- `excludes` listing `cognicode-developer`.

**Validation:** the validator treats `SET.yaml` as a SkillSet,
not a SkillBundle.

## REQ-84.1-05 — User skills do NOT advertise internal capabilities

The user skills (`cognicode`, `cognicode-mcp`) MUST NOT teach or
mention:

- e77 executable architecture as if it were a public MCP tool.
- LSI Facts/Evidence / Intelligence Event Log / SoftwareWorld /
  Trial / historical replay / shadow comparison as if they were
  public.
- Governed promotion authority (e80/e83) as if it existed at the
  MCP layer.
- AI SemanticMiner / FindingCritic / FixAgent as if they were
  public MCP tools.

**Validation:** the skills DO NOT contain strings like
"e77 executable architecture" in product-fiction contexts, and
they explicitly caveat `check_architecture` as "Tarjan SCC, NOT
e77" (REQ-84.1-06).

## REQ-84.1-06 — `check_architecture` is explicitly disambiguated

`skills/cognicode-mcp/SKILL.md` MUST contain a sentence of the
form:

```text
check_architecture is a Tarjan SCC cycle detector over the
project call graph. ... It is NOT the e77 executable-architecture
subsystem; there is no public MCP adapter for that yet.
```

**Validation:** the validator does not enforce this content
directly; it is enforced by code review on the skill body.

## REQ-84.1-07 — Skills contain no machine-specific paths

User skills (`cognicode`, `cognicode-mcp`) MUST NOT contain any of:

- `/var/home/rubentxu`
- `/home/rubentxu/Proyectos`
- `~/Proyectos/rust/CogniCode`
- `target/debug/` or `target/release/`
- `cd /home/`, `cd /Users/`, `cd /var/`, `cd /opt/`

**Validation:** enforced by `scripts/validate_skills.py`.

## REQ-84.1-08 — Skills contain no internal-only doc references

User skills MUST NOT reference:

- `docs/ROADMAP.md`
- `openspec/changes/` or `openspec/CHANGES.md`
- `docs/adr/`

**Validation:** enforced by `scripts/validate_skills.py`.

## REQ-84.1-09 — Skills contain no internal `just` recipes

User skills MUST NOT reference `just` recipes from the CogniCode
repo: `post-e31-audit`, `ci-t6`, `scorecard-nightly`,
`scorecard-streak`.

**Validation:** enforced by `scripts/validate_skills.py`.

## REQ-84.1-10 — Skills are version-independent

User skills MUST NOT contain hard-coded versions like `0.92.0`,
`0.95.0`, or internal milestone names (`e31`, `e63`, etc.).
Skill-version (frontmatter `metadata.version`) and runtime-version
constraint (`manifest.yaml compatibility.cognicode_runtime`) are
decoupled.

**Validation:** enforced by `scripts/validate_skills.py`.

## REQ-84.1-11 — Skills reference only real MCP tools

If a user skill references an MCP tool name in backticks
(e.g. `` `project_overview` ``), that tool MUST exist in the
runtime catalog (`tools/list`). Tokens that are obviously
parameter names (positioned right after `(` or `,`) are exempt.

**Validation:** enforced by `scripts/validate_skills.py`.

## REQ-84.1-12 — Old skills are archived

The directories `skills/cognicode-core/` and
`skills/cognicode-mcp-driven/` MUST be removed from `skills/`
and moved to
`openspec/changes/e84-1-cognicode-skill-productization/artifacts/old-skills/`.

**Validation:** `ls skills/` shows only `cognicode`,
`cognicode-mcp`, `cognicode-developer`, `cognicode-recommended`.

## REQ-84.1-13 — Validator is runnable as a CI gate

`scripts/validate_skills.py` MUST be runnable in CI without
network access (catalog is loaded from a local JSON file under
`artifacts/`). It MUST exit non-zero on any validation failure.

**Validation:** running the script with no arguments exits 0 on
the current `skills/` state and prints `PASS: all N skill(s)
validated`.

## REQ-84.1-14 — `SkillSource` identity model is documented

A `SkillSource` metadata model MUST be documented with fields
`repository`, `git_ref`, `path`, `content_digest`. Resolution modes
(`latest`, `pinned-tag`, `pinned-commit`, `offline-cached`) MUST be
specified.

**Validation:** the model is documented at
`artifacts/wu14-skillsource-model.md`. No implementation in this
cycle.

## REQ-84.1-15 — Skills.sh compatibility

Every public skill MUST be valid as a normal Agent Skill. The
`SKILL.md` frontmatter MUST be parseable by the skills.sh
ecosystem. `manifest.yaml` is OPTIONAL from skills.sh's
perspective; the skill works without it.

**Validation:** frontmatter conforms to standard Agent Skills
schema (`name`, `description`, `license` required).

## REQ-84.1-16 — Skills are NOT packaged in e85

The new skills MUST live in the git source at `skills/`. They
MUST NOT be bundled into the binary release tarball. e85 ships
binaries only; skills are fetched from git by `cogh` (future) or
by an agent's skills loader.

**Validation:** `find target -name SKILL.md` returns nothing
(release binaries do not embed skills).

## REQ-84.1-17 — Cycle exit

The cycle MUST close when:

- All artifacts are written.
- All skills pass `scripts/validate_skills.py`.
- `cargo test -p cognicode-cli --bin cogh` is unchanged (178/178
  + 2 ignored).
- `cargo check --workspace` exits 0.
- `just check-known-failures` shows 41-entry baseline intact.
- A verification report (`verification-report.md`) is written.
- The change is moved to `openspec/changes/archive/`.

The cycle MUST NOT auto-open e85.

**Validation:** archive directory contains the change; main
branch HEAD == origin/main; exit report in the archive.
