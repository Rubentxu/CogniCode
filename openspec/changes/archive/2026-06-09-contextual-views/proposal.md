# Proposal: Contextual Views (Phase 1)

## Intent

Provide file-level architectural context for any graph node: the focused symbol, its same-level call neighbors, its containing file (parent), and sibling symbols in that file (children). Delivers the GT-inspired contextual projection from the target product model, scoped to data that exists today (`lives_in` + call graph), without C4 abstraction edges.

## Scope

### In Scope
- **New DTO** `ContextualGraphResponse`: focusNode + sameLevel (nodes/edges) + parent (node/edge|null) + children (nodes/edges|null) + level
- **New REST endpoint** `GET /api/graph/:id/contextual?level=file&depth=1&max_nodes=200` — single bundled call for all contextual data
- **New `ExplorerService` method** `build_contextual_graph` — resolves symbol, fetches same-level BFS neighbors, resolves `lives_in` parent, queries `find_symbols_by_file` for siblings
- **New React component** `ContextualPanel` — renders focus card, neighbor minigraph, parent breadcrumb, children list using Cytoscape.js
- **New hook** `useContextualGraph` — SWR fetch from the contextual endpoint
- **Shell layout update** — ContextualPanel as optional replacement/supplement to InteractiveGraph column

### Out of Scope
- C4 abstraction edges (`part_of`, `belongs_to`, `deployed_as`, `in_system`)
- Multi-level traversal (Component/Container/System)
- Persisted named contextual views
- MCP `graph_contextual` tool (deferred to Phase 2)

## Capabilities

### New Capabilities
- `contextual-graph`: Backend endpoint + DTO for file-level contextual graph projection. Returns focus node with parent (file via `lives_in`), children (siblings via `find_symbols_by_file`), and same-level call neighbors (BFS via existing `callers`/`callees`).

### Modified Capabilities
- None at spec level. The existing `graph-subgraph` endpoint and `SubgraphResponse` DTO remain unchanged.

## Approach

**Bundled backend endpoint** (not frontend composition). A single `GET /api/graph/:id/contextual` collects all contextual data server-side, avoiding N+1 client requests. The endpoint reuses existing SymbolRepository methods (`resolve`, `callers`, `callees`, `find_symbols_by_file`) — no new infrastructure needed. Frontend receives a purpose-built `ContextualGraphResponse` and renders via a new `ContextualPanel` component using the existing Cytoscape.js wrapper patterns from `InteractiveGraph`.

Naming: `ContextualGraphResponse` avoids collision with the existing text-based `ContextualView` DTO.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/dto.rs` | New | `ContextualGraphResponse`, `ContextualGraphNode`, `ContextualGraphEdge` |
| `crates/cognicode-explorer/src/api.rs` | New | `contextual_handler` route + `build_contextual_graph` logic |
| `crates/cognicode-explorer/src/service.rs` | New | `build_contextual_graph()` method |
| `apps/explorer-ui/src/components/ContextualPanel/` | New | React component + Cytoscape integration |
| `apps/explorer-ui/src/hooks/useContextualGraph.ts` | New | SWR data-fetching hook |
| `apps/explorer-ui/src/components/Shell.tsx` | Modified | Add ContextualPanel to layout (single column addition) |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Users expect full C4 multi-level views; find file-scope incomplete | Medium | Clear "Phase 1" labeling in UI, roadmap notes |
| `find_symbols_by_file` performance for large files (1000+ symbols) | Low | `max_nodes` cap, truncated=true signal |
| `ContextualView` naming confusion with `ContextualGraphResponse` | Low | Disambiguated names; docs reference |

## Rollback Plan

- Revert `api.rs` route registration (single line)
- Delete `ContextualPanel/` and `useContextualGraph` frontend files
- Revert Shell.tsx layout change
- No database migrations; no spec-level rollback needed

## Dependencies

- SymbolRepository trait methods: `resolve`, `callers`, `callees`, `find_symbols_by_file` (all exist)
- `GraphNode`/`GraphEdge` DTOs — reused as-is in `ContextualGraphResponse`
- Cytoscape.js (already in `InteractiveGraph` dependency tree)

## Success Criteria

- [ ] `GET /api/graph/:id/contextual` returns valid `ContextualGraphResponse` with focus, parent, children, and same-level sections
- [ ] `ContextualPanel` renders focused node, neighbor minigraph, parent breadcrumb, and children list
- [ ] Clicking a node in the panel navigates to that node as the new focus
- [ ] Truncation handled gracefully when siblings > max_nodes
- [ ] No regression in existing `graph-subgraph` endpoint or `InteractiveGraph` behavior

## Entropy Budget

**Method**: Heuristic

| Metric | Estimate (bits) | Threshold | Status |
|--------|-----------------|-----------|--------|
| H(Δ_existing) | 0.3 | < 1.0 | ✅ |
| H(Δ_new) | 2.5 | > 0 | ✅ |
| New connascence pairs | 2 | < 3 | ✅ |
| OCP compliant? | Yes | yes | ✅ |

Pure extension: new route + new DTO + new component. Existing handlers unchanged. One Type connascence pair (`ContextualGraphResponse` ↔ `ContextualPanel`) is expected consumer/producer relationship.

**Verdict**: 🟢 Green — low-risk extension. No breaking changes.
