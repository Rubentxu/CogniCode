# e84.1 — CogniCode skill productization

## Why this cycle

After e86 closed, the user-facing CogniCode skill bundles in
`skills/` were stale in three significant ways:

1. **`cognicode-core` was a developer-skill mis-published as the
   public entry point.** It pointed at Rubentxu's local
   `/var/home/rubentxu/Proyectos/rust/CogniCode`, the internal
   `docs/ROADMAP.md`, and the `post-e31-audit` `just` recipe.
2. **`cognicode-mcp-driven` claimed 68 tools** when the runtime
   now exposes 73; pointed at `target/debug/cognicode-mcp`; and
   referenced `docs/adr/` and `docs/TEST-PLAN.md` — internal docs
   that a user does not have.
3. **The runtime tool surface was documented from memory, not
   `tools/list`.** The previous `docs/MCP-TOOLS.md` is dated
   2026-08-06 and is 5 tools behind.

Worse, the skills taught **what existed internally** rather than
**what a user can actually do today through the public CLI/MCP
surface**. The LSI roadmap's e63–e83 capabilities are powerful
but mostly not yet exposed; the skills should not promise them.

This cycle is a **bounded productization** of the skills. It does
not implement `cogh skill install`, does not build the e77
executable-architecture adapter, and does not start CP0.

## What this cycle delivers

1. **Three new skills** under `skills/`:
   - `cognicode` — entry point, install verification, surface choice.
   - `cognicode-mcp` — workflow guide for the 73-tool MCP server.
   - `cognicode-developer` — onboarding for CogniCode contributors
     (replaces the developer-as-user content of the old
     `cognicode-core`).
2. **One curated set**:
   - `cognicode-recommended` — `SET.yaml` listing `cognicode` and
     `cognicode-mcp`.
3. **Capability audit artifacts** (under
   `openspec/changes/e84-1-cognicode-skill-productization/artifacts/`):
   - `wu0-cli-capability.md` — CLI inventory with REAL/PARTIAL/
     STUB/FEATURE-GATED/INTERNAL status.
   - `wu1-mcp-capability.md` — runtime `tools/list` catalog +
     drift report.
   - `wu2-capability-domains.md` — user-oriented taxonomy
     (DISCOVER/NAVIGATE/GRAPH/IMPACT/QUALITY/SAFE CHANGE/CODE IO/
     INFRASTRUCTURE).
   - `wu3-check-architecture-semantics.md` — explicit
     "Tarjan SCC, NOT e77" caveat.
   - `wu4-skill-audit.md` — line-by-line audit of the old skills.
   - `wu5-skill-taxonomy.md` — USER vs DEVELOPER split decision.
   - `wu10-capability-matrix.md` — capability × surface matrix.
   - `wu14-skillsource-model.md` — SkillSource metadata model.
   - `wu15-recommended-set.md` — `cognicode-recommended` design.
   - `wu16-publication-spike.md` — skills.sh install spike plan.
   - `wu18-uat.md` — workflow UAT against release binaries.
   - `wu19-missing-surfaces.md` — recorded product debt
     (graph persistence bug, e77 not exposed, etc.).
   - `wu11-12-13-20-synthesis.md` — versioning, skills.sh, e85
     relationships.
4. **One validation tool**:
   - `scripts/validate_skills.py` — enforces: no machine paths,
     no `target/debug/` paths, no internal docs in user skills,
     no stale versions, valid frontmatter, tool references match
     the runtime catalog.
5. **Archived old skills**:
   - `openspec/changes/e84-1-cognicode-skill-productization/artifacts/old-skills/cognicode-core-archived/`
   - `openspec/changes/e84-1-cognicode-skill-productization/artifacts/old-skills/cognicode-mcp-driven-archived/`

## What this cycle does NOT deliver

- `cogh skill install` / `cogh skill update` / `cogh skill remove`.
- `skills.sh` private API client.
- Node.js / `npx` wrapper.
- New MCP tools or new CLI product commands.
- e77 architecture product adapter.
- Control Plane (CP0).
- Backstage integration.

These are deferred to future cycles (e86 followup, e87, etc.).
Missing surfaces recorded in `wu19-missing-surfaces.md`.

## Verification

- `python3 scripts/validate_skills.py` — PASS (4/4 skills).
- `scripts/verify-skills.sh` — PASS (existing validator).
- Workflow UAT against release binaries (WU18): PROJECT
  DISCOVERY works; ARCHITECTURE DEEP DIVE partially works (graph
  persistence bug recorded in WU19); CHANGE IMPACT partially
  works.
- Runtime MCP catalog captured (73 tools).

## Exit gates

- `cargo test -p cognicode-cli --bin cogh` — 178/178 + 2 ignored
  (unchanged from e86 followup).
- `cargo check --workspace` — exit 0.
- `just check-known-failures` — 41-entry baseline intact.
- `python3 scripts/validate_skills.py` — PASS.
- Skills.sh / GitHub install spike — deferred (WU16); will be
  run after the cycle's commits push.

## Status

Cycle in progress. WUs 0–5, 10–20, 17 complete. WU16 deferred
until after push. WU19 + WU3 captured real product debt (graph
persistence bug; e77 NOT exposed) — these are honest findings,
not blockers.
