# CogniCode MCP Tools Reference

CogniCode exposes a comprehensive set of MCP (Model Context Protocol) tools that let AI agents inspect, analyze, and navigate a code graph stored in PostgreSQL. This document catalogs every available tool, when to use it, and what it returns.

> **Total tools shipped: 49** (as of v0.19.0)
>
> Tools are grouped by purpose. Each entry lists the canonical tool name (used in `tools/call`), a one-line purpose, key parameters, and a short usage note.

## Conventions

All tools accept a JSON object as arguments and return a JSON-RPC `CallToolResult` with a structured envelope:

```json
{
  "ok": true,
  "data": { /* tool-specific payload */ },
  "meta": {
    "tool": "graph_pagerank",
    "ts": "2026-06-24T12:00:00Z"
  }
}
```

Errors return:

```json
{
  "ok": false,
  "error": {
    "code": "invalid_args",
    "message": "graph_subgraph: invalid `direction` `foo` (expected one of: outgoing, incoming, both)",
    "tool": "graph_subgraph"
  }
}
```

Common error codes: `invalid_args`, `missing_required_arg`, `facade_unavailable`, `service_error`, `not_loaded` (when no workspace is open), `feature_disabled` (multimodal-gated tool without feature flag).

---

## 1. Workspace & Session Management

These tools bind a working directory and create the in-memory state other tools operate on. You must open a workspace before using graph-impact tools.

### `explorer_open_workspace`

Bind a workspace root and load its call graph into memory.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `root_path` | string | no | Filesystem path. Omit to return the currently bound workspace. |

**Use when**: starting a new session, or switching between projects.

**Returns**: the workspace summary (root path, project metadata, indexed file count).

### `brain_open`, `brain_close`, `brain_status`, `brain_focus`, `brain_attach`, `brain_ask`

Brain sessions are persistent conversation contexts bound to a world-view of the graph. They let you iterate with the LLM across multiple tool invocations while preserving history.

- `brain_open` — create a new brain session.
- `brain_attach` — bind a brain to a workspace.
- `brain_ask` — send a natural-language question to the LLM (uses the bound world-view as context).
- `brain_focus` — narrow attention to a subtree (e.g. one component).
- `brain_status` — introspect session state, token usage, attached files.
- `brain_close` — terminate the session.

Plus three space-management tools for organizing knowledge inside a brain:

- `brain_add_space`, `brain_remove_space`, `brain_spaces` — manage named knowledge spaces within a brain session.

### `cognicode_ask`

Single-shot natural-language question against the graph (no persistent brain state).

**Use when**: you need a one-off answer and don't want the overhead of a brain session.

---

## 2. Search & Discovery

### `explorer_spotter_search`

Full-text search across symbols, files, modules, and named entry points. Returns ranked hits.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | yes | Search query. |
| `kind` | string | no | Optional kind filter (e.g. `Function`, `Struct`). |

**Use when**: you don't know the symbol id but have a name, partial name, or keyword.

**Returns**: ranked list of `{ id, kind, name, file, line, score }`.

### `explorer_query_moldql`

Execute a structured MoldQL query against the graph. MoldQL supports subgraph extraction, neighbor traversal, path queries, and cluster queries.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | object | yes | A MoldQL query AST (see `moldql::compile`). |

**Use when**: you need precise structural queries that Spotter's text search can't express — e.g. "all functions in module X that call Y recursively up to depth 3".

---

## 3. Object Inspection

### `explorer_inspect_object`

Resolve an object id (symbol, file, scope, issue, rule, component, container, system, decision, doc, evidence) and return its full summary.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `object_id` | string | yes | The object id to inspect. |

**Use when**: you have an id from `spotter_search`, `graph_subgraph`, or a brain answer and want to know everything about that object.

**Returns**: object metadata + available views + relations.

### `explorer_get_views`, `explorer_get_view`

List the views available for an object (`get_views`) or materialize a specific view (`get_view`).

| Parameter (get_views) | Type | Required | Description |
|-----------|------|----------|-------------|
| `object_id` | string | yes | The object id. |

**Use when**: you want to render a specific visualization (call graph, source, dependency map, etc.).

### `explorer_get_lenses`, `explorer_apply_lens`

List available lenses for an object (`get_lenses`) or apply one (`apply_lens`) — lenses are reusable analyses (e.g. `hotspots`, `dead_code`, `complexity_hotspots`).

**Use when**: you want a typed analysis rather than raw data.

### `lens_find_dead_code`

Find symbols not reachable from any entry point (root symbols). Useful for identifying candidates for deletion. Uses the existing `CallGraph::find_dead_code()` algorithm and supports custom entry points.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `limit` | integer | no | Maximum dead symbols returned (default 50). Total count is always reported. |
| `entry_points` | string[] | no | Explicit entry points. Defaults to graph roots (symbols with no incoming edges). |

**Returns**: `{ total_symbols, total_dead, dead_code_percent, dead_symbols: [...], entry_points }`.

**Use when**: "what code can I safely delete?" or "audit code coverage from entry points".

### `lens_find_intersection`

Run multiple lenses against the same object and return findings that appear in 2+ lenses (cross-cutting concerns that multiple analyses agree on).

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `object_id` | string | yes | Object to analyze. |
| `lens_ids` | string[] | yes | 2-5 lens ids to cross-reference. |
| `min_consensus` | integer | no | Minimum lenses that must agree (default 2). |

**Returns**: `{ findings: [{ finding_id, title, hypothesis, severity, confidence, contributing_lenses }], per_lens_counts }`.

**Use when**: filtering signal from noise — findings confirmed by multiple analyses are higher confidence.

### `lens_hotspots`

Top-N symbols by PageRank across the full graph, relative to an anchoring object. Complements `graph_god_nodes` (subgraph-scoped) by being graph-wide and rank-N.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `object_id` | string | yes | Anchor object (excluded from results). |
| `top_n` | integer | no | Top symbols to return (default 10, max 100). |
| `max_depth` | integer | no | Max BFS depth (default 3, currently unused — full-graph PageRank). |

**Returns**: `{ hotspots: [{ symbol_id, label, pagerank, in_degree, out_degree }], method: "page_rank" }`.

**Use when**: "which symbols should I worry about first?" — surfaces the most depended-upon code.

### `find_dead_code_v2`

Workspace-wide dead-code analysis with a confidence threshold filter. Wraps the internal MCP's `find_dead_code` and adds confidence gating so callers can request only high-confidence dead candidates.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `limit` | integer | no | Maximum entries returned (default 100). |
| `confidence_threshold` | number | no | Minimum confidence score (default 0.0, max 1.0). |

**Returns**: `{ dead_code: [{ symbol_id, kind, file, line, confidence }], total_dead, dead_code_percent, confidence_threshold }`.

**Use when**: you want confidence-filtered dead code (vs `lens_find_dead_code` which is hard-filtered to callable + type definitions).

### `find_cycles`

Find strongly-connected components (cycles) in the graph using Tarjan's SCC algorithm.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `min_scc_size` | integer | no | Minimum SCC size to report (default 2, suppresses singletons). |

**Returns**: `{ cycles: [[symbol_id, ...], ...], total_cycles, longest_cycle_length }`.

**Use when**: "what circular dependencies exist?" — useful for planning refactors.

### `health_dashboard`

Single-call summary of the workspace's graph health. Returns total symbol/edge counts, dead-code percent, cycle count, and a derived health score (0.0–1.0).

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `workspace_id` | string | no | Optional workspace scope. |

**Returns**: `{ symbols: { total, indexed, stale }, edges: { total }, health_score: 0.0..=1.0, findings: [{ title, severity }] }`.

**Use when**: dashboarding, monitoring, or quick project health checks.

---

## 4. Graph Traversal & Impact

These tools walk the call graph and report structural relationships.

### `graph_subgraph`

Extract a bounded subgraph rooted at a symbol.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `root` | string | yes | Symbol id of the subgraph root. |
| `direction` | string | no | `incoming`, `outgoing`, or `both`. Default: `both`. |
| `max_depth` | integer | no | BFS depth. Default: 3. |

**Use when**: you want to render a call graph fragment in the Explorer.

**Returns**: nodes + edges within the bounded region.

### `graph_cluster`

Find clusters (strongly connected components or weakly connected components) in the graph.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `method` | string | no | `scc` (default) or `connected`. |

**Returns**: array of clusters, each as an array of symbol ids.

### `graph_explain`

Find the lowest-cost path between two symbols (or the cheapest explanation for a given edge).

**Returns**: the path with provenance and confidence per hop.

### `impact_radius`

All symbols reachable from a root within `max_depth` edges.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `root` | string | yes | Root symbol. |
| `max_depth` | integer | no | Default: 3. |
| `direction` | string | no | Default: `both`. |

**Use when**: you want to know "if I change X, what else might break?".

### `impact_forward_radius`

Same as `impact_radius` but only outgoing edges.

### `impact_has_path`

Boolean: is there a path from `from` to `to`?

### `impact_shortest_path`

Return the actual shortest path between two symbols.

### `impact_detect_cycles`

Find all strongly-connected components and report those with more than one node as cycles.

### `impact_component`

Return the connected component containing a given symbol.

---

## 5. Graph Analysis (v0.16.0+)

These tools expose the `cognicode-graph-algos` library via MCP. Each operates on a **subgraph** specified by `root`, `depth`, and `direction`. The subgraph is extracted first (using `graph_subgraph` semantics), then the algorithm runs on that subgraph only.

All eight tools share this subgraph spec:

```json
{
  "subgraph": {
    "root": "<symbol id>",
    "depth": 3,
    "direction": "outgoing" | "incoming" | "both"
  }
  // + algorithm-specific options
}
```

### `graph_pagerank`

Compute PageRank scores for every node in the subgraph.

**Options**: `alpha` (default 0.85), `max_iterations` (default 100).

**Returns**: `{ "scores": { "<symbol id>": <score>, ... } }`.

**Use when**: ranking nodes by global importance within a localized region.

### `graph_god_nodes`

Find "god" nodes — symbols whose PageRank places them in the top `percentile` of the subgraph.

**Options**: `percentile` (default 0.95).

**Returns**: `{ "nodes": [{ "id": "<symbol id>", "score": <pagerank> }, ...] }`.

**Use when**: identifying dominant symbols (hot candidates for refactoring or deeper inspection).

### `graph_communities`

Detect communities using Label Propagation — clusters of densely connected symbols.

**Options**: `max_iterations` (default 100).

**Returns**: `{ "communities": [[ "<symbol id>", ... ], ...] }`.

**Use when**: discovering module boundaries or hidden coupling clusters that don't align with the file structure.

### `graph_community_god_nodes`

Find god nodes **per community** — gives you the most important symbol in each detected cluster.

**Options**: `percentile` (default 0.95).

**Returns**: `{ "nodes": [{ "community_index": <int>, "id": "<symbol id>", "score": <pagerank> }, ...] }`.

### `graph_surprising_connections`

Find cross-community edges — connections between symbols in different communities. These are candidates for refactoring (they violate community boundaries).

**Options**: `limit` (default 10).

**Returns**: `{ "edges": [{ "source_id": "...", "target_id": "...", "score": <surprise score> }, ...] }`.

### `graph_transitive_reduction`

Compute the transitive reduction of the subgraph — the minimal edge set that preserves reachability. Removes redundant transitive edges.

**Returns**: `{ "edges": [{ "source_id": "...", "target_id": "..." }, ...] }`.

**Use when**: simplifying a dense call graph for visualization without losing the essential structure.

### `graph_feedback_arc_set`

Find a feedback arc set — a set of edges whose removal breaks all cycles. Useful for planning refactors that convert cyclic dependencies into DAGs.

**Returns**: `{ "edges": [{ "source_id": "...", "target_id": "..." }, ...] }`.

### `graph_all_simple_paths`

Enumerate all simple paths (no repeated nodes) from a `from` symbol to a `to` symbol, bounded by `max_hops`.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `from` | string | yes | Source symbol. |
| `to` | string | yes | Target symbol. |
| `max_hops` | integer | no | Default: 10. |

**Returns**: `{ "paths": [[ "<symbol id>", "<symbol id>", ... ], ...] }`.

**Use when**: understanding all the routes data can take from A to B through the call graph.

---

## 6. Architecture & Drift

### `detect_architecture_drift`

Compare the actual graph structure against a declared architecture (e.g. `expected-architecture.yaml`) and report findings where code diverges from intent.

**Use when**: enforcing C4 boundaries, layer rules, or naming conventions documented in your ADR.

---

## 7. Multimodal Ingestion (feature-gated)

The following tools require the `multimodal` feature flag at build time. They ingest non-code artifacts into the same graph, allowing ADRs, issues, and benchmarks to become first-class navigable objects.

### `docs_ingest`

Ingest markdown documentation files as `Doc` graph nodes, with `Cites` edges linking docs to the symbols they mention.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `paths` | string[] | yes | File paths to ingest. |

### `issues_ingest`

Ingest tracker issues (GitHub, Jira, Linear — adapter-configured) as `Issue` graph nodes with `Resolves` / `Cites` edges.

### `graph_search`

Keyword search across the multimodal graph — symbols, docs, issues, decisions, evidence.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | yes | Search query. |
| `node_kinds` | string[] | no | Filter by node kind (e.g. `["Decision", "Issue"]`). |

---

## 8. View Management

Named views and ViewSpecs let users persist a custom visualization configuration.

### `view_save`

Persist a `ViewSpec` (title, view kind, data source, transform, renderer).

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `view_spec` | object | yes | The ViewSpec to save. |

### `view_load`

Load a previously saved ViewSpec by id.

### `view_list`

List all saved ViewSpecs (filterable by owner or scope).

### `view_delete`

Delete a saved ViewSpec by id.

---

## 9. Brain Spaces (v0.16.0+)

Brain sessions support named knowledge spaces for organizing long-running investigations.

- `brain_add_space` — create a new named space within the current brain.
- `brain_remove_space` — delete a space.
- `brain_spaces` — list all spaces in the current brain.

**Use when**: investigating multiple parallel hypotheses in one session (e.g. "performance regression hypothesis" vs "memory leak hypothesis") without them contaminating each other.

---

## Quick Recipes

### Find the top 10 most important symbols in a module

```json
{
  "tool": "graph_pagerank",
  "arguments": {
    "subgraph": { "root": "sym:crate::auth", "depth": 4 }
  }
}
```

Then sort by score and take the top 10.

### Find which modules break boundaries

```json
{
  "tool": "graph_surprising_connections",
  "arguments": {
    "subgraph": { "root": "sym:crate::", "depth": 3 },
    "limit": 20
  }
}
```

### Refactor: convert cycles to a DAG

```json
{
  "tool": "graph_feedback_arc_set",
  "arguments": {
    "subgraph": { "root": "sym:crate::parser", "depth": 5 }
  }
}
```

The returned edges are candidates for extraction or inversion.

### Visualize a call chain

1. `graph_subgraph` with `direction: "both"` and `max_depth: 2` from a root.
2. Pipe the result to the renderer's graph component.

### Discover and analyze at the same time

1. `explorer_spotter_search` to find a candidate symbol by name.
2. `graph_subgraph` to scope around it.
3. `graph_pagerank` + `graph_communities` to characterize the structure.

---

## Feature-Gating & Build Variants

Some tools are gated behind Cargo feature flags:

| Feature | Gated tools |
|---------|-------------|
| `multimodal` | `docs_ingest`, `issues_ingest`, `graph_search` |

When the server is built without a feature, calling a gated tool returns:

```json
{
  "ok": false,
  "error": {
    "code": "feature_disabled",
    "message": "this tool requires the multimodal feature"
  }
}
```

The default build enables `postgres` but **not** `multimodal`.

---

## Error Handling

All tools return structured errors. Agents should pattern-match on `error.code`:

| Code | Meaning |
|------|---------|
| `invalid_args` | Schema validation failed — check parameter types. |
| `missing_required_arg` | A required parameter was absent. |
| `facade_unavailable` | The backing service is not wired in the current server config. |
| `service_error` | The service call failed — message contains details. |
| `not_loaded` | No workspace is bound — call `explorer_open_workspace` first. |
| `feature_disabled` | Tool requires a Cargo feature that isn't enabled in this build. |
| `invalid_input` | Argument value is semantically wrong (e.g. unknown direction). |

Tools never panic; all errors are caught and returned through the envelope.

---

## Tool Inventory (quick index)

| Family | Tools | Count |
|--------|-------|-------|
| Workspace | `explorer_open_workspace` | 1 |
| Brain (session) | `brain_open`, `brain_close`, `brain_status`, `brain_focus`, `brain_attach`, `brain_ask`, `brain_add_space`, `brain_remove_space`, `brain_spaces` | 9 |
| Single-shot LLM | `cognicode_ask` | 1 |
| Search | `explorer_spotter_search`, `explorer_query_moldql`, `graph_search` | 3 |
| Inspection | `explorer_inspect_object`, `explorer_get_views`, `explorer_get_view`, `explorer_get_lenses`, `explorer_apply_lens` | 5 |
| Graph traversal | `graph_subgraph`, `graph_cluster`, `graph_explain`, `detect_architecture_drift` | 4 |
| Impact | `impact_radius`, `impact_forward_radius`, `impact_has_path`, `impact_shortest_path`, `impact_detect_cycles`, `impact_component` | 6 |
| Graph analysis (v0.16.0) | `graph_pagerank`, `graph_god_nodes`, `graph_communities`, `graph_community_god_nodes`, `graph_surprising_connections`, `graph_transitive_reduction`, `graph_feedback_arc_set`, `graph_all_simple_paths` | 8 |
| Lens analysis (v0.18.0) | `lens_find_dead_code`, `lens_find_intersection`, `lens_hotspots` | 3 |
| Workspace health (v0.19.0) | `find_dead_code_v2`, `find_cycles`, `health_dashboard` | 3 |
| Multimodal ingest | `docs_ingest`, `issues_ingest` | 2 |
| View management | `view_save`, `view_load`, `view_list`, `view_delete` | 4 |
| **Total** | | **49** |

---

## Versioning

This document tracks tools available at the version noted at the top. To discover the actual set at runtime, call the MCP `tools/list` method on the server — the returned `tools` array is the source of truth.

For changelog of which tools were added in which version, see `CHANGELOG.md` or the git tags (`v0.x.y`).
