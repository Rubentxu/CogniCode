# Design: Visualization Stack (Cytoscape.js + elkjs + REST Endpoint)

## Technical Approach

Add three new capabilities as independent modules that integrate at the Shell boundary:

1. **REST endpoint** (`GET /api/graph/:id/subgraph`) — axum route returning Cytoscape-ready JSON with style-class derivation
2. **Web Worker** (`layout.worker.ts`) — elkjs running off-main-thread via comlink, exposing `layout()`, `cancel()`, `onProgress()`
3. **InteractiveGraph component** — React component wrapping Cytoscape.js, consuming the endpoint via `apiGet` + zod, routing layout to the worker

Data flows: `apiGet → SubgraphResponse (zod) → adapter → Cytoscape.ElementsDefinition → worker.layout() → positioned elements → Cytoscape.render`. Shell gains a 4th column at ≥1440px desktop.

## Architecture Decisions

| Decision | Choice | Rejected | Rationale |
|----------|--------|----------|-----------|
| Worker bundling | Vite native `new Worker(new URL(...), {type:'module'})` | comlink `wrap` + webpack worker-loader | Vite 5+ handles `?worker` imports; no extra config. comlink only for RPC surface, not bundling. |
| Style class taxonomy | `function` / `module` / `external` on nodes; `edge.calls` / `edge.implements` / `edge.uses` on edges | Cytoscape inline styles | Class-based enables CSS-like stylesheet, theming, and the selection state machine (`selected` / `highlighted` / `dimmed`) without re-painting. |
| Shell integration | 4th column at ≥1440px; overlay on tablet | Replace LensPanel, split panel | Non-destructive extension. SvgGraph remains fallback for ≤50 nodes. |
| Schema location | `schemas.ts` — `graphNodeSchema`, `graphEdgeSchema`, `subgraphResponseSchema` | Separate `graph-schemas.ts` | All existing schemas live in one file. One module = one import boundary. |
| Backend style derivation | Free function `fn style_class_for(kind: &str) -> &'static str` in `api.rs` | Separate `style` module | Single helper, used in one handler. Avoids over-modularising a 10-line mapping. |

## Data Flow

```
Browser                                    Server
───────                                    ──────
InteractiveGraph
  │
  ├─ apiGet("/api/graph/:id/subgraph", subgraphResponseSchema, {depth, direction, max_nodes})
  │         │                                         │
  │         │◄──── JSON {nodes[], edges[], truncated} ─┤
  │         │                                         │
  ├─ adapter(restNodes, restEdges) → cytoscape.ElementsDefinition
  │
  ├─ worker.layout(elements, {algorithm: "layered"})
  │         │
  │         │◄──── positioned elements ────────────────┤
  │
  ├─ cy.mount(container)
  │
  └─ cy.on('tap', 'node') → onSelectObject(node.id())
```

### Selection state machine

```
click(node) → node.addClass('selected')
            → incidentEdges.addClass('highlighted')
            → non-incident nodes/edges.addClass('dimmed')
clear()      → removeClass('selected', 'highlighted', 'dimmed')
```

## File Changes

| File | Action | Lines (est.) | Description |
|------|--------|:---:|-------------|
| `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx` | Create | ~180 | Cytoscape wrapper with mount, selection, a11y fallback table |
| `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.test.tsx` | Create | ~150 | 12 RED tests from spec |
| `apps/explorer-ui/src/components/InteractiveGraph/adapter.ts` | Create | ~40 | REST DTO → Cytoscape elements mapper |
| `apps/explorer-ui/src/components/InteractiveGraph/adapter.test.ts` | Create | ~60 | Adapter unit tests |
| `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts` | Create | ~60 | Cytoscape style-class taxonomy + visual rules |
| `apps/explorer-ui/src/components/InteractiveGraph/layout.worker.ts` | Create | ~70 | elkjs + comlink worker |
| `apps/explorer-ui/src/components/InteractiveGraph/layout.worker.test.ts` | Create | ~120 | 15 RED tests from spec |
| `apps/explorer-ui/src/components/InteractiveGraph/index.ts` | Create | ~5 | Barrel export |
| `apps/explorer-ui/src/components/Shell.tsx` | Modify | +25 | Add 4th column grid at ≥1440px, React.lazy import |
| `apps/explorer-ui/src/components/viewport.ts` | Modify | +5 | Add `"ultrawide"` viewport tier (≥1440px) |
| `apps/explorer-ui/src/api/schemas.ts` | Modify | +35 | Add `graphNodeSchema`, `graphEdgeSchema`, `subgraphResponseSchema` |
| `apps/explorer-ui/src/api/types.ts` | Modify | +6 | Re-export new types |
| `apps/explorer-ui/src/api/client.ts` | Modify | +10 | Add `fetchSubgraph()` helper |
| `apps/explorer-ui/src/mocks/fixtures.ts` | Modify | +80 | 3 graph fixtures (small 10-node, medium 50-node, large 200-node) |
| `apps/explorer-ui/src/mocks/handlers.ts` | Modify | +25 | MSW handler for `GET /api/graph/:id/subgraph` |
| `apps/explorer-ui/package.json` | Modify | +4 | Add `cytoscape`, `@types/cytoscape`, `elkjs`, `comlink` |
| `crates/cognicode-explorer/src/api.rs` | Modify | +60 | New route + `SubgraphQuery` params + `style_class_for` fn |
| `crates/cognicode-explorer/src/api.rs` | Modify | +40 | Handler `subgraph()` + error mapping |
| `crates/cognicode-explorer/src/dto.rs` | Modify | +30 | `GraphNode`, `GraphEdge`, `SubgraphResponse` structs |
| `crates/cognicode-explorer/src/error.rs` | Modify | +8 | Add `InvalidQuery`, `SymbolNotFound`, `GraphUnavailable` variants |
| `crates/cognicode-explorer/src/api_graph_tests.rs` | Create | ~200 | 19 RED tests from spec |

**Total**: ~21 files, ~1200 lines new, ~120 lines modified.

## Interfaces / Contracts

### Rust DTOs (`dto.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub style_class: String, // "function" | "module" | "external"
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation_type: String,
    pub style_class: String, // "edge.calls" | "edge.implements" | "edge.uses"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgraphResponse {
    pub root: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated_reason: Option<String>,
}
```

### Zod schemas (`schemas.ts`)

```typescript
export const graphNodeSchema = z.object({
  id: z.string(),
  label: z.string(),
  kind: z.string(),
  style_class: z.enum(["function", "module", "external"]),
  metadata: z.record(z.unknown()),
});

export const graphEdgeSchema = z.object({
  id: z.string(),
  source: z.string(),
  target: z.string(),
  relation_type: z.string(),
  style_class: z.enum(["edge.calls", "edge.implements", "edge.uses"]),
});

export const subgraphResponseSchema = z.object({
  root: z.string(),
  nodes: z.array(graphNodeSchema),
  edges: z.array(graphEdgeSchema),
  truncated: z.boolean(),
  truncated_reason: z.string().nullable().optional(),
});
```

### Worker surface (`layout.worker.ts`)

```typescript
export interface LayoutApi {
  layout(elements: ElementsDefinition, options: LayoutOptions): Promise<ElementsDefinition>;
  cancel(): void;
  onProgress(cb: (progress: number) => void): void;
}

export interface LayoutOptions {
  algorithm: "layered" | "force" | "radial";
  width?: number;
  height?: number;
  nodeSeparation?: number;
  rankSeparation?: number;
  iterations?: number;
  animate?: boolean;
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit (Rust) | `style_class_for`, query param validation, truncation logic, error mapping | `#[tokio::test]` with mock `SymbolRepository` — 19 RED tests |
| Unit (TS) | `adapter.ts` mapping, zod schema round-trip, worker `layout()`/`cancel()` | Vitest + jsdom — 15 RED tests for worker, 12 for component |
| Integration | `GET /api/graph/:id/subgraph` end-to-end | `axum::test` with `ExplorerService` mock — covered in Rust unit layer |
| Component | InteractiveGraph renders, selection, a11y fallback | RTL + MSW — 12 RED tests |
| MSW | Handler returns correct fixtures for valid/error requests | Vitest assert on `HttpResponse.json` shape |

## TDD Sequence (RED gates)

**Phase 1 — Rust backend (19 tests)**
1. `style_class_for` unit tests: Function→`function`, Module→`module`, External→`external`, unknown→`function`+warn
2. Query param validation: depth out of range (400), direction invalid (400), max_nodes overflow (400)
3. Handler success: valid id → 200 + `SubgraphResponse` matching schema
4. Truncation: reachable > max_nodes → `truncated: true`, `truncated_reason: "node_cap"`
5. Error cases: empty id (400), missing symbol (404), graph not ready (503)
6. Edge integrity: every edge source/target in nodes set

**Phase 2 — Worker (15 tests)**
1. `layout()` returns positioned elements for 10-node graph
2. Unknown algorithm rejects with `InvalidLayoutOption`
3. `cancel()` rejects in-flight with `LayoutCancelled`
4. `animate: true` emits monotonic `[0..1]` ending at 1.0
5. `animate: false` emits exactly one `1.0`
6. >500 nodes with `animate: false` rejects with `LayoutTooLarge`
7. Worker recovers after cancellation

**Phase 3 — Frontend (12 tests)**
1. Renders `data-testid="interactive-graph"` for valid data
2. Renders `data-testid="interactive-graph-empty"` for null/empty
3. Click dispatches `onSelectObject(id)` once
4. Selected node gets `selected` class, incident edges `highlighted`
5. Non-selected get `dimmed` class
6. Clearing `selectedId` removes all three classes
7. Unknown `style_class` falls back to `function` + `console.warn`
8. `role="application"` on container
9. Fallback table `role="complementary"` with all nodes
10. Tab reaches graph container
11. Enter/Space activates focused node
12. `React.lazy` code-splitting works (bundle check)

## Migration / Rollout

No migration required. The endpoint is a new route; the component is a lazy-loaded addition to Shell. To rollback: remove the `InteractiveGraph` import from Shell, revert viewport to 3-tier, delete the endpoint route. SvgGraph stays untouched.

## Open Questions

- [ ] Should the `ultrawide` viewport tier (≥1440px) also apply to the tablet lens overlay, or remain desktop-only?
- [ ] `ExplorerService.subgraph()` needs a graph traversal method on `SymbolRepository` — is `traverse_bfs(root, depth, direction)` available or must we add a new port method?
