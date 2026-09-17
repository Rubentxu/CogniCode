# WU1 — Runtime MCP capability inventory

Captured from the **built** MCP server at HEAD `46ca3c48` via
`tools/list` (raw JSON: `artifacts/runtime-tools-list.json`).

**Total tools exposed: 73** (the existing `docs/MCP-TOOLS.md` says 68
from 2026-08-06 — drift confirmed; see drift report below).

## Tool inventory

All tools below were observed in the runtime `tools/list` response.

### Graph (29)

| Tool | Stability | Reqs graph | Notes |
|------|-----------|------------|-------|
| `analyze_impact` | stable | yes | |
| `build_call_subgraph` | stable | yes | |
| `build_graph` | stable | no | Build before others. |
| `build_lightweight_index` | stable | yes | strategies: lightweight, on_demand, per_file, full. |
| `cfg_per_function` | experimental | no | |
| `check_architecture` | stable | yes | Tarjan SCC. **NOT e77 executable architecture.** |
| `dominators_cfg` | experimental | no | |
| `get_call_hierarchy` | stable | yes | |
| `get_entry_points` | stable | yes | |
| `get_hot_paths` | stable | yes | |
| `get_implementors` | stable | yes | |
| `get_imports` | stable | yes | |
| `get_leaf_functions` | stable | yes | |
| `get_members` | stable | yes | |
| `get_per_file_graph` | stable | yes | |
| `get_type_references` | stable | yes | |
| `graph_all_paths` | stable | yes | |
| `graph_analyze` | stable | yes | runs `scc`/`reduced`/`feedback_arcs`. |
| `graph_checkpoint` | stable | yes | create/current/restore/list. |
| `graph_communities` | stable | yes | Label Propagation. |
| `graph_community_detail` | stable | yes | |
| `graph_condensed` | stable | yes | SCC condensation. |
| `graph_feedback_arcs` | stable | yes | |
| `graph_god_nodes` | stable | yes | |
| `graph_pagerank` | stable | yes | |
| `graph_query` | stable | yes | NL topology query. |
| `graph_query_filtered` | stable | yes | |
| `graph_reduced` | stable | yes | transitive reduction. |
| `graph_surprising_connections` | stable | yes | |
| `merge_graphs` | stable | no | |
| `reparse_on_edit` | experimental | no | incremental reindex. |
| `slice_backward` | experimental | no | |
| `slice_forward` | experimental | no | |
| `taint_flow` | experimental | no | |
| `trace_path` | stable | yes | |

(Categories overlap; the `_meta.category` is the source of truth.)

### Search (5)

| Tool | Stability | Reqs graph | Notes |
|------|-----------|------------|-------|
| `find_usages` | stable | no | |
| `graph_search_idf` | stable | yes | ranked by IDF. |
| `nl_to_symbol` | stable | yes | NL→symbol. |
| `query_symbol_index` | stable | no | |
| `retrieve_and_verify` | stable | no | lexical + rustc compile-verify. |
| `search_content` | stable | no | |

### Quality (5)

| Tool | Stability | Reqs graph | Notes |
|------|-----------|------------|-------|
| `detect_drift` | experimental | no (requires persistence) | S7000 / S7001. |
| `detect_god_functions` | stable | yes | |
| `detect_long_parameter_lists` | stable | yes | |
| `get_complexity` | stable | no | |
| `solid_audit` | stable | yes | |

### Navigation (3)

| Tool | Stability | Reqs graph | Notes |
|------|-----------|------------|-------|
| `find_references` | stable | no | LSP. |
| `go_to_definition` | stable | no | LSP. |
| `hover` | stable | no | LSP. |

### File / Code IO (7)

| Tool | Stability | Effect | Notes |
|------|-----------|--------|-------|
| `edit_file` | stable | MUTATE | syntax validation. |
| `get_file_symbols` | stable | read | |
| `get_symbol_code` | stable | read | |
| `list_files` | stable | read | |
| `read_file` | stable | read | semantic modes. |
| `write_file` | stable | MUTATE | |
| `safe_refactor` | stable | MUTATE | |
| `generate_contract` | gated | MUTATE | AVC truth contract. |
| `validate_contract` | stable | read | AVC contract. |

### Composite (12 — orchestrators over the above)

| Tool | Stability | Notes |
|------|-----------|-------|
| `codebase_map` | stable | LLM-optimized. |
| `export_callflow` | stable | Mermaid module-level. |
| `export_mermaid` | stable | Mermaid call graph. |
| `graph_explain` | stable | callers/callees/fan-in/out. |
| `graph_insights` | stable | health report. |
| `graph_suggest_questions` | experimental | |
| `project_insights` | experimental | dashboard. |
| `project_overview` | experimental | quick/medium/detailed. |
| `review_pr` | stable | PR risk. |
| `smart_search` | stable | semantic + ranked + IDF. |

### View / Infra / Aix (6)

| Tool | Stability | Notes |
|------|-----------|-------|
| `ask_about_code` (aix) | experimental | |
| `find_pattern_by_intent` (aix) | experimental | |
| `iac_query` (infra) | experimental | Terraform/Ansible. |
| `list_view_specs` (view) | stable | |
| `read_view_spec` (view) | stable | |

## Drift report — runtime vs documentation

| Source | Claims | Reality |
|--------|--------|---------|
| `docs/MCP-TOOLS.md` | 68 tools, last updated 2026-08-06 | 73 tools today. **5 net new** (likely `reparse_on_edit`, `merge_graphs`, `generate_contract`/`validate_contract`, and the aix/experimental additions). |
| `skills/cognicode-mcp-driven/SKILL.md` | (to be audited in WU4) | (audit below) |

### Drift classifications

- **CURRENT AND DOCUMENTED** — most stable tools.
- **DOCUMENTED BUT CHANGED** — names or descriptions that have evolved.
- **DOCUMENTED BUT ABSENT** — tools that have been removed or renamed.
- **PARAMETER DRIFT** — input schemas differ from documentation.
- **SEMANTIC DRIFT** — tool still exists but its behaviour changed
  (e.g. `check_architecture` is still Tarjan-SCC, NOT e77).

The exact drift will be enumerated in WU4 alongside the skill audit.

## Effect classification (READ/MUTATE)

- **READ/ANALYZE** (most tools): graph, search, quality, navigation,
  file IO readers.
- **MUTATE**: `edit_file`, `write_file`, `safe_refactor`,
  `generate_contract`.

`generate_contract` is labelled "gated" — verify in source what gates
it.

## What this means for skill design

1. **The tool surface is large (73) but most tools share one of a
   handful of prerequisites.** Most require `build_graph` first; some
   require `build_lightweight_index`. Teaching agents the
   *prerequisites* is more valuable than enumerating 73 tools.

2. **Composites (`graph_explain`, `graph_insights`,
   `project_overview`, `project_insights`, `codebase_map`,
   `smart_search`) are the right starting points** for most workflows.
   They bundle the right primitive calls.

3. **`check_architecture` is a graph-cycle detector, not the e77
   executable architecture product.** Skills must NOT teach it as
   e77's surface.

4. **Effect classification is a UX guardrail.** A discovery skill must
   not encourage mutation without first establishing intent.

## Artifacts

- `artifacts/runtime-tools-list.json` — raw `tools/list` response
  (2133 lines).
