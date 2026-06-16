# ADR-027: MCP Tool Consolidation — 67→49 Tools

**Status:** Accepted  
**Date:** 2026-06-16  
**Source:** Deep audit of CogniCode MCP vs Graphify

## Context

CogniCode MCP has 67 registered tools — 3.4x more than Graphify's 22. This
creates decision fatigue for AI agents. An agent calling `list_tools()` sees
a flat list of 67 tools with no hierarchy, no families, and significant
redundancy. Several tools are near-duplicates (differing only by a parameter)
or composites that make their constituent tools redundant.

Additionally, 7 tools have handler implementations but are not registered in
the tool list (graph_query_filtered, export_callflow, get_graph_report,
get_type_references, get_imports, get_implementors, get_members).

## Decision

### Phase 1: Register missing tools (+7)

Register the 7 tools with handlers but no `Tool::new()` entries. This is a
bug fix — the tools exist in the dispatch but are invisible to agents.

### Phase 2: Remove redundant tools (-18)

| Tool | Reason |
|------|--------|
| `build_lightweight_index` | Replaced by Scan stage manifest |
| `merge_file_graphs` | Replaced by PG as single source |
| `reparse_on_edit` | Replaced by file watcher (notify) |
| `complete_task`, `poll_tasks` | Agent management, not graph tools |
| `graph_god_nodes` | Absorbed by `graph_insights` |
| `graph_communities` | Absorbed by `graph_insights` |
| `graph_community_detail` | Absorbed by `graph_insights` |
| `graph_surprising_connections` | Absorbed by `graph_insights` |
| `check_architecture` | Absorbed by `graph_insights` (cycles+health) |
| `ranked_symbols` | Absorbed by `smart_search` |
| `graph_search_idf` | Absorbed by `smart_search` |
| `find_usages_with_context` | Absorbed by `find_usages` (add `context_lines`) |
| `get_hot_symbols` | Absorbed by `get_hot_paths` (add `source` param) |
| `graph_all_paths` | Absorbed by `trace_path` (add `all` param) |
| `get_outline` | Absorbed by `get_file_symbols` (add `hierarchical` param) |
| `compare_call_graphs` | Absorbed by `compare_graph` (add `mode` param) |
| `detect_api_breaks` | Absorbed by `compare_graph` |
| `evaluate_refactor_quality` | Absorbed by `compare_graph` |

### Phase 3: Consolidate composites (-7→+3, net -4)

| Consolidation | Tools merged | New tool |
|---------------|-------------|----------|
| Graph analytics | graph_condensed + graph_reduced + graph_feedback_arcs | `graph_analyze(mode)` |
| Search | semantic_search + ranked_symbols + graph_search_idf | `smart_search(algorithm)` |
| Overview | smart_overview + auto_diagnose + generate_system_prompt_context + suggest_context | `project_overview(detail)` |

### Tool deprecation policy

Deprecated tools are not removed from the codebase — they remain functional
but are marked `#[deprecated]` in the dispatch. The `list_tools()` response
includes `deprecated: true` in the tool's metadata. Agents can still call
them, but they see a warning. After 2 release cycles, they are removed.

### Naming convention

Consolidated tools follow `snake_case` with optional parameters instead of
separate tools:

```json
// BEFORE (3 tools):
{ "tool": "semantic_search", "arguments": { "query": "auth" } }
{ "tool": "ranked_symbols", "arguments": { "query": "auth" } }
{ "tool": "graph_search_idf", "arguments": { "query": "auth" } }

// AFTER (1 tool):
{ "tool": "smart_search", "arguments": { "query": "auth", "algorithm": "fuzzy" } }
{ "tool": "smart_search", "arguments": { "query": "auth", "algorithm": "ranked" } }
{ "tool": "smart_search", "arguments": { "query": "auth", "algorithm": "idf" } }
```

### Final tool taxonomy (49 tools in 12 families)

| Family | Count | Tools |
|--------|-------|-------|
| **BUILD** | 1 | build_graph |
| **ANALYZE** | 4 | graph_insights, graph_pagerank, graph_analyze, project_overview |
| **SEARCH** | 2 | smart_search, find_usages |
| **NAVIGATE** | 5 | get_call_hierarchy, trace_path, analyze_impact, get_entry_points, get_leaf_functions |
| **FILES** | 5 | read_file, search_content, list_files, write_file, edit_file |
| **SYMBOLS** | 4 | get_file_symbols, get_symbol_code, get_complexity, nl_to_symbol |
| **REFACTOR** | 2 | safe_refactor, compare_graph |
| **LSP** | 3 | go_to_definition, hover, find_references |
| **AIX** | 3 | ask_about_code, find_pattern_by_intent, suggest_plan |
| **GRAPHIFY** | 5 | graph_query, graph_explain, get_graph_report, export_callflow, graph_query_filtered |
| **EDGE-TYPE** | 4 | get_type_references, get_imports, get_implementors, get_members |
| **AVC** | 2 | generate_contract, validate_contract |
| **MISC** | 9 | query_symbol_index, get_per_file_graph, build_call_subgraph, export_mermaid, retrieve_and_verify, detect_drift, detect_god_functions, detect_long_parameter_lists, get_hot_paths, graph_suggest_questions, suggest_context |

## Rationale

- **Decision fatigue is real.** 67 tools overload the agent's context window
  and make it harder to choose the right tool. Graphify's 22 tools are proven
  to be sufficient for comprehensive code intelligence.
- **Composites are better than individuals.** An agent making one `graph_insights`
  call gets more value than 5 individual calls. Fewer round-trips, lower latency.
- **Parameters over new tools.** Adding `context_lines` to `find_usages` is
  better than having a separate `find_usages_with_context` tool. Parameters are
  self-documenting; tool proliferation is not.
- **Deprecation, not deletion.** Marking tools deprecated preserves backward
  compatibility for existing agent workflows while signaling that they should
  migrate.

## Consequences

- 18 tools deprecated, 7 tools registered, 7 tools consolidated into 3.
- All deprecated tools remain functional for 2 release cycles.
- `list_tools()` response adds `deprecated` field and `family` grouping.
- Agent prompts and documentation must be updated to reflect the new taxonomy.
- Graphify-equivalent tools now represent 50% of the catalog (vs 30% before).

## Alternatives Considered

- **Keep all 67 tools:** rejected — decision fatigue, redundancy, maintenance burden.
- **Remove deprecated tools immediately:** rejected — breaks existing agent workflows.
  Two-cycle deprecation period gives agents time to migrate.
- **Tool families without consolidation:** rejected — families help but don't solve
  the core problem of redundancy.
