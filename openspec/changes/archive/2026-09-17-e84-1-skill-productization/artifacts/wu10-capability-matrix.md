# WU10 — Capability coverage matrix

Per-capability × per-surface matrix. Each cell shows whether the
capability is reachable today through that surface, plus whether a
public skill covers it.

Surfaces:

- **CLI** — `cognicode` + `cogh` commands.
- **MCP** — runtime `tools/list` (73 tools today).
- **Explorer** — the in-repo browser UI (visual).
- **Skill** — public user skill that teaches the capability.

Statuses: **YES** (reachable today), **PARTIAL** (reachable with caveats),
**NO** (not reachable), **?** (not yet verified).

## Matrix

| Capability | CLI | MCP | Explorer | Skill | Status |
|------------|-----|-----|----------|-------|--------|
| Project overview / discovery | PARTIAL (`analyze`, `index`) | YES (`project_overview`, `project_insights`, `codebase_map`) | YES (overview view) | YES (`cognicode`) | PUBLIC |
| Symbol lookup | YES (`index query`, `index outline`) | YES (`query_symbol_index`, `get_symbol_code`) | YES | YES (`cognicode`) | PUBLIC |
| LSP navigation (def/hover/refs) | YES (`navigate ...`) | YES (`go_to_definition`, `hover`, `find_references`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Call graph | YES (`graph ...`) | YES (`build_graph`, 30+ graph tools) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Graph algorithms (SCC, PageRank, communities) | PARTIAL (`graph on-demand`, `graph mermaid`) | YES (full) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Cycle detection (Tarjan SCC) | NO | YES (`check_architecture`) | YES | YES (`cognicode-mcp`, with NOT-e77 caveat) | PUBLIC |
| Impact analysis (per-symbol) | YES (`graph impact`) | YES (`analyze_impact`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Call hierarchy | YES (`graph hierarchy`) | YES (`get_call_hierarchy`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Hot paths / entry points / leaves | YES (`graph hot-paths`, `entry-points`, `leaf-functions`) | YES (`get_hot_paths`, `get_entry_points`, `get_leaf_functions`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Trace execution paths | YES (`graph trace-path`) | YES (`trace_path`, `graph_all_paths`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| PR risk review | NO | YES (`review_pr`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Quality (SOLID) | NO | YES (`solid_audit`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Quality (god functions / long params / complexity) | PARTIAL (`graph complexity`) | YES (`detect_god_functions`, `detect_long_parameter_lists`, `get_complexity`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Quality (intent drift / AVC violations) | NO | YES (`detect_drift`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Safe refactor (with validation) | YES (`refactor`) | YES (`safe_refactor`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Contract generation / validation | NO | YES (`generate_contract`, `validate_contract`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Compile-verified retrieval | NO | YES (`retrieve_and_verify`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Incremental reindex | NO | YES (`reparse_on_edit`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Read file | NO | YES (`read_file`, `get_file_symbols`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Edit / write file | NO | YES (`edit_file`, `write_file`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| IaC (Terraform/Ansible) | NO | YES (`iac_query`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| Graph Mermaid export | YES (`graph mermaid`) | YES (`export_mermaid`, `export_callflow`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| **e77 executable architecture** | **NO** | **NO** | **NO** | **NO** (skill says so) | **NOT PUBLIC — internal-only roadmap.** |
| Canonical Facts/Evidence inspection | NO | NO | NO | NO | INTERNAL ONLY |
| Intelligence Event Log queries | NO | NO | NO | NO | INTERNAL ONLY |
| SoftwareWorld / Trial | NO | NO | NO | NO | INTERNAL ONLY |
| Historical replay / held-out promotion | NO | NO | NO | NO | FUTURE CONTROL API |
| Shadow comparison | NO | NO | NO | NO | INTERNAL ONLY |
| AI SemanticMiner / FindingCritic / FixAgent | NO | NO (aix tools are experimental, not product) | NO | NO | INTERNAL ONLY |
| Governed promotion authority | NO | NO | NO | NO | FUTURE (e80/e83, not yet exposed) |
| Slicing / CFG / dominators / taint flow | NO | YES (experimental only) | YES | YES (`cognicode-mcp`, marked experimental) | PUBLIC-EXPERIMENTAL |
| Project listing / registry | NO | NO (no listing tool today) | YES (Explorer views) | NO | PARTIAL — Explorer only |
| View spec catalog | NO | YES (`list_view_specs`, `read_view_spec`) | YES | YES (`cognicode-mcp`) | PUBLIC |
| SkillSource model (metadata only) | NO | NO | NO | NO | META — implemented in WU14 docs only |
| `cogh skill install` | NO | NO | NO | NO | FUTURE (e86) |

## What this matrix tells us

### `PUBLIC CAPABILITY WITH NO SKILL COVERAGE`

None. Every public capability has at least one of the user skills
covering it.

### `INTERNAL CAPABILITY ACCIDENTALLY ADVERTISED`

None in the new skills. The `cognicode-mcp` skill explicitly
**excludes** e77 executable architecture, LSI facts/evidence, and
the governed promotion authority model from its teaching. The
`check_architecture` description includes the "Tarjan SCC, NOT e77"
caveat. Future skills must continue this discipline.

### `PUBLIC CAPABILITY WITH ONLY ONE SURFACE`

- **CLI-only**: install lifecycle (`cogh install/uninstall/list/
  current/latest/update/reshim/rollback/doctor/where/init/plugin/
  ide/version`). The user skill doesn't teach CLI lifecycle commands
  in detail — they're covered in `cogh --help`.
- **MCP-only**: most quality tools (`solid_audit`, `detect_drift`,
  `detect_long_parameter_lists`), IaC, contracts, edit_file /
  write_file. These are intentionally MCP-only; the CLI focuses on
  analysis, the MCP on agent-driven reasoning + mutation.

### `EXPLORER-ONLY`

View specs and the visual project listing. These exist in Explorer
but not (yet) in the MCP. Recorded as future evolutives.

### `INTERNAL / FUTURE`

- e77 architecture (FUTURE).
- Canonical Facts/Evidence, Intelligence Event Log (INTERNAL ONLY).
- SoftwareWorld / Trial (INTERNAL ONLY).
- Historical replay / held-out promotion (FUTURE CONTROL API).
- Shadow comparison (INTERNAL ONLY).
- Governed promotion authority (FUTURE, e80/e83).
- AI SemanticMiner / FindingCritic / FixAgent (INTERNAL ONLY).

These are explicitly NOT in any public skill. They show up in
ADRs (`docs/adr/`) for the developer audience.

## Why the matrix matters

This matrix is the **input** for:

- **Explorer evolutives** — what to surface visually.
- **Control Plane (CP1+)** — what authority the future control
  plane will gate.
- **Future MCP/CLI evolutives** — what to bring from INTERNAL to
  PUBLIC.

It also prevents the skill from drifting into product-fiction: if
a capability is INTERNAL or FUTURE, no skill teaches it as if it
were PUBLIC.

## Status

Captured. Future re-runs (post-e85, post-e86 followups) will
update this matrix as capabilities cross the INTERNAL → PUBLIC
boundary.
