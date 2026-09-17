# WU5 — Skill taxonomy (USER vs DEVELOPER)

## Core principle

> A CogniCode skill must describe what a user/agent can actually do
> TODAY through a PUBLIC CLI/MCP surface, not everything that exists
> internally in cognicode-core.

Therefore, this cycle splits the skill set into two identity groups
that must NEVER be mixed:

1. **User skills** — installed by an end user who has `cogh`-installed
   CogniCode on their machine. The user does NOT have a CogniCode
   source checkout.
2. **Developer skills** — used by contributors to the CogniCode
   project itself. Requires a source checkout.

The two groups have different audiences and different rules about
content (see "rules" below).

## Recommended user-skill set (initial)

A small set with distinct jobs. Avoid ten tiny skills.

### `cognicode` (or `cognicode-core`)

**Purpose:** general entry point.

**Job:** "I just installed CogniCode and want to discover what this
project is."

- Discover the project (`project_overview`, `project_insights`,
  `codebase_map`).
- Locate relevant code (`query_symbol_index`, `get_symbol_code`).
- Pick the right surface (CLI vs MCP) for the next step.
- Drive a small CLI workflow (`cognicode analyze`, `cognicode
  doctor`).

This skill is small (the entry point). It refers to `cognicode-mcp`
for deeper MCP work.

### `cognicode-mcp`

**Purpose:** teach agents how to use the CogniCode MCP effectively.

**Job:** the full 73-tool surface, organized by capability domain
(WU2). Teaches tool composition (WU7), read/mutate safety (WU8),
and CLI vs MCP routing (WU9).

This is the largest user skill. It is the only one that references
the runtime MCP catalog.

### `cognicode-architecture`

**Purpose:** deep graph/architecture workflows.

**Job:** `build_graph` → `graph_insights` → `graph_communities` →
`graph_explain` → `check_architecture` (with the explicit
NOT-e77 caveat from WU3).

Optional narrow skill. Only include if it provides distinct value
over `cognicode-mcp`.

### `cognicode-impact-analysis`

**Purpose:** change-impact workflows.

**Job:** `analyze_impact` → `get_call_hierarchy` / `trace_path` →
`review_pr`.

Optional narrow skill.

### `cognicode-safe-refactor`

**Purpose:** safe-change workflows.

**Job:** `analyze_impact` → `generate_contract` → `safe_refactor` →
`validate_contract` → `reparse_on_edit`.

Optional narrow skill.

## Recommended developer-skill set

### `cognicode-developer`

**Purpose:** onboarding for CogniCode contributors.

**Job:** the current `cognicode-core/SKILL.md` content, but
**rewritten** to:

- Read the CogniCode repo (no `cd /var/home/rubentxu/...`).
- Use `just` recipes (the user is a developer; they have the repo).
- Reference internal docs that DO exist in the public repo
  (`docs/adr/`, `openspec/`, `Cargo.toml`).
- Not pretend to be a user-skill.

This skill is **not** part of the `cognicode-recommended` bundle
(WU15). It is published separately, with a different name, and is
intended only for people working on CogniCode itself.

## Rules

### User skills MUST NOT contain

- Hard-coded local paths (`/var/home/rubentxu/...`,
  `~/Proyectos/rust/CogniCode`).
- `target/debug/...` or `target/release/...` paths.
- References to internal-only docs that don't ship in the binary
  release (`docs/ROADMAP.md`, `openspec/`, `docs/adr/`).
- `just` recipes from the CogniCode repo (`post-e31-audit`,
  `ci-t6`, `scorecard-nightly`, etc.).
- Capability claims for features that don't exist in the public MCP
  (e.g. e77 executable architecture).

These are mechanically enforced in WU17 (skill static validation).

### User skills SHOULD

- Reference the runtime MCP catalog for tool details, not enumerate
  every tool.
- Teach decision-making (intent → tool family → small set of tools).
- Distinguish read/analyze from mutate (WU8).
- Behave correctly against a fresh `cogh install mcp-server` of
  v0.95.0, regardless of where the user lives.

### Developer skills

- Allowed to reference internal docs and `just` recipes.
- Allowed to assume a CogniCode source checkout.
- Should NOT be in the `cognicode-recommended` bundle.

## Final taxonomy decision for this cycle

After auditing the surface (WU1, WU2, WU4), the minimum set that
provides distinct value is:

```text
cognicode               # entry point, discover + navigate
cognicode-mcp           # full MCP teaching
cognicode-developer     # dev-only, separate identity
```

We **defer** the narrower skills (`cognicode-architecture`,
`cognicode-impact-analysis`, `cognicode-safe-refactor`) to a future
cycle. Reasoning: with 73 tools and 8 capability domains, splitting
narrow skills at this point creates more cognitive overhead than it
removes. The `cognicode-mcp` skill will teach the workflows
inline; if a workflow is so distinct that it warrants its own skill,
that will surface through future UAT and audit.

## Naming

- Keep `cognicode-mcp` as the existing skill name (it's referenced
  by the existing `cognicode-core` skill and is recognizable).
- Keep `cognicode` as the new entry-point skill (short and
  recognisable; replaces `cognicode-core` for users).
- New: `cognicode-developer` (replaces the developer-as-user
  content of the old `cognicode-core`).

The old `cognicode-core` directory will be **removed** (git mv to
archive or delete) — its content moves to `cognicode-developer`
with the local-paths and `just`-recipe commands preserved (because
they are correct for a developer audience).
