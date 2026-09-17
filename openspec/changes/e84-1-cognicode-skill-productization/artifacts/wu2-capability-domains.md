# WU2 — Capability domains (user-oriented)

Derived from the **runtime** inventory (WU1), not from MCP method
prefixes. Grouping is by *user intent*, not by `_meta.category`.

The runtime decides the taxonomy. This document is the result.

## DISCOVER

Understand the shape of the project before touching code.

| Tool | Surface | Notes |
|------|---------|-------|
| `project_overview` | MCP composite | quick / medium / detailed levels. |
| `project_insights` | MCP composite | dashboard: symbols, edges, dead code, health score. |
| `codebase_map` | MCP composite | LLM-optimized (compact ~400 / detailed ~2000 tokens). |
| `list_files` | MCP | `.gitignore`-aware. |
| `list_view_specs` / `read_view_spec` | MCP view | Built-in views: overview, call-graph, etc. |

**CLI surface:** `cognicode analyze [PATH]` is the deterministic
shallow-discovery. `cognicode doctor` reports tool availability.

**Recommended starting flow:**

```text
project_overview (quick)
   ↓
project_insights (dashboard)
   ↓
codebase_map (LLM-optimized if context is tight)
```

## NAVIGATE

Find specific symbols, definitions, references, types — the LSP
inner loop.

| Tool | Surface | Notes |
|------|---------|-------|
| `go_to_definition` | MCP / `cognicode navigate definition` | LSP. |
| `hover` | MCP / `cognicode navigate hover` | LSP. |
| `find_references` | MCP / `cognicode navigate references` | LSP. |
| `get_symbol_code` | MCP / `cognicode index symbol-code` | Source of a symbol. |
| `query_symbol_index` | MCP / `cognicode index query` | By name. |
| `nl_to_symbol` | MCP | NL → symbol. |
| `search_content` | MCP / `cognicode ...` | Content search. |
| `get_file_symbols` | MCP | Symbols in a file. |

**CLI surface:** `cognicode navigate {definition,hover,references}`
and `cognicode index {build,query,outline,symbol-code}`.

**Recommended starting flow:**

```text
query_symbol_index (find by name)
   ↓
get_symbol_code (read the source)
   ↓
go_to_definition / find_references (jump around)
```

## GRAPH / ARCHITECTURE EXPLORATION

Understand modules, dependencies, cycles, communities, hot paths,
god nodes, surprising connections — the structural layer.

| Tool | Surface | Notes |
|------|---------|-------|
| `build_graph` | MCP | Prerequisite for most graph tools. |
| `build_lightweight_index` | MCP | Faster but limited. |
| `graph_insights` | MCP composite | Health report. |
| `graph_explain` | MCP composite | Caller/callee/fan-in/out for one symbol. |
| `graph_communities` | MCP | Label Propagation. |
| `graph_community_detail` | MCP | Members + cohesion score. |
| `graph_pagerank` | MCP | Importance scores. |
| `graph_god_nodes` | MCP | PageRank above a percentile. |
| `graph_surprising_connections` | MCP | Cross-community edges. |
| `graph_all_paths` | MCP | Simple paths between two symbols. |
| `graph_query` / `graph_query_filtered` | MCP | NL topology query. |
| `graph_condensed` / `graph_reduced` / `graph_feedback_arcs` / `graph_analyze` | MCP | Algorithmic primitives. |
| `graph_checkpoint` | MCP | Persistence. |
| `merge_graphs` / `get_per_file_graph` | MCP | Per-file ↔ merged. |
| `reparse_on_edit` | MCP | Incremental reindex. |
| `check_architecture` | MCP | Tarjan SCC cycle detector. **NOT e77.** |
| `get_call_hierarchy` / `get_entry_points` / `get_leaf_functions` / `get_hot_paths` / `get_implementors` / `get_imports` / `get_members` / `get_type_references` | MCP | Specific views. |
| `trace_path` | MCP | BFS between two symbols. |
| `export_callflow` | MCP | Mermaid module-level. |
| `export_mermaid` | MCP | Mermaid call graph. |
| `build_call_subgraph` | MCP | Subgraph around a symbol. |
| `slice_forward` / `slice_backward` / `cfg_per_function` / `dominators_cfg` / `taint_flow` | MCP (experimental) | Slicing / CFG. |

**CLI surface:** `cognicode graph {on-demand,per-file,full,hot-paths,
entry-points,leaf-functions,trace-path,mermaid,hierarchy,complexity,
impact}`.

**Recommended starting flow:**

```text
build_graph
   ↓
graph_insights (health overview)
   ↓
graph_communities (clusters)
   ↓
graph_explain (per-symbol deep dive)
```

## IMPACT

"What breaks if I change this?" — scoping before mutation.

| Tool | Surface | Notes |
|------|---------|-------|
| `analyze_impact` | MCP / `cognicode graph impact` | Risk level + impacted files. |
| `trace_path` | MCP / `cognicode graph trace-path` | BFS paths. |
| `get_call_hierarchy` | MCP / `cognicode graph hierarchy` | |
| `get_hot_paths` | MCP / `cognicode graph hot-paths` | |
| `graph_all_paths` | MCP | All simple paths. |
| `review_pr` | MCP composite | PR-level risk. |

**Recommended starting flow:**

```text
analyze_impact (symbol-level)
   ↓
get_call_hierarchy (callers/callees)
   ↓
review_pr (when reviewing a PR)
```

## QUALITY

Detect smells and SOLID violations.

| Tool | Surface | Notes |
|------|---------|-------|
| `solid_audit` | MCP | SRP, OCP, LSP, ISP, DIP. |
| `detect_drift` | MCP (experimental) | docstring vs body (S7000), unsafe/panic/unwrap (S7001). |
| `detect_god_functions` | MCP / `cognicode ...` | |
| `detect_long_parameter_lists` | MCP / `cognicode ...` | |
| `get_complexity` | MCP / `cognicode graph complexity` | cyclomatic, cognitive, nesting. |

**Recommended starting flow:**

```text
solid_audit (broad scan)
   ↓
get_complexity + detect_god_functions (per-function)
   ↓
detect_long_parameter_lists (signature smell)
```

## SAFE CHANGE

Mutate with guardrails.

| Tool | Surface | Effect | Notes |
|------|---------|--------|-------|
| `safe_refactor` | MCP | MUTATE | With validation + preview. |
| `generate_contract` | MCP | MUTATE (gated) | AVC truth contract. |
| `validate_contract` | MCP | read | AVC contract. |
| `retrieve_and_verify` | MCP | read | Lexical + rustc compile-verify. |
| `reparse_on_edit` | MCP | read | Incrementally reindex after a change. |

**CLI surface:** `cognicode refactor {rename,extract,inline,move}`.

**Recommended starting flow:**

```text
analyze_impact (scope)
   ↓
generate_contract (capture truth before mutation)
   ↓
safe_refactor
   ↓
validate_contract
   ↓
reparse_on_edit (reindex incrementally)
```

## CODE IO

Read, edit, write.

| Tool | Surface | Effect | Notes |
|------|---------|--------|-------|
| `read_file` | MCP | read | Semantic modes. |
| `edit_file` | MCP | MUTATE | Syntax-validated edits. |
| `write_file` | MCP | MUTATE | Overwrite. |
| `get_file_symbols` | MCP / `cognicode index outline` | read | |
| `list_files` | MCP / `cognicode ...` | read | |

**CLI surface:** none directly equivalent on the CLI today. The
analysis CLI focuses on read/inspection.

## INFRASTRUCTURE

Infra-as-code.

| Tool | Surface | Notes |
|------|---------|-------|
| `iac_query` | MCP (experimental) | Terraform + Ansible. Requires `build_graph` and persistence. |

**CLI surface:** none. `cognicode` is code-only.

## Experimental / aix

| Tool | Surface | Notes |
|------|---------|-------|
| `ask_about_code` | MCP (experimental) | |
| `find_pattern_by_intent` | MCP (experimental) | |
| `graph_suggest_questions` | MCP (experimental) | |

These are intentionally not in any of the user-facing workflows
above. They exist; they may be useful; they are not part of the
recommended flows.

## Cross-domain

A few tools sit in multiple domains. `graph_explain` lives in GRAPH
and IMPACT. `codebase_map` lives in DISCOVER and NAVIGATE. The
domain assignment above is by primary intent; cross-references are
allowed.

## What this is NOT

This is **not** a tool encyclopedia. Each domain lists the tools
agents typically reach for, plus the recommended starting flow.
Tools not listed in a domain are still real and usable; this is
guidance, not a constraint.
