# Proposal: Visualization Stack (Cytoscape.js + elkjs + D3.js)

## Intent

Replace the static `SvgGraph` (pre-computed SVG, no interactivity beyond pan/zoom) with a live interactive graph engine. Users need to explore call graphs, dependency clusters, and impact paths visually — selecting nodes, expanding subgraphs, and applying hierarchical layouts — without leaving the Explorer shell. The current SVG renderer cannot scale beyond ~50 nodes and offers no layout algorithm choice.

## Scope

### In Scope
- InteractiveGraph React component wrapping Cytoscape.js with pan/zoom, selection, and context menus
- elkjs layout engine running in a Web Worker (hierarchical, force-directed, layered)
- `GET /api/graph/:id/subgraph` REST endpoint returning nodes + edges as Cytoscape-ready JSON
- Graph data adapter translating REST DTOs → Cytoscape elements with style mappings
- MSW mock handlers + realistic fixture graphs for offline development

### Out of Scope
- D3.js analytics dashboards (deferred to next change)
- Named views, ExplorerQL autocomplete, C4 projections
- Server-side layout computation
- Mermaid export of graph views

## Capabilities

### New Capabilities
- `interactive-graph`: Cytoscape.js-based graph visualization with elkjs layouts, node/edge styling, and selection-driven navigation

### Modified Capabilities
- None — no existing spec-level behavior changes. The Shell gains a new panel slot, and the REST API gains a new route. Both are pure extensions.

## Approach

Client-side layout (elkjs in Web Worker) keeps Rust crates focused on graph algorithms. The `InteractiveGraph` component follows the same pattern as `SvgGraph`: it consumes a `LayoutResult`-like interface and dispatches `onSelectObject` to the parent. Add to Shell as a 4th panel column on desktop (≥1440px), overlay on tablet.

Backend: add `GET /api/graph/:id/subgraph?depth=N&direction=both` returning `{nodes: GraphNode[], edges: GraphEdge[]}` where each node carries a `style_class` for Cytoscape mapping. No existing routes are touched.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `apps/explorer-ui/src/components/InteractiveGraph/` | New | Cytoscape.js wrapper, layout worker, adapter |
| `apps/explorer-ui/src/components/Shell.tsx` | Modified | Add 4th panel column on desktop |
| `apps/explorer-ui/src/api/schemas.ts` | Modified | Add `GraphNode`/`GraphEdge` zod schemas |
| `apps/explorer-ui/src/api/client.ts` | Modified | Add `fetchSubgraph` fetcher |
| `apps/explorer-ui/src/mocks/` | Modified | Add graph fixtures + MSW handlers |
| `apps/explorer-ui/package.json` | Modified | Add `cytoscape`, `elkjs`, type packages |
| `crates/cognicode-explorer/src/api.rs` | Modified | Add `/api/graph/:id/subgraph` route |
| `crates/cognicode-explorer/src/dto.rs` | Modified | Add `SubgraphResponse` DTO |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Cytoscape.js bundle size (~200KB) pushes Vite build over budget | Medium | Lazy-load `InteractiveGraph` via `React.lazy`; Web Worker for elkjs isolates layout cost |
| Cytoscape.js API learning curve delays implementation | Low | Encapsulate in single component; adapter layer insulates rest of app |
| No server-side layout limits offline/preview use | Low | MSW mocks provide offline layout; server-side caching deferred to hybrid phase |

## Rollback Plan

Remove the `InteractiveGraph` component import from Shell, revert Shell to 3-column grid, delete `/api/graph/:id/subgraph` route. The `SvgGraph` component is NOT removed — it remains as fallback. No data migration needed.

## Dependencies

- `cytoscape@^3.30` + `@types/cytoscape`
- `elkjs@^0.9` + `comlink` (Web Worker RPC)
- Backend: CogniCode graph database must be indexed (existing `MoldQLStore`)

## Success Criteria

- [ ] InteractiveGraph renders 200+ node graphs at >30fps with elkjs layered layout
- [ ] Node selection dispatches `onSelectObject(id)` and highlights callee/caller edges
- [ ] `GET /api/graph/:id/subgraph` returns valid JSON validated by frontend zod schema
- [ ] Existing SvgGraph tests continue passing (no regression)
- [ ] MSW handlers cover subgraph endpoint with ≥3 fixture graphs (small, medium, large)

## Entropy Budget

**Method**: Heuristic (CogniCode unavailable)

| Metric | Estimate | Threshold | Status |
|--------|----------|-----------|--------|
| H(Δ_existing) | 0.5 bits | < 1.0 | ✅ OCP compliant |
| H(Δ_new) | 2.6 bits | — | ✅ Pure extension |
| New connascence pairs | 4 (all ≤1 bit) | < 3 pairs critical | ✅ All Name/Type |
| Max I(A;B) introduced | 1.0 bit | < 3.0 | ✅ Low coupling |

**OCP**: Adding a 4th Shell panel and a new REST route modifies 0.5 bits of existing code — well under the 1.0-bit threshold. All other changes are new files or additive lines in existing files.

**Verdict**: 🟢 Green — low-risk extension. No existing behavior changes. No hidden Meaning connascence.
