# CogniCode MCP Tools

> **Source of truth: runtime `tools/list`** (68 tools, paginated — see `sandbox/scripts/list_mcp_tools.sh`).
> Regenerated 2026-08-06 from the live MCP server. Do NOT hand-edit the tool list; re-run the probe.

## Overview

- Total runtime tools: **68**
- Probe: `bash sandbox/scripts/list_mcp_tools.sh` (paginated, base64 offset cursor, PAGE_SIZE=20)
- Coverage gate (G2): `just release-scorecard` → generate_tool_coverage.py

## Tool Catalog

### analyze* (1 tools)

| Tool | Description |
|------|-------------|
| `analyze_impact` | Analyze the impact of changing a symbol. Returns impacted files and risk level. |

### ask* (1 tools)

| Tool | Description |
|------|-------------|
| `ask_about_code` | Answer questions about code flow by tracing execution paths between symbols. |

### build* (3 tools)

| Tool | Description |
|------|-------------|
| `build_call_subgraph` | Build an on-demand call subgraph centered on a symbol. |
| `build_graph` | Build the call graph for a project directory. Must be called before get_call_hierarchy, analyze_impact, or che |
| `build_lightweight_index` | Build a lightweight symbol index for fast lookups. Supports strategies: lightweight, on_demand, per_file, full |

### check* (1 tools)

| Tool | Description |
|------|-------------|
| `check_architecture` | Detect cycles and architecture violations using Tarjan SCC algorithm. Requires build_graph first. |

### codebase* (1 tools)

| Tool | Description |
|------|-------------|
| `codebase_map` | Generate a compact, LLM-optimized codebase map. Format: compact (~400 tokens) or detailed (~2000). |

### detect* (3 tools)

| Tool | Description |
|------|-------------|
| `detect_drift` | Analyze a source file for intent drift (S7000: docstring-body mismatch), AVC violations (S7001: unsafe/panic/u |
| `detect_god_functions` | Find overly large or complex functions (god functions) that should be refactored. |
| `detect_long_parameter_lists` | Find functions with too many parameters that should be consolidated into structs. |

### edit* (1 tools)

| Tool | Description |
|------|-------------|
| `edit_file` | Edit files with syntax validation. |

### export* (2 tools)

| Tool | Description |
|------|-------------|
| `export_callflow` | Export a community-level Mermaid architecture call-flow diagram. Shows module-level relationships. |
| `export_mermaid` | Export call graph or subgraph as Mermaid flowchart. Optionally render to SVG with a theme. |

### find* (3 tools)

| Tool | Description |
|------|-------------|
| `find_pattern_by_intent` | Match natural language intent descriptions to known code patterns. |
| `find_references` | Find all references to a symbol using LSP. |
| `find_usages` | Find all usages of a symbol across the project. |

### generate* (1 tools)

| Tool | Description |
|------|-------------|
| `generate_contract` | Generate an AVC truth contract from an existing function. Returns syntax, semantic, and safety constraints. |

### get* (12 tools)

| Tool | Description |
|------|-------------|
| `get_call_hierarchy` | Traverse call graph to find callers (incoming) or callees (outgoing). Requires build_graph first. |
| `get_complexity` | Calculate code complexity metrics (cyclomatic, cognitive, nesting). |
| `get_entry_points` | Find symbols with no incoming edges (entry points in the call graph). Requires build_graph first. |
| `get_file_symbols` | Extract symbols (functions, classes, variables) from a source file. Set compressed=true for natural language s |
| `get_hot_paths` | Find functions with highest fan-in (most called functions). |
| `get_implementors` | Find all types that implement a given trait/interface. Uses Implements edges. Requires build_graph first. |
| `get_imports` | List all imports for a file. Uses Imports edges from the ingest pipeline. Requires build_graph first. |
| `get_leaf_functions` | Find symbols with no outgoing edges (leaf functions in the call graph). Requires build_graph first. |
| `get_members` | List methods and fields of a class/struct. Uses Contains edges. Requires build_graph first. |
| `get_per_file_graph` | Get the call graph for a specific file. |
| `get_symbol_code` | Get the full source code of a symbol at a given location, including docstrings. |
| `get_type_references` | List type annotation references for a symbol (param types, return types, field types). Uses References edges f |

### go* (1 tools)

| Tool | Description |
|------|-------------|
| `go_to_definition` | Navigate to the definition of a symbol using LSP. |

### graph* (17 tools)

| Tool | Description |
|------|-------------|
| `graph_all_paths` | Find all simple paths between two symbols in the call graph. Requires build_graph first. |
| `graph_analyze` | Run advanced graph algorithms: scc, reduced, or feedback_arcs. |
| `graph_checkpoint` | Manage graph checkpoints: create (build+checkpoint), current (get latest), restore (get by id), list (list all |
| `graph_communities` | Detect code communities using Label Propagation. Groups symbols that are tightly coupled into clusters. Return |
| `graph_community_detail` | Get details for a specific community detected by graph_communities (members, internal/external edge counts, co |
| `graph_condensed` | Compute the SCC condensation of the call graph: every strongly connected component is collapsed into a single  |
| `graph_explain` | Composite deep-dive on a symbol: callers, callees, fan-in/out, complexity. Saves multiple tool calls. Requires |
| `graph_feedback_arcs` | Find a feedback arc set — edges whose removal would make the call graph acyclic. The greedy heuristic is not o |
| `graph_god_nodes` | Find god nodes — symbols with unusually high PageRank (above the supplied percentile). These are symbols that  |
| `graph_insights` | Get a complete architecture health report: god nodes, circular dependencies, community overview, surprising cr |
| `graph_pagerank` | Compute PageRank importance scores for all symbols in the call graph. Returns a ranked list of symbols by depe |
| `graph_query` | Natural language graph topology query. Ask 'what connects X to Y?' and get a subgraph with provenance. Require |
| `graph_query_filtered` | Graph query with provenance, node kind, and community filters. Requires build_graph first. |
| `graph_reduced` | Compute the transitive reduction of the call graph — the minimal set of dependency edges that preserves reacha |
| `graph_search_idf` | Search symbols ranked by IDF (Inverse Document Frequency) importance. Rare terms score higher. Includes hub by |
| `graph_suggest_questions` | Generate intelligent questions about the codebase architecture based on graph analysis. Helps identify areas t |
| `graph_surprising_connections` | Find surprising cross-community connections. These are edges between symbols in different communities, indicat |

### hover* (1 tools)

| Tool | Description |
|------|-------------|
| `hover` | Get type information and documentation for a symbol at a position using LSP. |

### iac* (1 tools)

| Tool | Description |
|------|-------------|
| `iac_query` | Query infrastructure-as-code resources (Terraform, Ansible) and their dependencies from the graph. Requires bu |

### list* (2 tools)

| Tool | Description |
|------|-------------|
| `list_files` | List project files with .gitignore awareness. |
| `list_view_specs` | List all ViewSpecs visible to the current workspace (built-in + persisted runtime). Returns descriptors (id, t |

### merge* (1 tools)

| Tool | Description |
|------|-------------|
| `merge_graphs` | Merge per-file call graphs into a consolidated project graph. |

### nl* (1 tools)

| Tool | Description |
|------|-------------|
| `nl_to_symbol` | Convert natural language descriptions to symbol matches using keyword extraction and semantic search. |

### project* (2 tools)

| Tool | Description |
|------|-------------|
| `project_insights` | Dashboard in a single call: symbols, edges, entry points, dead code, health score, hot paths. |
| `project_overview` | Get a comprehensive project overview at quick, medium, or detailed levels. |

### query* (1 tools)

| Tool | Description |
|------|-------------|
| `query_symbol_index` | Query the symbol index to find locations of a symbol by name (case-insensitive). |

### read* (2 tools)

| Tool | Description |
|------|-------------|
| `read_file` | Smart file reader with semantic modes. |
| `read_view_spec` | Read a full ViewSpec by id (built-in kebab id like 'overview', 'call-graph'; or runtime UUID). Returns the com |

### reparse* (1 tools)

| Tool | Description |
|------|-------------|
| `reparse_on_edit` | Incrementally reindex changed files without rebuilding the full graph. Much faster than full rebuild for small |

### retrieve* (1 tools)

| Tool | Description |
|------|-------------|
| `retrieve_and_verify` | Search for code matching a query and verify Rust files via sandboxed rustc compilation. Combines lexical searc |

### review* (1 tools)

| Tool | Description |
|------|-------------|
| `review_pr` | Analyze PR impact: provide changed files, get risk level, impacted files, and breaking changes. |

### safe* (1 tools)

| Tool | Description |
|------|-------------|
| `safe_refactor` | Perform safe refactoring with validation and preview. |

### search* (1 tools)

| Tool | Description |
|------|-------------|
| `search_content` | Search file contents with .gitignore awareness. |

### smart* (1 tools)

| Tool | Description |
|------|-------------|
| `smart_search` | Run semantic_search + ranked_symbols + graph_search_idf in parallel with deduplication. Returns merged results |

### solid* (1 tools)

| Tool | Description |
|------|-------------|
| `solid_audit` | Analyze code for SOLID principle violations (SRP, OCP, LSP, ISP, DIP). Returns violations with severity, locat |

### trace* (1 tools)

| Tool | Description |
|------|-------------|
| `trace_path` | Find execution path between two symbols using BFS. |

### validate* (1 tools)

| Tool | Description |
|------|-------------|
| `validate_contract` | Validate generated code against an AVC truth contract. Returns pass/fail with violations and fix suggestions. |

### write* (1 tools)

| Tool | Description |
|------|-------------|
| `write_file` | Create or overwrite files within the workspace. |

