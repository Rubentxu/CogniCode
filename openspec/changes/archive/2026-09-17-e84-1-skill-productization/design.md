# e84.1 — Design

## Skill taxonomy (USER vs DEVELOPER)

The cycle splits skills into two identity groups that must NEVER
be mixed (see `wu5-skill-taxonomy.md`):

| Identity | Audience | Allowed content |
|----------|----------|-----------------|
| User skills | End users with `cogh`-installed CogniCode | No machine paths, no `target/debug/`, no internal docs, no `just` recipes, no capability claims for non-public subsystems. |
| Developer skills | CogniCode contributors with a source checkout | Internal docs, `just` recipes, repo-relative paths. |

## Skill set

```text
skills/
  cognicode/                  # entry-point user skill
    SKILL.md
    manifest.yaml
  cognicode-mcp/              # workflow user skill (the 73-tool surface)
    SKILL.md
    manifest.yaml
  cognicode-developer/        # developer-only
    SKILL.md
    manifest.yaml
  cognicode-recommended/      # SkillSet (SET.yaml, not SKILL.md)
    SET.yaml
```

The old `cognicode-core` and `cognicode-mcp-driven` directories
are archived under `artifacts/old-skills/`.

## Skill content design

`skills/cognicode/SKILL.md`:

- Step 1: install verification (`cogh version`, `cogh doctor`,
  `cognicode doctor`).
- Step 2: surface choice (CLI vs MCP vs Explorer).
- Step 3: first discovery (CLI commands + MCP `project_overview`,
  `project_insights`, `codebase_map`).
- Step 4: locate code (CLI `index query`, MCP `query_symbol_index`).
- Step 5: when to load `cognicode-mcp` for deeper workflows.

`skills/cognicode-mcp/SKILL.md`:

- Capability domains (DISCOVER/NAVIGATE/GRAPH/IMPACT/QUALITY/
  SAFE CHANGE/CODE IO/INFRASTRUCTURE).
- Read vs mutate UX guardrail.
- Four canonical flows: PROJECT DISCOVERY, ARCHITECTURE DEEP
  DIVE, CHANGE IMPACT, SAFE REFACTOR — with actual parameter
  names from the runtime schemas.
- Prerequisites (`build_graph` first; `check_architecture`
  workaround for the persistence bug).
- Architecture warnings (Tarjan SCC, NOT e77).
- Error recovery.
- CLI ↔ MCP ↔ Explorer routing.

`skills/cognicode-developer/SKILL.md`:

- Required setup (Rust, just, repo clone).
- Required reading (ADRs, openspec, AGENTS.md).
- Required process (SDDK + OpenSpec cycle).
- Key development commands (`just build`, `just test-unit`, etc.).
- Repository conventions.
- Distribution lifecycle (e84/e85/e86 reference).

## SkillSource identity model

See `wu14-skillsource-model.md`. Implemented as documentation
only. No `cogh skill install` wiring in this cycle.

## Validation

`scripts/validate_skills.py`:

- Rejects `/var/home/rubentxu`, `target/debug/`, `target/release/`,
  `cd /home/`, `cd /Users/`, `cd /var/`, `cd /opt/`.
- Rejects references to internal-only docs (`docs/ROADMAP.md`,
  `openspec/changes/`, `docs/adr/`) in non-developer skills.
- Rejects internal `just` recipes (`post-e31-audit`, `ci-t6`,
  `scorecard-nightly`, `scorecard-streak`).
- Rejects stale versions (`0.92.x`).
- Requires frontmatter `name`, `description`, `license`.
- Validates tool references against the runtime catalog.
- Validates `manifest.yaml` / `SET.yaml` YAML syntax.
- Treats SkillSet (SET.yaml) and SkillBundle (manifest.yaml)
  differently.

## Exit gates

- `cargo test -p cognicode-cli --bin cogh` — unchanged.
- `cargo check --workspace` — unchanged.
- `just check-known-failures` — unchanged.
- `python3 scripts/validate_skills.py` — PASS.
- `scripts/verify-skills.sh` — PASS.

## Out of scope

- `cogh skill install` (e86).
- e77 executable architecture adapter.
- Control Plane (CP0).
- Linux Release Factory (e85) — receives the skills as a
  decoupled artifact (WU20).
