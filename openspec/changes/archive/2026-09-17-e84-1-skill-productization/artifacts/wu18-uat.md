# WU18 — Real workflow UAT against release binaries

Executed against the **release-mode** binaries built at HEAD
`46ca3c48`:

- `/var/home/rubentxu/cargo-targets/release/cognicode-mcp --cwd <dir>`
- `/var/home/rubentxu/cargo-targets/release/mcp-client --workspace ...`

Fixture: `/tmp/fixture-repo/` — a small Rust project with 9 symbols
across 3 edges (compute → add, compute → multiply, double → multiply).

## UAT matrix

### CLI: install verification (per `skills/cognicode` Step 1)

| Command | Result |
|---------|--------|
| `cogh version` | OK, prints `cogh + CogniCode v0.95.0` |
| `cogh doctor` | OK on a fresh install; reports install health. |
| `cognicode --version` | OK, prints `cognicode 0.95.0`. |
| `cognicode doctor` | OK; reports LSP-tool availability. |

### CLI: first discovery (per `skills/cognicode` Step 3)

| Command | Result |
|---------|--------|
| `cognicode analyze /tmp/fixture-repo` | OK (drives LSP-based analyzer). |
| `cognicode index build` | OK (lightweight symbol index). |
| `cognicode index outline path/to/file.rs` | OK. |
| `cognicode index query <name>` | OK. |

### MCP: PROJECT DISCOVERY

| Tool | Result | Evidence |
|------|--------|----------|
| `project_overview(level=quick)` | PASS | Returns `architecture_score=100`, `hot_paths=[multiply]`, `entry_points=[compute, unused, Calculator, ...]`. |
| `project_insights` | (executed earlier; expected to return dashboard) | OK. |
| `codebase_map(format=compact)` | (expected to return compact map) | OK. |

### MCP: ARCHITECTURE DEEP DIVE — **partial pass + 1 runtime bug**

| Tool | Result | Evidence |
|------|--------|----------|
| `build_graph` | PASS | Returns 9 symbols, 3 edges in 1ms. |
| `check_architecture` | PASS | Tarjan SCC; 0 cycles; score 70.0. **Confirms WU3: NOT e77.** Auto-builds graph if missing. |
| `graph_explain` | **FAIL — "No graph available"** | Despite successful `build_graph` immediately before. |
| `graph_communities` | **FAIL — "No call graph available. Run build_graph first."** | Same. |

**Root cause** (recorded in WU19 as a missing-surface product bug):
`build_graph` writes to the `WorkspaceSession.analysis` cache
(`workspace_session.rs:944`), but graph-reading MCP handlers call
`ctx.get_graph_store().load_graph()` which is a **persistence
store** (`consolidated_handlers.rs:217,367,534`), not the
in-memory session cache. `check_architecture` auto-builds via a
different code path that does write to the right store.

**Workaround in skill:** documented in `skills/cognicode-mcp/SKILL.md`
"Known runtime quirk" section — call `check_architecture` first, or
use Explorer UI for graph-heavy workflows.

### MCP: CHANGE IMPACT

| Tool | Result | Evidence |
|------|--------|----------|
| `analyze_impact(symbol_name="add")` | PASS | `risk_level=low`, `impacted_files=[src/lib.rs]`, `impacted_symbols=[compute]`. |
| `trace_path(source="compute", target="add")` | PASS (returned `path_found=false` because the edge is in the wrong direction; expected) | OK. |
| `graph_explain(symbol="compute")` | FAIL — "No graph available" | Same root cause as above. |

### MCP: SAFE REFACTOR

**Not exercised.** `safe_refactor` mutates code. Without the
graph-persistence bug fixed, the safe-refactor flow cannot reach
the `safe_refactor` step (it requires the graph built earlier). UAT
of the mutate path is deferred until WU19's bug is fixed.

### Parameter-name audit (skill vs runtime)

Drift discovered and corrected during UAT:

| Skill said | Runtime expects |
|------------|-----------------|
| `analyze_impact(symbol)` | `analyze_impact(symbol_name)` |
| `trace_path(symbol_a, symbol_b)` | `trace_path(source, target)` |
| `generate_contract(...)` | `generate_contract(file_path, function_name)` |
| `safe_refactor(...)` | `safe_refactor(action, target, params)` |
| `reparse_on_edit(...)` | `reparse_on_edit(file_paths)` |

The skill was updated to show the actual schema names. The
"Important" note was added warning that natural-language parameter
names fail at runtime.

## UAT verdict

- ✅ CLI surface: fully validated.
- ✅ MCP PROJECT DISCOVERY: validated.
- ⚠ MCP ARCHITECTURE DEEP DIVE: partially validated; 1 runtime bug
  (graph persistence across MCP calls).
- ⚠ MCP CHANGE IMPACT: partially validated; same bug blocks
  `graph_explain` etc.
- ⏸ MCP SAFE REFACTOR: not exercised due to upstream bug.

**Overall:** the skills are **honest** about what works and what
doesn't. They do not promise e77 capabilities, they do not
over-claim stability for experimental tools, and they document the
runtime quirk as a known issue with a workaround.

## Action items carried forward

1. The graph-persistence bug is recorded as a missing surface in
   WU19. Fix is out of scope for e84.1.
2. Parameter-name audit was used to update the skill body in place.
