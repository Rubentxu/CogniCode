# WU4 — Audit of existing skills

Two skills exist today in `skills/`:

```text
skills/cognicode-core/
  SKILL.md       33 lines
  manifest.yaml  21 lines

skills/cognicode-mcp-driven/
  SKILL.md       97 lines
  manifest.yaml  29 lines
  references/
    bootstrap.sh  (placeholder echo)
    invoke.sh     (placeholder echo)
  assets/
    metadata.json
```

Both are **stale** in significant ways. The audit below classifies
each instruction.

## `skills/cognicode-core/SKILL.md` — this is a developer skill, mis-labelled as the public entry

| Line(s) | Content | Classification |
|---------|---------|----------------|
| 3–7 | "Core CogniCode workflow — analyze, build, test, deploy. Trigger: When a task involves understanding or modifying the **CogniCode codebase** (graph store, MCP server, Explorer UI)." | **DEV-REPO-SPECIFIC.** This is a skill for *developing CogniCode*, not for *using* CogniCode on a user project. |
| 10 | `version: "0.92.0"` | **STALE VERSION.** CogniCode is at 0.95.0 today (HEAD `46ca3c48`). |
| 24 | `cd /var/home/rubentxu/Proyectos/rust/CogniCode` | **LOCAL-MACHINE-SPECIFIC.** Hard-coded Rubentxu's home path. A user who installs CogniCode has no such path. **MUST be removed.** |
| 25 | `cat docs/ROADMAP.md | head -30` | **DEV-REPO-SPECIFIC.** `ROADMAP.md` is an internal working document. Per `AGENTS.md`, it is local-only and never pushed to remote. A user has no `ROADMAP.md`. |
| 26 | `just post-e31-audit` | **DEV-REPO-SPECIFIC + STALE COMMAND.** `post-e31-audit` is a CogniCode internal `just` recipe (e31 milestone). A user has no `just` and no `post-e31-audit`. |
| 27 | `ls docs/adr/` | **DEV-REPO-SPECIFIC.** Same reason as `ROADMAP.md`. |
| 32 | `cognicode-sandbox` and `cognicode-uat` layered skills | **NEVER EXISTED in `skills/`.** These are not present in the repository today. |

**Verdict:** This skill is a developer onboarding skill for the
CogniCode repo. It must be **renamed / re-purposed** to
`cognicode-developer` and **rewritten** to remove Rubentxu's local
paths, internal-roadmap references, and `just`-recipe commands.

## `skills/cognicode-mcp-driven/SKILL.md`

| Line(s) | Content | Classification |
|---------|---------|----------------|
| 4 | "Drive CogniCode engineering workflows via the 68-tool MCP server." | **STALE CAPABILITY CLAIM.** Today: 73 tools (WU1). |
| 19 | "Drive CogniCode engineering workflows through the 68-tool MCP server." | Same. |
| 30–31 | `cd /var/home/rubentxu/Proyectos/rust/CogniCode` | **LOCAL-MACHINE-SPECIFIC.** Same as above. **MUST be removed.** |
| 36 | `target/debug/cognicode-mcp --cwd .` | **DEV-REPO-SPECIFIC + STALE PATH.** A user has installed binaries (via `cogh`), not `target/debug` paths. **MUST be removed.** |
| 41–43 | `target/debug/cognicode-mcp --cwd . <<EOF {"jsonrpc":"2.0","id":1,"method":"tools/list"} EOF` | Same. |
| 48 | `docs/TEST-PLAN.md` | **DEV-REPO-SPECIFIC.** A user has no CogniCode repo checkout. |
| 54 | "Layout: `SKILL.md` (body) + `manifest.yaml` (cogh metadata) + `references/` (scripts) + `assets/` (data files)." | **PARTIAL.** This is the CogniCode extension layout, not the Agent Skills ecosystem contract. A skills.sh consumer doesn't know about `manifest.yaml`. |
| 56 | "`metadata.version` mandatory" | **STALE.** Should not be tied to binary version. |
| 61–72 | "Common tools to reach for" list | **PARTIAL + MISSING.** Lists 12 tools; missing many of the modern surface (graph_insights, project_overview, project_insights, codebase_map, smart_search, graph_communities, graph_explain, safe_refactor, generate_contract, validate_contract, retrieve_and_verify, iac_query, review_pr, etc.). Does not teach tool composition. |
| 69 | `detect_drift` listed as common | **EXPERIMENTAL stability, not common.** Mis-ranks this tool. |
| 70 | `find_pattern_by_intent` listed as common | **EXPERIMENTAL stability, not common.** Mis-ranks this tool. |
| 76–81 | Errors and recovery | **GENERAL ADVICE** — keep, but expand. |
| 82 | "see `cogh doctor` for tool schema validation" | **STALE / UNVERIFIED.** `cogh doctor` is for install validation; it does not validate tool schemas. |
| 85–89 | "CI gates (use `cogh` automation)" with `just post-e31-audit`, `just ci-t6`, `just scorecard-nightly`, `just scorecard-streak` | **DEV-REPO-SPECIFIC.** All `just` recipes are CogniCode internal; users do not have them. **MUST be removed.** |
| 94–97 | Cross-references to `docs/TEST-PLAN.md`, `docs/RELEASE-1.0.0-PLAN.md`, `docs/adr/...` | **DEV-REPO-SPECIFIC.** All internal docs. |
| (missing) | No teaching of tool composition (PROJECT DISCOVERY → ARCHITECTURE DEEP DIVE → CHANGE IMPACT → SAFE REFACTOR) | **MISSING MODERN CAPABILITY.** This is the most valuable addition. |
| (missing) | No read/mutate distinction (READ/ANALYZE vs MUTATE) | **MISSING SAFETY GUIDANCE.** |
| (missing) | No CLI vs MCP guidance | **MISSING.** |
| (missing) | No "do not advertise check_architecture as e77" warning | **PRODUCT FICTION RISK.** (WU3) |

**Verdict:** This skill is the **right place to start** for the new
MCP teaching skill, but it needs a full rewrite. The body is too
short for what it claims (97 lines for 73 tools + safety guidance +
tool composition is impossible). The structure must change.

## `manifest.yaml` audit

Both `manifest.yaml` files declare `apiVersion: cognicode/v1`. This
is **CogniCode extension metadata, not Agent Skills ecosystem
metadata**. Per WU12, this is fine as an optional extension — the
brief explicitly says `manifest.yaml` may remain optional
CogniCode-specific lifecycle metadata. No change required to
`manifest.yaml` schema.

However, **the `version: "0.92.0"` field is wrong** (binary is at
0.95.0 today). Should be replaced with a skill-version, decoupled
from the binary version.

## `references/bootstrap.sh` and `references/invoke.sh`

Both are placeholders (`echo "..."`). They are not actually invoked
from `SKILL.md`. **Either implement them or delete them** — empty
references confuse readers and pollute the bundle.

## Summary

| Issue | Where | Action |
|-------|-------|--------|
| Dev-repo-specific paths | `cognicode-core/SKILL.md:24`, `cognicode-mcp-driven/SKILL.md:30-31` | REMOVE in rewrite. |
| `target/debug/...` paths | `cognicode-mcp-driven/SKILL.md:36,41` | REMOVE in rewrite. |
| `ROADMAP.md`, `just post-e31-audit`, internal `docs/adr/` | `cognicode-core/SKILL.md:25-27`, `cognicode-mcp-driven/SKILL.md:85-97` | REMOVE in rewrite. |
| 68 tools (stale) | `cognicode-mcp-driven/SKILL.md:4,19` | UPDATE to 73, but better: stop enumerating, teach composition. |
| Wrong version (`0.92.0`) | both manifests + frontmatter | DECOUPLE from binary version. |
| Missing tool composition teaching | `cognicode-mcp-driven/SKILL.md` | ADD (WU7). |
| Missing read/mutate distinction | `cognicode-mcp-driven/SKILL.md` | ADD (WU8). |
| Missing CLI/MCP guidance | `cognicode-mcp-driven/SKILL.md` | ADD (WU9). |
| `cognicode-sandbox`, `cognicode-uat` referenced but not present | `cognicode-core/SKILL.md:32` | REMOVE or actually create the skills. |
| Placeholder `references/*.sh` | both skills | DELETE or IMPLEMENT. |
| `cognicode-core` is a dev-repo skill, mis-published as product | the whole skill | RENAME to `cognicode-developer` (per WU5). |
