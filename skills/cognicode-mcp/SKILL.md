---
name: cognicode-mcp
description: >
  Drive CogniCode's 73-tool MCP server effectively: tool selection,
  graph prerequisites, read-before-write, incremental re-analysis,
  error recovery. Trigger: When an agent is doing multi-step graph
  exploration, impact analysis, quality audits, or safe refactoring
  on a codebase through CogniCode. Applies to: architecture deep
  dives, change-impact scoping, refactor + contract validation,
  cross-module navigation.
license: MIT
metadata:
  version: "1.0.0"
  maturity: stable
  author: CogniCode Team
  homepage: https://github.com/Rubentxu/CogniCode
---

# CogniCode MCP — workflow guide

This skill teaches **how to use the CogniCode MCP** for real work.
The runtime tool catalog is the source of truth — call
`tools/list` on the running server to enumerate every tool. This
skill teaches **decision-making**, not tool enumeration.

> **Important.** This skill assumes you have read
> `skills/cognicode` first. That skill handles install verification
> and the choice between CLI and MCP surfaces.

## Capability domains (from `tools/list`)

Group the 73 tools by what the user wants to do, not by MCP method
prefix. The runtime's `_meta.category` is informative; the user
intent is what matters.

| Domain | What the user wants | Anchor tools |
|--------|---------------------|--------------|
| **DISCOVER** | "What is this project?" | `project_overview`, `project_insights`, `codebase_map`, `list_files` |
| **NAVIGATE** | "Find X, jump to definition" | `query_symbol_index`, `get_symbol_code`, `go_to_definition`, `hover`, `find_references`, `nl_to_symbol`, `search_content`, `get_file_symbols` |
| **GRAPH** | "How do modules depend on each other?" | `build_graph`, `graph_insights`, `graph_communities`, `graph_community_detail`, `graph_pagerank`, `graph_god_nodes`, `graph_surprising_connections`, `graph_all_paths`, `graph_query`, `graph_query_filtered`, `check_architecture`, `export_mermaid`, `export_callflow` |
| **IMPACT** | "What breaks if I change this?" | `analyze_impact`, `trace_path`, `get_call_hierarchy`, `get_hot_paths`, `review_pr` |
| **QUALITY** | "Where is the code smelly?" | `solid_audit`, `detect_drift`, `detect_god_functions`, `detect_long_parameter_lists`, `get_complexity` |
| **SAFE CHANGE** | "Mutate safely with guardrails" | `safe_refactor`, `generate_contract`, `validate_contract`, `retrieve_and_verify`, `reparse_on_edit` |
| **CODE IO** | "Read or write code" | `read_file`, `edit_file`, `write_file`, `list_files`, `get_file_symbols` |
| **INFRASTRUCTURE** | "Query IaC" | `iac_query` |

Experimental tools (`ask_about_code`, `find_pattern_by_intent`,
`graph_suggest_questions`, `slice_*`, `cfg_per_function`,
`dominators_cfg`, `taint_flow`) are real but not part of the
recommended flows below. Use them when you have a specific need
that no stable tool covers.

## Read vs mutate (UX guardrail)

| Effect | Tools |
|--------|-------|
| **READ / ANALYZE** | everything in DISCOVER, NAVIGATE, GRAPH, IMPACT, QUALITY, INFRASTRUCTURE |
| **MUTATE** | `edit_file`, `write_file`, `safe_refactor`, `generate_contract` |

Default to READ. Only mutate when the user's request explicitly
requires a code change. After any mutation, run
`reparse_on_edit` to incrementally reindex.

## Tool composition — the four canonical flows

### PROJECT DISCOVERY

```text
project_overview(level=quick)
   ↓
project_insights
   ↓
codebase_map(format=compact)   ← only if context is tight
```

Use this when entering an unfamiliar repo. Each call is small and
fast.

### ARCHITECTURE DEEP DIVE

```text
build_graph                    ← prerequisite for most graph tools
   ↓
graph_insights                 ← health overview
   ↓
graph_communities              ← clusters
   ↓
graph_community_detail         ← per-cluster
   ↓
graph_explain (per symbol)     ← deep dive
```

If a tool returns "graph not built", call `build_graph` first.
Do NOT call `check_architecture` and assume e77 behaviour — see
"Architecture warnings" below.

### CHANGE IMPACT

```text
analyze_impact(symbol_name=...)    ← risk level + impacted files
   ↓
get_call_hierarchy(...)             ← callers/callees
   ↓
trace_path(source=..., target=...)  ← call paths
   ↓
review_pr(changed_files=...)        ← when reviewing a PR
```

### SAFE REFACTOR

```text
analyze_impact                      ← scope the change
   ↓
generate_contract(file_path=..., function_name=...)
                                    ← capture current behaviour
   ↓
safe_refactor(action=..., target=...)
                                    ← preview + apply
   ↓
validate_contract                   ← verify post-conditions
   ↓
reparse_on_edit(file_paths=[...])   ← reindex incrementally
```

This is the only path that should mutate code through the MCP. Do
not use `edit_file` or `write_file` to "fix" things — use
`safe_refactor`.

**Important.** The parameter names above match the runtime
schemas — `symbol_name`, `source`, `target`, `action`, `file_path`,
`function_name`, `file_paths`. If you call these tools with the
"natural" name (e.g. `symbol` instead of `symbol_name`), the call
fails with "missing field". When in doubt, query the tool's
`inputSchema` via `tools/list`.

## Prerequisites — read these once

Most graph tools require `build_graph` first. Some require
`build_lightweight_index` instead. The runtime metadata
(`_meta.requires_graph`) tells you which.

If you call a graph tool and it errors with "graph not built" or
returns an empty result:

1. Call `build_graph` with the project directory.
2. Re-issue your tool call.
3. If still empty, try a subdirectory rather than `.`.

**Known runtime quirk (configuration-dependent, e84.1 WU18
follow-up).** In the default MCP configuration, `build_graph`
then any graph reader (`graph_explain`, `graph_communities`,
`graph_pagerank`, etc.) work in the same process — both share the
in-memory graph cache (`CachedGraphStore` fallback wrapping
`analysis_service.graph_cache()`).

When the runtime is configured with a **persistent SQLite-backed
`GraphStore`** (via `HandlerContext::with_graph_store`), only the
**manifest** is persisted by `build_graph`. The graph itself is
loaded by the manifest-driven cache path on the **next**
`build_graph` call. In that configuration, a fresh `graph_explain`
immediately after `build_graph` may return "No call graph
available".

Workarounds for the persistent configuration:

- Call `check_architecture` first — it auto-builds and
  self-loads.
- Or use the Explorer UI for graph-heavy workflows.

The unit test for the default fallback path passes; the persistent
path is unverified.

Do **not** keep retrying `build_graph` in a loop; that won't fix
it.

## Architecture warnings (product-fiction guardrail)

- **`check_architecture` is a Tarjan-SCC cycle detector over the
  call graph.** It detects mutual recursion and circular call
  dependencies. **It is NOT the e77 executable-architecture
  subsystem** — there is no public MCP adapter for e77 today.
- **`graph_insights` and `graph_god_nodes` are empirical graph
  metrics.** They do not encode architectural rules, governance
  policies, or ADR-derived expected architecture.
- **`review_pr` is PR-risk analysis, not PR approval.** There is no
  promotion-authority model in the MCP (e80/e83). The tool returns
  a risk level; humans (or a higher-layer control plane) make
  promotion decisions.

Do not promise these capabilities to the user. If the user asks for
"architecture rules" or "promotion authority", explain that those
are part of the LSI roadmap and not yet in the public MCP.

## Error recovery

When a tool call returns an error:

1. Check `isError` in the response. If `true`, the message is in
   `content[0].text` — read it before retrying.
2. **Operational errors** (graph not built, path not found, file not
   read): re-issue with corrected arguments. These are not bugs.
3. **Schema errors** (wrong parameter name, wrong type): the schema
   is in the tool's `inputSchema`. Re-issue with the right shape.
4. **Cold-cache errors** (`build_graph` returns fewer nodes than
   expected): try a subdirectory, not the repo root.
5. **Genuine tool bugs**: file an issue with the workspace path,
   the tool name, and the `content[0].text` message.

## CLI ↔ MCP routing

Use the **CLI** when:

- The operation is a single, deterministic action
  (`cognicode analyze .`, `cogh latest --json`).
- You are scripting or wiring into CI.
- You want a quick "is this set up?" check (`cogh doctor`,
  `cognicode doctor`).

Use the **MCP** when:

- You need to compose multiple tools to answer one question
  (the four flows above).
- The user wants the agent to **reason** about the answer, not
  just print it.
- You need graph-aware navigation, which the CLI cannot provide.

Use the **Explorer UI** when:

- The user wants to see the graph or browse symbols visually.

The three surfaces share capabilities where applicable (e.g.
`cognicode graph impact` and MCP `analyze_impact` answer the same
question through different transports).

## Versioning

This skill tracks **capability**, not release numbers. If a
capability described here changes:

- The runtime `tools/list` is the source of truth.
- `cogh version` reports the installed version.

There is no version-pinned reference in this skill.

## What this skill does NOT do

- It does not teach you how to **install** CogniCode. Use `cogh`.
- It does not list all 73 tools. Use `tools/list`.
- It does not promise internal CogniCode subsystems (e77
  architecture, LSI facts/evidence, governed promotion) as public
  MCP capabilities.
