# e84.1 — Tasks

21 WUs. Each WU has a result file under
`openspec/changes/e84-1-cognicode-skill-productization/artifacts/`.

## WU0 — CLI capability truth — DONE

Inventory of every `cognicode` and `cogh` subcommand with REAL/
PARTIAL/STUB/FEATURE-GATED/INTERNAL status. Raw `cognicode --help`
and `cogh --help` transcripts captured.

Result: `artifacts/wu0-cli-capability.md`.

## WU1 — MCP capability truth — DONE

Captured runtime `tools/list` (73 tools) and compared to
`docs/MCP-TOOLS.md` (68 tools, 2026-08-06). Drift report
included.

Result: `artifacts/wu1-mcp-capability.md`,
`artifacts/runtime-tools-list.json`.

## WU2 — Capability domains — DONE

User-oriented taxonomy (DISCOVER/NAVIGATE/GRAPH/IMPACT/QUALITY/
SAFE CHANGE/CODE IO/INFRASTRUCTURE). Recommended starting flow
per domain.

Result: `artifacts/wu2-capability-domains.md`.

## WU3 — Architecture semantics — DONE

`check_architecture` is Tarjan SCC cycle detector over the call
graph; NOT e77 executable architecture. Explicit warning in skill.

Result: `artifacts/wu3-check-architecture-semantics.md`.

## WU4 — Audit existing skills — DONE

Line-by-line classification of the two old skills
(`cognicode-core`, `cognicode-mcp-driven`). Stale version (0.92.0),
local-machine paths, internal docs, internal `just` recipes, 68
vs 73 tool count.

Result: `artifacts/wu4-skill-audit.md`.

## WU5 — USER vs DEVELOPER skill taxonomy — DONE

Decision: split into three user skills (entry point, MCP,
developer) and one SET (`cognicode-recommended`). Deferred
narrower skills (`cognicode-architecture`, etc.) to future cycles.

Result: `artifacts/wu5-skill-taxonomy.md`.

## WU6 — Skill pedagogy (decision-making, not tool dump) — DONE

`skills/cognicode/SKILL.md` and `skills/cognicode-mcp/SKILL.md`
are organized by user intent and capability domain, not by tool
enumeration.

## WU7 — Tool composition — DONE

Four canonical flows written into `skills/cognicode-mcp/SKILL.md`:
PROJECT DISCOVERY, ARCHITECTURE DEEP DIVE, CHANGE IMPACT,
SAFE REFACTOR. Each flow uses actual runtime parameter names
(post-WU18 UAT correction).

## WU8 — Read/write safety — DONE

Read/mutate table in `skills/cognicode-mcp/SKILL.md`. Default to
READ. SAFE CHANGE flow is the only path that mutates through MCP.

## WU9 — CLI vs MCP vs Explorer — DONE

Routing section in `skills/cognicode/SKILL.md` and `skills/cognicode-mcp/SKILL.md`.

## WU10 — Capability coverage matrix — DONE

Result: `artifacts/wu10-capability-matrix.md`. Confirmed:
**no PUBLIC capability lacks skill coverage**, **no INTERNAL
capability is advertised as public**.

## WU11 — Version-independent wording — DONE

Skill versions decoupled from binary versions. Validator rejects
`0.92.x`. Skill body uses no milestone names.

Result: `artifacts/wu11-12-13-20-synthesis.md`.

## WU12 — skills.sh compatibility — DONE

Frontmatter valid, no `manifest.yaml` required by skills.sh.
`manifest.yaml` is the CogniCode extension.

Result: `artifacts/wu11-12-13-20-synthesis.md`.

## WU13 — Canonical source + skills.sh relationship — DONE

GitHub = source of truth. skills.sh = discovery. `cogh` = official
install (future). No runtime dependency on skills.sh.

Result: `artifacts/wu11-12-13-20-synthesis.md`.

## WU14 — SkillSource identity model — DONE

Documented as metadata only. No implementation.

Result: `artifacts/wu14-skillsource-model.md`.

## WU15 — `cognicode-recommended` SET — DONE

`skills/cognicode-recommended/SET.yaml` with `kind: SkillSet`,
`includes: [cognicode, cognicode-mcp]`, `excludes:
[cognicode-developer]`.

Result: `artifacts/wu15-recommended-set.md`.

## WU16 — skills.sh publication spike — DEFERRED

Requires commit + push. Test plan documented.

Result: `artifacts/wu16-publication-spike.md`. Run after push.

## WU17 — Skill static validation — DONE

`scripts/validate_skills.py`:
- Rejects forbidden patterns in user skills.
- Requires valid frontmatter.
- Validates tool references against runtime catalog.
- Treats SkillSet vs SkillBundle correctly.

Currently PASS for all 4 skills. 51 MCP tool refs validated
against the runtime catalog.

## WU18 — Real workflow UAT — DONE

UAT against release binaries (`cognicode-mcp`, `mcp-client`)
against a fixture Rust repo. PROJECT DISCOVERY works; ARCHITECTURE
DEEP DIVE partially works (graph persistence bug); CHANGE IMPACT
partially works. Parameter-name drift discovered and corrected in
the skill body.

Result: `artifacts/wu18-uat.md`.

## WU19 — Document missing product surfaces — DONE

Class A: 1 runtime bug (graph persistence across MCP calls) + 3
pinned e86 followup bugs.
Class B: 8 internal-only capabilities not exposed.
Class C: 3 mis-described CLI help entries.
Class D: 4 thin-coverage areas.
Class E: 3 discovery/publishing gaps.

Result: `artifacts/wu19-missing-surfaces.md`.

## WU20 — Relationship to e85 — DONE

Skills live in source (git), not in binary tarball. e85 ships
binaries; skills fetched from git by `cogh` (future) or agent's
skills loader. No coupling.

Result: `artifacts/wu11-12-13-20-synthesis.md`.

## Summary

20 of 21 WUs done. WU16 deferred to after push.

Cycle commits:

- New skills (`skills/cognicode/`, `skills/cognicode-mcp/`,
  `skills/cognicode-developer/`, `skills/cognicode-recommended/`).
- Archived old skills.
- New validator (`scripts/validate_skills.py`).
- 14 artifact files (WU results).
- 4 openspec change files (proposal, design, tasks, specs/).
