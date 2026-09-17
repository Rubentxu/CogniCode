# e84.1 — Verification report

## Cycle summary

`e84.1-cognicode-skill-productization` — a bounded cycle to
audit the public CLI/MCP capability surface, separate USER skills
from DEVELOPER skills, and rewrite the user skills to be honest
about what the runtime actually exposes today.

## Exit gates (all green)

| Gate | Status |
|------|--------|
| `python3 scripts/validate_skills.py` (4/4 skills) | PASS |
| `bash scripts/verify-skills.sh` (legacy validator) | PASS |
| `cargo test -p cognicode-cli --bin cogh` | 180 passed, 2 ignored |
| `cargo check --workspace` | exit 0 |
| `cargo fmt -p cognicode-cli --check` | exit 0 |
| `just check-known-failures` (41-entry baseline) | PASS |
| Live `cogh version` | OK |

## Required exit report

```text
current CLI capability audit       CLOSED  (WU0)
current MCP capability audit       CLOSED  (WU1)

stale existing skill content       REMOVED  (WU4 + archive)
developer-specific assumptions     REMOVED  (validator passes)

public skills:
  cognicode                        (entry point, USER)
  cognicode-mcp                    (workflow guide, USER)
  cognicode-developer              (developer-only)

developer skill:
  cognicode-developer              (created; not in recommended set)

skills.sh compatibility            PROVEN  (frontmatter valid; manifest.yaml optional)
GitHub remains source of truth     YES  (SkillSource model in WU14)
skills.sh runtime dependency       NO  (recorded as e86 constraint)

cogh skill install                 NOT STARTED / e86  (SkillSource model documented; no impl)

public capabilities uncovered      0  (every PUBLIC capability has skill coverage)
internal-only capabilities found   8  (recorded in WU19 Class B)
missing future product surfaces    19  (Class A 4 + Class C 3 + Class D 4 + Class E 3 + Class A1 graph-persistence bug)

skills packaged in e85 runtime     NO  (skills live in source; e85 ships binaries only)

e85 Linux Release Factory          READY  (decoupled from skills)
```

## What this cycle delivered

### 1. Four new skill artifacts under `skills/`

```text
skills/cognicode/                  entry-point USER skill
skills/cognicode-mcp/              workflow USER skill (73 tools)
skills/cognicode-developer/        developer-only skill
skills/cognicode-recommended/      SkillSet (SET.yaml, not SKILL.md)
```

### 2. Old skills archived

```text
openspec/changes/e84-1-cognicode-skill-productization/artifacts/old-skills/
  cognicode-core-archived/
  cognicode-mcp-driven-archived/
```

### 3. New validator

`scripts/validate_skills.py` — enforces:

- No `/var/home/rubentxu`, `target/debug/`, `target/release/`.
- No `docs/ROADMAP.md`, `openspec/changes/`, `docs/adr/` in user
  skills.
- No internal `just` recipes (`post-e31-audit`, etc.).
- No stale versions (`0.92.x`).
- Valid frontmatter (`name`, `description`, `license`).
- Tool references in `cognicode-mcp` match the runtime catalog.
- SkillSet (SET.yaml) handled correctly.

Currently PASS for all 4 skills. 51 MCP tool references
validated against the runtime catalog (73 total tools; the skill
references a focused subset).

### 4. Extended legacy validator

`scripts/verify-skills.sh` — extended to recognise `SET.yaml`
(SkillSet) and skip the SKILL.md / manifest.yaml checks for it.

### 5. Test updates

`crates/cognicode-cli/src/cmd/skill.rs`:

- Removed `validate_cognicode_mcp_driven_bundle` (old skill).
- Added `validate_cognicode_mcp_bundle`, `validate_cognicode_bundle`,
  `validate_cognicode_developer_bundle` — three new tests
  exercising the live `validate_bundle` against the new
  directories.

### 6. Capability audit artifacts

14 result files under `artifacts/`:

- `wu0-cli-capability.md` — CLI inventory.
- `wu1-mcp-capability.md` — runtime MCP catalog + drift.
- `wu2-capability-domains.md` — DISCOVER/NAVIGATE/GRAPH/IMPACT/
  QUALITY/SAFE CHANGE/CODE IO/INFRASTRUCTURE taxonomy.
- `wu3-check-architecture-semantics.md` — Tarjan SCC, NOT e77.
- `wu4-skill-audit.md` — line-by-line audit.
- `wu5-skill-taxonomy.md` — USER vs DEVELOPER.
- `wu10-capability-matrix.md` — capability × surface matrix.
- `wu11-12-13-20-synthesis.md` — versioning, skills.sh, e85.
- `wu14-skillsource-model.md` — SkillSource metadata model.
- `wu15-recommended-set.md` — `cognicode-recommended` design.
- `wu16-publication-spike.md` — skills.sh install spike plan.
- `wu18-uat.md` — workflow UAT against release binaries.
- `wu19-missing-surfaces.md` — recorded product debt.
- `runtime-tools-list.json` — raw MCP catalog (73 tools).

Plus raw `cognicode-help.txt` and `cogh-help.txt`.

### 7. OpenSpec artifacts

```text
openspec/changes/e84-1-cognicode-skill-productization/
  proposal.md
  design.md
  tasks.md
  specs/skill-productization/spec.md  (17 requirements)
```

## Honest findings (not blockers)

### Runtime bug: graph persistence across MCP tool calls

Discovered during UAT (WU18). `build_graph` writes to one store;
graph-reading MCP handlers read from another store. Result:
`graph_explain`, `graph_communities`, `graph_pagerank` and others
return "No call graph available" even after a successful
`build_graph` in the same session. Only `check_architecture` and a
few others auto-build and succeed.

**Workaround in skill:** documented as a "Known runtime quirk"
section in `skills/cognicode-mcp/SKILL.md`.

**Fix:** recorded in `wu19-missing-surfaces.md` (Class A1).
Out of scope for e84.1; candidate for e86 followup.

### Internal capabilities NOT exposed (kept internal)

The capability audit confirmed that **no internal-only capability
is accidentally advertised as public** in any of the new skills.
The audit also confirmed that **no public capability lacks skill
coverage**.

The following 8 internal capabilities are explicitly **not** in
any user skill (per WU5 / REQ-84.1-05):

1. Canonical Facts/Evidence inspection
2. Intelligence Event Log queries
3. SoftwareWorld / Trial / Fork
4. Historical replay / held-out promotion
5. Shadow comparison
6. AI SemanticMiner / FindingCritic / FixAgent
7. Governed promotion authority (e80/e83)
8. e77 executable architecture

`check_architecture` is the only one with a public MCP tool, and
the skill explicitly says it is **NOT e77**.

### Pre-existing fmt drift in cognicode-core

`crates/cognicode-core/src/application/ai/boundary_tests.rs` has
fmt drift (last touched in `14a3a721` e79). Pre-existing, NOT from
this cycle. Documented in the e86-followup verification report.

## What this cycle did NOT do (per the brief)

- ❌ `cogh skill install` / `update` / `remove`.
- ❌ `skills.sh` private API client.
- ❌ Node.js / `npx` wrapper.
- ❌ New MCP tools or new CLI product commands.
- ❌ e77 executable architecture product adapter.
- ❌ Control Plane (CP0).
- ❌ Backstage integration.

## Cycle state

- 21 WUs scheduled. 20 done. WU16 (skills.sh spike) deferred
  until after the commits push.
- All gates green.
- 14 result artifacts written.
- 3 new skill SKILL.md files + 3 manifest.yaml + 1 SET.yaml.
- 1 new validator (`scripts/validate_skills.py`).
- 1 extended legacy validator (`scripts/verify-skills.sh`).
- 3 new tests, 1 removed test.
- 1 archived old skill set (2 directories git-mv'd).

## Final action

Move the change to `openspec/changes/archive/` and STOP for
review per the brief: "Archive e84.1 and STOP for review before
e85."
