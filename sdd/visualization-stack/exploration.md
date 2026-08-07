## Exploration: Visualization Stack (Cytoscape.js + elkjs + D3.js)

### Current State

The CogniCode Explorer already has a React 19 + TypeScript + Tailwind 4 frontend at `apps/explorer-ui/`. It is a Vite-built SPA that connects to the axum REST API at `crates/cognicode-explorer/src/api.rs` (10 endpoints, served at `127.0.0.1:8010`). The dev server proxies `/api/*` to the backend.

**What exists today:**
- 3-panel Shell layout (MillerColumns | ObjectInspector | LensPanel), responsive with 3 breakpoints
- SWR-based data fetching with zod schema validation at the boundary
- MSW mock handlers for all 11 endpoints — full offline development possible
- SvgGraph component (`apps/explorer-ui/src/components/SvgGraph/SvgGraph.tsx`) — renders small pre-computed call graphs as interactive SVG with pan/zoom/selection
- The SvgGraph is layout-agnostic: it consumes a `LayoutResult` (nodes with x, y pre-computed by a mock `POST /api/diagrams/layout`)
- Spotter search via cmdk (command palette)
- Workspace management, object inspector, lens panel, quality dashboard

**What does NOT exist:**
- No Cytoscape.js, elkjs, or D3.js in `package.json` dependencies
- No graph data API endpoint — the REST API only returns flat `InspectableObjectSummary` data, not graph subgraphs
- No interactive graph engine — the SvgGraph is SVG-only and renders from pre-laid-out positions
- No layout computation on either client or server side
- No named views, no ExplorerQL autocomplete, no C4 projections

**Backend data available today** (24 MCP tools, of which the REST API surfaces a subset):

| MCP Tool                  | REST Equivalent            | What it returns                        |
| ------------------------- | -------------------------- | -------------------------------------- |
| explorer_inspect_object   | GET /api/objects/:id       | Object summary (flat)                  |
| explorer_get_view         | GET /api/objects/:id/views/:vid | Contextual view (structured)     |
| explorer_get_lenses       | GET /api/objects/:id/lenses | Lens descriptors                      |
| explorer_apply_lens       | GET /api/objects/:id/lenses/:lid | Lens results (flat)               |
| explorer_spotter_search   | GET /api/workspaces/:wid/spotter | Hit list                             |
| graph_subgraph            | **NO REST ENDPOINT YET**   | Subgraph DTO with nodes + edges       |
| graph_cluster             | **NO REST ENDPOINT YET**   | SCC / connected components            |
| graph_explain             | **NO REST ENDPOINT YET**   | Path explanation between two nodes    |
| impact_radius             | **NO REST ENDPOINT YET**   | Reverse BFS symbol list               |
| impact_forward_radius     | **NO REST ENDPOINT YET**   | Forward BFS symbol list               |
| impact_shortest_path      | **NO REST ENDPOINT YET**   | Path between symbols                  |
| explorer_query_moldql     | **NO REST ENDPOINT YET**   | Free-form query results               |

### Affected Areas

- `apps/explorer-ui/package.json` — add `cytoscape`, `elkjs`, `d3` (and type packages)
- `apps/explorer-ui/src/components/SvgGraph/` — either extract interface for reuse or replace with Cytoscape
- `apps/explorer-ui/src/components/Shell.tsx` — add graph panel as a viewport-aware pane (desktop 4th column or replaces ObjectInspector when graph is primary view)
- `apps/explorer-ui/src/api/client.ts` — add graph-data endpoint fetchers
- `apps/explorer-ui/src/api/schemas.ts` — add zod schemas for graph data (nodes, edges, subgraphs)
- `apps/explorer-ui/src/mocks/handlers.ts` — add MSW handlers for graph endpoints
- `apps/explorer-ui/src/mocks/fixtures.ts` — add realistic graph fixture data
- `crates/cognicode-explorer/src/api.rs` — add `/api/graph/*` endpoints (subgraph, cluster, explain, path)
- `crates/cognicode-explorer/src/mcp.rs` — (optional) bridge MCP graph primitives to REST responses
- `crates/cognicode-explorer/src/dto.rs` — add REST-facing DTOs for graph data (shaped for frontend consumption)
- `crates/cognicode-explorer/src/service.rs` — add `subgraph()`, `cluster()`, `explain_path()` service methods if not already present

### Approaches

#### 1. Client-side layout: elkjs in the browser, backend sends raw graph data

The backend sends nodes and edges (IDs, labels, kinds, edge types, confidence). The frontend runs `elkjs` in the browser to compute positions, then feeds the result to Cytoscape.js for rendering.

- **Pros:**
  - Backend stays simple — just serialize graph data
  - Layout is responsive to viewport size (elkjs can recompute on resize)
  - No server CPU for layout computation
  - elkjs Web Worker support avoids main-thread blocking
- **Cons:**
  - Payload size: raw graph data is larger than pre-laid-out positions
  - Initial render needs layout computation (latency on first paint)
  - elkjs bundle size (~200KB) added to frontend
  - Layout quality depends on client-side tuning, not server-side caching
- **Effort:** Medium

#### 2. Server-side layout: backend computes positions with a Rust layout engine (or calls elkjs via node)

The backend runs layout computation (in Rust via a layout algorithm, or via a sidecar process), returns positions in the graph response. The frontend just renders with Cytoscape.js.

- **Pros:**
  - Smaller payload (positions pre-computed)
  - Layouts can be cached per named view (per `visualization-stack.md` performance note)
  - Consistent layout regardless of client
  - Frontend is purely a renderer — simpler code
- **Cons:**
  - Adds server-side complexity (no Rust-native elkjs equivalent)
  - Layout quality from Rust layout algorithms (Dagre port, etc.) may not match elkjs
  - Server CPU cost for every layout request
  - Cannot adapt layout to viewport size
- **Effort:** High

#### 3. Hybrid: client-side for interactive graph, server-side for named views

Use `elkjs` in the browser for the main interactive graph (the "you are in a graph" model). For named views and C4 projections (which are opened, not explored), the server caches pre-computed layouts. The frontend requests a named view's layout once.

- **Pros:**
  - Best of both: interactive exploration gets responsive layouts, shared views get cached quality
  - Matches the performance note in `visualization-stack.md`: "the team caches layouts per named view"
  - Gradual implementation: start with client-side, add server caching later
- **Cons:**
  - Two code paths for layout (more testing surface)
  - Cache invalidation for named views when the graph changes
- **Effort:** Low for MVP (client-side only), Medium for full hybrid

### Recommendation

**Approach 3 (Hybrid) with an MVP starting client-side only.**

The rationale:
1. `visualization-stack.md` explicitly says "`elkjs` produces a layout. Cytoscape.js consumes the layout and renders." — this is a client-side pipeline.
2. The roadmap Phase 3 says "Implement the contextual view renderer on top of the view model, with Cytoscape.js as the main renderer and `elkjs` for layouts."
3. The existing `SvgGraph` component already demonstrates the pattern: it receives pre-computed `LayoutResult` and renders. We evolve this to Cytoscape.js consuming `elkjs` output.
4. Client-side layout avoids creating a Rust layout engine (no native elkjs equivalent) and keeps the Rust crates focused on graph algorithms, not visualization.
5. Named-view layout caching can be added later when the named-view mechanism exists (Phase 3 task "Implement the named view store").

**MVP scope for this change:**

| What                              | Library        | Where                       |
| --------------------------------- | -------------- | --------------------------- |
| Interactive graph component       | Cytoscape.js   | `apps/explorer-ui/src/components/CytoscapeGraph/` |
| Hierarchical/C4 layouts           | `elkjs`        | `apps/explorer-ui/src/lib/layout.ts` (Web Worker) |
| Graph data API endpoint           | axum (Rust)    | `GET /api/graph/:id/subgraph` |
| Graph data DTO (nodes + edges)    | serde (Rust)   | `crates/cognicode-explorer/src/dto.rs` |
| Cytoscape styles (node/edge CSS)  | Cytoscape.js   | `apps/explorer-ui/src/lib/graph-styles.ts` |
| Pan/zoom/fit interaction model    | Cytoscape.js   | Built-in (replaces SvgGraph custom pan/zoom) |

**Deferred to later phases:**
- D3.js heatmaps and analytic views (Phase 3, after interactive graph)
- Named views and cached layouts (Phase 3)
- React Flow for editor-shaped surfaces (if needed)
- Mermaid export from named views

### Mapping 24 MCP Tools to UI Interactions

The 24 MCP tools serve the agent-facing surface. The UI-facing surface maps a subset to REST endpoints and user interactions:

| User Action                              | REST Endpoint (to add) | Backing MCP/Method          |
| ---------------------------------------- | ---------------------- | --------------------------- |
| Click node → show subgraph               | GET /api/graph/:id/subgraph?depth=3 | graph_subgraph / service.subgraph() |
| Right-click → "Show impact radius"       | GET /api/graph/:id/impact?direction=incoming | impact_radius / service.impact_radius() |
| Select two nodes → "Find path"           | GET /api/graph/path?from=X&to=Y | impact_shortest_path / service.shortest_path() |
| "Show clusters" button                   | GET /api/graph/clusters?method=scc | graph_cluster / service.cluster_components() |
| Spotter search → select result → inspect | (existing) GET /api/objects/:id | explorer_inspect_object |
| Lens selector → apply lens               | (existing) GET /api/objects/:id/lenses/:lid | explorer_apply_lens |
| ExplorerQL query bar                     | (new) POST /api/query | explorer_query_moldql |
| View tabs (callers/callees/etc.)         | (existing) GET /api/objects/:id/views/:vid | explorer_get_view |

### Relationship Between ExplorerQL and Visualization

ExplorerQL is the query language that drives the data shown in views. The relationship:

1. **Query → Subgraph → Render**: A user runs `FROM auth::login CALLERS DEPTH 2` → backend returns a subgraph DTO → frontend renders with Cytoscape.js + elkjs.
2. **Curated questions → Pre-compiled ExplorerQL**: Each curated question in `target-product-model.md` maps to an ExplorerQL expression. The "Suggested questions panel" in the UI fires these queries.
3. **Lenses are ExplorerQL + style rules**: A lens is an ExplorerQL query that filters/aggregates the graph + a style mapping for Cytoscape.js rendering.
4. **Named views are saved ExplorerQL + layout cache**: A named view stores the query, the focused node, and the elkjs layout positions (cached).

### Risks

- **elkjs bundle size (~200KB)**: Mitigated by loading it lazily only when the graph panel is active. Web Worker off-thread layout prevents jank.
- **Cytoscape.js learning curve**: The library has a rich API (collections, selectors, events). Mitigated by encapsulating inside a single `CytoscapeGraph` React component with a clean prop interface.
- **No Rust-native elkjs equivalent**: If server-side layout is ever needed (for named-view caching), we'd need to call a sidecar or port a layout algorithm. Mitigated by keeping layout client-side for MVP.
- **Graph data API shape mismatch**: MCP subgraph DTOs are shaped for agents. The UI needs DTOs shaped for rendering (with labels, kinds, confidence for styling). Mitigated by adding REST-specific DTOs in the API layer.
- **SvgGraph migration**: The existing SvgGraph component has tests and accessibility features. Mitigated by keeping SvgGraph as a fallback renderer for simple graphs and adding CytoscapeGraph alongside it, not replacing.

### Ready for Proposal

**Yes.** The decision documents (`visualization-stack.md`, `roadmap.md`, `core-mcp-boundaries.md`) provide unambiguous guidance. The codebase exploration confirms the gaps and the path to close them. The orchestrator can proceed to `sdd-propose` with:

1. Scope MVP: interactive graph with Cytoscape.js + elkjs (client-side layout), graph data REST endpoint, one view (dependency graph at code level).
2. Defer: D3.js analytics, named views, ExplorerQL autocomplete, Mermaid export.
3. First implementation task: add `npm install cytoscape elkjs @types/cytoscape` and create the `CytoscapeGraph` component consuming mock graph data.
