## Exploration: Contextual Views

### Current State

The **target product model** (`docs/explorer-graph/target-product-model.md:208-211`) defines a contextual view as:

> "The 'contextual' view of any node is the projection that includes that node, its neighbors at the same level, and its parents and children at adjacent levels. This is the GT-inspired view: a node with the abstractions above and below it, presented in one panel."

The product model defines **4 C4 abstraction levels** (Code, Component, Container, System) and **edge types for climbing levels**: `part_of`, `deployed_as`, `in_system`, `belongs_to`. These edges exist in the model but **NOT in the current implementation** — the call graph today only has `calls`/`called_by` edges plus `lives_in` (symbol→file).

The current rendering stack is:
- **InteractiveGraph** (`apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx`): Cytoscape.js component rendering `SubgraphResponse` from `GET /api/graph/:id/subgraph`. Clicking a node fires `onSelectObject(id)`, the parent applies `selected`/`highlighted`/`dimmed` classes. Layout is `preset`.
- **Shell** (`apps/explorer-ui/src/components/Shell.tsx`): 4-column layout on ultrawide (MillerColumns | ObjectInspector | LensPanel | InteractiveGraph). The graph column reads `activeObjectId` from global state. Selection in the graph is read-only for now — clicks highlight but don't navigate.
- **useSubgraph** hook: fetches from `GET /api/graph/:id/subgraph` with SWR. Defaults: depth=3, direction=both, max_nodes=500.

The backend has **two separate "view" concepts**:
1. **`ContextualView` DTO** (`dto.rs:153-166`): text-based view with blocks, relations, evidence, findings. Served by `GET /api/objects/:id/views/:view_id` and `explorer_get_view` MCP tool. Used in ObjectInspector panel.
2. **`SubgraphResponse` DTO** (`dto.rs:481-498`): graph-based view with nodes and edges. Served by `GET /api/graph/:id/subgraph` and `graph_subgraph` MCP tool. Used in InteractiveGraph panel.

The **named-views system** stores `(level, lens, focus_node, max_depth)` tuples in PostgreSQL. `view_load` rebuilds a `ContextualView` by calling `contextual_view(focus_node, lens)` — no separate graph projection is stored.

### Affected Areas

- **`apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx`** — the Cytoscape component. Would need to support a new data shape (contextual panel data with parents/children sections), or a companion panel component.
- **`apps/explorer-ui/src/components/Shell.tsx`** — the layout. A contextual panel is a new column/overlay, or replaces/augments the InteractiveGraph column.
- **`crates/cognicode-explorer/src/api.rs`** — the subgraph handler (`build_subgraph`). Would need a new endpoint or an extension for multi-level traversal.
- **`crates/cognicode-explorer/src/dto.rs`** — `SubgraphResponse` and `GraphNode`/`GraphEdge`. A contextual response would need a richer shape (parent/child sections).
- **`crates/cognicode-explorer/src/service.rs`** — `contextual_view()` method and `view_focus_mvp_id()`. The named-views `load_view` delegate calls this.
- **`crates/cognicode-explorer/src/mcp.rs`** — `graph_subgraph` tool. Could be extended or a new `graph_contextual` tool added.
- **`crates/cognicode-explorer/src/domain/views.rs`** — existing view builders (overview, callgraph, source, quality, file/scope). No multi-level builder exists yet.
- **`docs/explorer-graph/target-product-model.md`** — the definition of contextual views (lines 208-211) and the abstraction levels table (lines 147-163).
- **`docs/explorer-graph/visualization-stack.md`** — the decision table for new views. A contextual view is an "interactive navigable graph" → should use Cytoscape.js.

### Approaches

#### 1. Full backend — new endpoint `/api/graph/:id/contextual`
Build a new REST endpoint that traverses the graph at multiple levels: BFS within the same level (call edges), plus one hop up (via `part_of`/`belongs_to`/`lives_in`) and one hop down (reverse). Returns a structured `ContextualGraphResponse` with sections for focus, same-level neighbors, parents, and children.

- **Pros**: Clean separation. Purpose-built for the UI. The frontend gets exactly the data it needs in one request.
- **Cons**: The C4 edge types (`part_of`, `deployed_as`, `in_system`, `belongs_to`) **do not exist** in the current call graph implementation. Building this endpoint without those edges would produce a misleading or incomplete result. Requires significant backend infrastructure first.
- **Effort**: High (requires implementing multi-level edges in the call graph first)

#### 2. Frontend composition — multiple existing API calls + client-side assembly
The contextual panel makes 3 calls: `GET /api/graph/:id/subgraph` (same-level neighbors), `GET /api/objects/:id/views/overview` (focus node details), and `GET /api/objects/:file_id/views/symbols` (siblings in the same file). The frontend composes the response into a contextual-view shape.

- **Pros**: No new backend endpoint. Works with existing data. Shows real architectural context (file containment IS available via `lives_in` and `find_symbols_by_file`).
- **Cons**: N+1 pattern — 3 requests per selection. Complex client-side assembly logic. The hierarchical parent/child relationships are limited to file-scope (no component/container/system levels).
- **Effort**: Medium

#### 3. Phased delivery (RECOMMENDED)
**Phase 1 (v1)**: UI panel component + existing data
- Build a `ContextualPanel` React component that sits alongside or replaces the InteractiveGraph column
- For the **focused node**: use the existing ObjectInspector's `inspect_object` data or the subgraph's root node
- For **same-level neighbors**: use the existing SubgraphResponse (callers/callees at same depth)
- For **parents**: use the `lives_in` file reference (every symbol has a `file` field). Resolve the file as a parent node.
- For **children** (symbols in the same file): use `find_symbols_by_file` via a new REST endpoint or derive from graph state
- The panel layout: focus card at top, "Same level" section (minigraph of neighbors), "Parent" breadcrumb, "Children" list

**Phase 2 (v2)**: C4 edges + full multi-level contextual view
- Implement `part_of`, `belongs_to`, `deployed_as`, `in_system` edges in the call graph/explorer backend
- Add a `GET /api/graph/:id/contextual` endpoint that leverages these edges for true multi-level traversal
- Replace the Phase 1 best-effort file-scope approach with real C4 traversal

- **Pros**: Delivers value in Phase 1 without waiting for C4 edge infrastructure. Phase 1 provides real file-scope context that is already available. Phase 2 upgrades to full architectural context. Each phase independently useful.
- **Cons**: Phase 1 contextual view is architecturally incomplete (no Component/Container/System levels). Phase 2 requires backend work that may take weeks. Two release cycles.
- **Effort**: Phase 1: Medium. Phase 2: High.

### Recommendation

**Approach 3 — Phased delivery.**

Phase 1 is feasible today because:
- Every symbol node carries a `file` field — the `lives_in` relationship is already materialized
- `find_symbols_by_file` exists on the `SymbolRepository` trait — sibling symbols in the same file are queryable
- The `view_focus_mvp_id` function already maps `level` strings to MVP ID prefixes (`function`→`symbol:`, `file`→`file:`, `scope`→`scope:`)
- The named-views level field (`function`, `module`, `scope`) maps naturally to the first two C4 levels

Phase 1 is NOT the full GT-inspired view from the product model, but it provides immediate architectural awareness: "this function lives in this file, alongside these other symbols, and calls/gets called by these neighbors."

### View Model Shape (Phase 1)

```typescript
// New DTO for the contextual graph panel
interface ContextualGraphResponse {
  focusNode: GraphNode;
  level: "function" | "file" | "scope";
  
  // Same-level neighbors from BFS call graph
  sameLevel: {
    nodes: GraphNode[];
    edges: GraphEdge[];
    truncated: boolean;
  };
  
  // Parent: the file or scope containing this node
  parent: {
    node: GraphNode;       // the file or scope
    edge: GraphEdge;       // "lives_in" or "belongs_to" edge
  } | null;
  
  // Children: symbols/files inside this node (if this is a file or scope)
  children: {
    nodes: GraphNode[];
    edges: GraphEdge[];    // "contains" edges
    truncated: boolean;
  } | null;
}
```

Phase 2 would extend this with `parents` (plural, multi-level chain) and `children` at all C4 levels.

### Risks

- **Phase 1 limitation**: Without Component/Container/System edges, the contextual view only shows one level of architectural context (symbol↔file). Users expecting the full GT-inspired view will find it incomplete.
- **API chattiness**: If Phase 1 uses frontend composition, 3 requests per node click could feel slow. Mitigation: add a single `GET /api/graph/:id/contextual?level=file` endpoint that bundles the same data in one request.
- **Naming collision**: The existing `ContextualView` DTO (text-based, from `dto.rs`) and a new `ContextualGraphResponse` (graph-based) would cause confusion. Recommendation: name the new one `ContextualGraphResponse` or `GraphContextView` to disambiguate.
- **InteractiveGraph coupling**: The existing `InteractiveGraph` is tightly coupled to `SubgraphResponse`. Adding a new data shape may require refactoring the component or building a new `ContextualGraph` component. Mitigation: keep the InteractiveGraph for the main graph, add a new `ContextualPanel` component for the contextual view.

### Ready for Proposal

**Yes** — with the caveat that the proposal should explicitly scope Phase 1 to file-level context and defer C4 multi-level traversal to Phase 2. The orchestrator should present the user with:

1. Confirmation that Phase 1 delivers file-scope contextual views (symbol → file parent, file siblings, call neighbors)
2. The Phase 2 dependency on C4 edge infrastructure (`part_of`/`belongs_to`/`deployed_as`/`in_system`) being implemented first
3. The decision on API approach for Phase 1: new bundled endpoint vs. frontend composition of existing endpoints

### Entropy Analysis (Connascence Landscape)

**Method**: Heuristic

| Component A | Component B | Connascence Type | I(bits) | Severity |
|-------------|-------------|------------------|---------|----------|
| InteractiveGraph.tsx | SubgraphResponse DTO | Type | 1.6 | ⚠️ Medium |
| Shell.tsx | InteractiveGraph.tsx | Meaning | 0.8 | ⚠️ Low |
| api.rs (build_subgraph) | service.rs (symbol_repo) | Name | 1.0 | ⚠️ Low |
| mcp.rs (graph_subgraph) | ImpactAnalysisService | Type | 1.0 | ⚠️ Low |
| New ContextualPanel | SubgraphResponse + file symbols | Type | ~2.0 | ⚠️ Medium |

**Critical Pairs (I > 3.0 bits)**: None  
**Hidden Connascence (Meaning)**: The InteractiveGraph's `onSelectObject` is wired as a no-op placeholder in Shell.tsx ("Selection is read-only in this column for now"). Any contextual panel that navigates on click must coordinate with the global state — the existing comment signals intent, but the contract is undocumented.  
**SOLID-Entropy Violations**: None detected at exploration stage.  
**Recommendation**: Accept current coupling levels. The Type connascence between InteractiveGraph and SubgraphResponse is expected — they are a consumer/producer pair. Adding a new ContextualPanel will introduce 1-2 new connascence pairs with the SubgraphResponse or a new endpoint.

**Coupling Score**: H_external ≈ 1.3 bits (within acceptable range)
