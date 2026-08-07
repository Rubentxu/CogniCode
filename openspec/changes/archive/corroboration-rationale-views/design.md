# Design: Corroboration & Rationale Views

## Technical Approach

Five new capabilities wired in three layers:

1. **Backend port + adapter** — Extend `GraphRepository` port with two methods (`edges_by_kind`, `rationale_subgraph`); implement on the in-memory `GenericGraphRepository`.
2. **Backend scoring + endpoint** — Pure scoring function in `cognicode-core` (`provenance_weight`, `edge_score`, `target_score`, `score_subgraph`); new axum route `GET /api/graph/:id/rationale` returning `SubgraphResponse` with `corroboration_scores` map.
3. **Frontend component + styles** — New `RationaleView` React component wrapping `InteractiveGraph` with a dagre TB layout, source-count badge, lazy-loaded corroboration stylesheet.
4. **Named view integration** — `view_load` dispatches on `lens`; `lens="rationale"` calls `build_rationale_graph` and wraps the result in `RationaleViewPayload`.

Data flow: `RationaleView` → SWR hook → `GET /api/graph/:id/rationale` → `build_rationale_graph` → `repo.rationale_subgraph` (BFS multimodal) + `score_subgraph` (corroboration) → `SubgraphResponse` → adapter → cytoscape elements with dagre TB layout.

## Architecture Decisions

| Decision | Choice | Rejected | Rationale |
|----------|--------|----------|-----------|
| Scoring computation | On-the-fly per request (no materialization) | Cached / materialized `corroboration_scores` table | Spec says ≤200ms for depth=2, scoring is O(edges). No migration, no invalidation, simpler v1. |
| Edge kind filter | Fixed allow-list of 4 multimodal kinds | Configurable per-request kind list | Spec is rationale-only; adding config would be a YAGNI escape hatch. Hard-coded in port method. |
| Direction | Always bidirectional | `?direction=incoming/outgoing/both` | Rationale reads both ways: `Code → Justifies → Decision → Cites → Doc` AND `Doc → CorroboratedBy → Decision`. A direction filter would be a misleading UX surface. |
| Score formula | `min(1.0, Σ bucket_max_score_per_provenance)` | Naive `Σ(provenance_weight × confidence)` per edge | Bucket-max prevents a single source with many edges from inflating the score; matches the "independent sources" intuition. |
| Layout algorithm | `cytoscape-dagre` (separate from elkjs) | elkjs `layered` | Spec requires dagre TB; elkjs layered is more general but slower and produces different rankdir. Separate worker is clean. |
| Style isolation | Lazy-loaded `corroboration.stylesheet.ts` merged on mount | Merged into global `stylesheet.ts` | Other views (ContextualPanel, SvgGraph) don't need corroboration classes; bundle size and rule-count stay lean. |
| Named-view surface | `lens` field in saved row dispatches in `view_load` | New `view_load_rationale` tool | Surface stays at 28 tools; back-compat for non-rationale rows preserved. Single dispatch table. |
| `view_load` envelope | New `RationaleViewPayload` wrapper | Reuse `ContextualView` | Different shape (`subgraph` + scores + source_count) does not fit `ContextualView`'s `focusNode/parent/children/sameLevel` contract. Distinct DTO is honest. |
| Max depth cap | 5 (hard cap, silent clamp from saved value) | Reject `max_depth > 5` with 400 | Saved rows may have legacy `max_depth` values; silently clamping preserves UX. Log line surfaces the change. |

## Data Flow

```
Browser                                            Server
───────                                            ──────
RationaleView (focusNodeId="A")
  │
  ├─ useRationaleGraph("A", {maxDepth:3, maxNodes:50})
  │     │
  │     └─ apiGet("/api/graph/A/rationale?max_depth=3&max_nodes=50", subgraphResponseSchema)
  │                                                       │
  │                                                       ├─ axum: rationale_handler
  │                                                       │     ├─ RationaleParams::validate()
  │                                                       │     ├─ validate_id(id)
  │                                                       │     └─ service.build_rationale_graph(focus, 3, 50)
  │                                                       │           │
  │                                                       │           ├─ repo.rationale_subgraph(focus, 3, 50)  ← BFS multimodal
  │                                                       │           │     └─ GenericGraphRepository
  │                                                       │           │           └─ edges_by_kind(node, [Justifies, Cites, Resolves, CorroboratedBy])
  │                                                       │           │
  │                                                       │           └─ corroboration::score_subgraph(nodes, edges)
  │                                                       │                 ├─ edge_score(e) per edge
  │                                                       │                 └─ target_score(target, edges) per target
  │                                                       │
  │                                                       └─► SubgraphResponse { nodes, edges, truncated, truncation_reason, corroboration_scores }
  │
  ├─ toCytoscapeElements(restNodes, restEdges, restScores)
  │     ├─ For each edge: data.score_band, data.source_count_band
  │     └─ For focus node: data.confidence_band
  │
  ├─ InteractiveGraph (layout="dagre")
  │     │
  │     └─ layout.dagre.worker.ts → cytoscape-dagre { rankdir: TB, nodesep: 50, ranksep: 80 }
  │
  └─ cytoscape.mount(cy)
        └─ cy.style().fromJson([...existing, ...corroboration.stylesheet])
        └─ cy.on('tap', 'node') → onSelectObject(id)
```

### Named view save / load

```
view_save (lens="rationale", focus="A", max_depth=3)
  └─► PG INSERT INTO named_views (lens='rationale', focus_node='A', max_depth=3, ...)

view_load (id=I)
  └─► SELECT * FROM named_views WHERE id=I AND workspace=? AND owner=?
  └─► match lens {
        "rationale" => load_rationale_view(I, scope)    ← NEW
                       └─► build_rationale_graph(focus, min(max_depth, 5), 50)
                       └─► wrap in RationaleViewPayload
        _           => existing contextual_view(focus, lens)   ← preserved
      }
```

## File Changes

| File | Action | Lines (est.) | Description |
|------|--------|:---:|-------------|
| `crates/cognicode-core/src/domain/services/corroboration.rs` | Create | ~140 | Pure scoring functions + 12 unit tests |
| `crates/cognicode-core/src/domain/services/corroboration/tests.rs` | Create | ~180 | RED tests for the module |
| `crates/cognicode-core/src/domain/services/mod.rs` | Modify | +2 | Export `corroboration` |
| `crates/cognicode-explorer/src/ports/graph_repository.rs` | Modify | +40 | Add `edges_by_kind` + `rationale_subgraph` to trait |
| `crates/cognicode-explorer/src/adapters/generic_graph_repository.rs` | Modify | +120 | Implement both port methods + RED tests |
| `crates/cognicode-explorer/src/dto.rs` | Modify | +30 | `corroboration_scores` field on `SubgraphResponse`; new `RationaleViewPayload` |
| `crates/cognicode-explorer/src/service.rs` | Modify | +80 | `build_rationale_graph` + `load_rationale_view` + RED tests |
| `crates/cognicode-explorer/src/api.rs` | Modify | +90 | `rationale_handler` + route + `RationaleParams` + RED tests |
| `crates/cognicode-explorer/src/api_rationale_tests.rs` | Create | ~280 | 14 RED tests for the endpoint + query param validation |
| `crates/cognicode-explorer/src/mcp.rs` | Modify | +60 | `view_load` dispatch branch + `load_rationale_view` + RED tests |
| `apps/explorer-ui/src/api/schemas.ts` | Modify | +15 | Extend `subgraphResponseSchema` with `corroboration_scores`; add `rationaleViewPayloadSchema` |
| `apps/explorer-ui/src/api/client.ts` | Modify | +12 | Add `fetchRationale` |
| `apps/explorer-ui/src/api/client.test.ts` | Modify | +30 | 2 RED tests |
| `apps/explorer-ui/src/components/InteractiveGraph/adapter.ts` | Modify | +45 | Emit `score_band` / `source_count_band` / `confidence_band` |
| `apps/explorer-ui/src/components/InteractiveGraph/adapter.test.ts` | Modify | +60 | 3 RED tests for score mapping |
| `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx` | Modify | +30 | Accept `layout: "dagre" \| ...` prop |
| `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.test.tsx` | Modify | +25 | 2 RED tests for dagre layout wiring |
| `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts` | Modify | +20 | Add 3 confidence-band node classes |
| `apps/explorer-ui/src/components/InteractiveGraph/corroboration.stylesheet.ts` | Create | ~80 | 3 score-band + 3 source-count edge classes |
| `apps/explorer-ui/src/components/InteractiveGraph/corroboration.stylesheet.test.ts` | Create | ~110 | 6 RED tests for the corroboration styles |
| `apps/explorer-ui/src/components/InteractiveGraph/layout.dagre.worker.ts` | Create | ~70 | cytoscape-dagre worker |
| `apps/explorer-ui/src/components/InteractiveGraph/layout.dagre.worker.test.ts` | Create | ~100 | RED tests for TB rankdir |
| `apps/explorer-ui/src/components/RationaleView/RationaleView.tsx` | Create | ~180 | Top-level component |
| `apps/explorer-ui/src/components/RationaleView/RationaleView.test.tsx` | Create | ~200 | 5 RED tests |
| `apps/explorer-ui/src/components/RationaleView/useRationaleGraph.ts` | Create | ~50 | SWR hook |
| `apps/explorer-ui/src/components/RationaleView/useRationaleGraph.test.ts` | Create | ~100 | 4 RED tests |
| `apps/explorer-ui/src/components/RationaleView/RationaleView.module.css` | Create | ~40 | Layout classes |
| `apps/explorer-ui/src/components/RationaleView/index.ts` | Create | ~5 | Barrel |
| `apps/explorer-ui/src/components/Shell.tsx` | Modify | +25 | Add route + panel slot |
| `apps/explorer-ui/src/mocks/fixtures.ts` | Modify | +50 | 1 rationale fixture |
| `apps/explorer-ui/src/mocks/handlers.ts` | Modify | +25 | MSW handler for `/api/graph/:id/rationale` |
| `apps/explorer-ui/package.json` | Modify | +2 | Add `cytoscape-dagre` |

**Total**: ~32 files, ~2050 lines new, ~250 lines modified (~2300 total). Exceeds 400-line PR budget → chained PRs (see tasks §"Work Unit Forecast").

## Interfaces / Contracts

### Rust — port trait extension

```rust
// crates/cognicode-explorer/src/ports/graph_repository.rs
#[async_trait]
pub trait GraphRepository: Send + Sync {
    // ... existing methods ...

    #[cfg(feature = "multimodal")]
    async fn edges_by_kind(
        &self,
        node: &NodeId,
        kinds: &[EdgeKind],
    ) -> Result<Vec<GraphEdge>, ExplorerError>;

    #[cfg(feature = "multimodal")]
    async fn rationale_subgraph(
        &self,
        focus: &NodeId,
        max_depth: u32,
        max_nodes: usize,
    ) -> Result<(Vec<GraphNode>, Vec<GraphEdge>), ExplorerError>;
}
```

### Rust — DTO extension

```rust
// crates/cognicode-explorer/src/dto.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgraphResponse {
    pub root: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated_reason: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub corroboration_scores: HashMap<String, f64>,  // ← NEW
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RationaleViewPayload {  // ← NEW
    pub subgraph: SubgraphResponse,
    pub corroboration_scores: HashMap<String, f64>,
    pub source_count: u32,
}
```

### Rust — scoring module

```rust
// crates/cognicode-core/src/domain/services/corroboration.rs
pub fn provenance_weight(p: &Provenance) -> f64 { /* exhaustive match */ }

pub fn edge_score(edge: &GraphEdge) -> f64 {
    (provenance_weight(&edge.provenance) * edge.confidence).clamp(0.0, 1.0)
}

pub fn target_score(target: &NodeId, edges: &[GraphEdge]) -> f64 {
    let mut buckets: HashMap<&Provenance, f64> = HashMap::new();
    for e in edges.iter().filter(|e| &e.target == target) {
        let s = edge_score(e);
        buckets.entry(&e.provenance).and_modify(|cur| *cur = cur.max(s)).or_insert(s);
    }
    buckets.values().sum::<f64>().clamp(0.0, 1.0)
}

pub fn score_subgraph(_nodes: &[GraphNode], edges: &[GraphEdge]) -> HashMap<String, f64> {
    edges.iter()
        .filter(|e| !e.id.is_empty())
        .map(|e| (e.id.clone(), edge_score(e)))
        .collect()
}
```

### TypeScript — zod schemas

```typescript
// apps/explorer-ui/src/api/schemas.ts
export const subgraphResponseSchema = z.object({
  root: z.string(),
  nodes: z.array(graphNodeSchema),
  edges: z.array(graphEdgeSchema),
  truncated: z.boolean(),
  truncated_reason: z.string().nullable().optional(),
  corroboration_scores: z.record(z.number().min(0).max(1)).default({}),  // ← NEW
});

export const rationaleViewPayloadSchema = z.object({  // ← NEW
  subgraph: subgraphResponseSchema,
  corroboration_scores: z.record(z.number().min(0).max(1)),
  source_count: z.number().int().nonnegative(),
});
```

### TypeScript — hook

```typescript
// apps/explorer-ui/src/components/RationaleView/useRationaleGraph.ts
export interface RationaleOptions {
  maxDepth?: number;
  maxNodes?: number;
}

export function useRationaleGraph(id: string, opts: RationaleOptions) {
  return useSWR(
    id ? ["/api/graph", id, "rationale", opts] : null,
    ([_p, nodeId, _suffix, params]) =>
      fetchRationale(nodeId, params as RationaleOptions),
    { dedupingInterval: 300, focusThrottleInterval: 1000, revalidateOnFocus: false }
  );
}
```

## Sequence: rationale endpoint request

```
Client                         Axum                       Service                       Repository
──────                         ────                       ───────                       ─────────
GET /api/graph/A/rationale
?max_depth=3&max_nodes=50
       │
       ├──────────────────────► rationale_handler
       │                              │
       │                              ├─ RationaleParams.validate() → Ok(3, 50)
       │                              ├─ validate_id("A") → Ok
       │                              ├─ state.service
       │                              │    .build_rationale_graph("A", 3, 50)
       │                              │           │
       │                              │           ├─────────────► repo.rationale_subgraph("A", 3, 50)
       │                              │           │                        │
       │                              │           │                        ├─ BFS queue: [A]
       │                              │           │                        ├─ depth 0: emit A
       │                              │           │                        ├─ depth 1: edges_by_kind(A, [4 kinds])
       │                              │           │                        ├─ depth 2: edges_by_kind(B, [4 kinds]) …
       │                              │           │                        └─ Ok(([A,B,C,D], [(A,B,J), (B,C,Ci), (C,D,Co)]))
       │                              │           │
       │                              │           ├─ corroboration::score_subgraph(&nodes, &edges)
       │                              │           │     └─ HashMap { "e1": 0.9, "e2": 0.63, "e3": 0.25 }
       │                              │           │
       │                              │           └─► Ok(SubgraphResponse { …, corroboration_scores })
       │                              │
       │                              └─► Json(SubgraphResponse)  (200 OK, application/json)
       │
       ◄──────────────────────────
```

## Sequence: named view rationale load

```
MCP caller                  mcp::dispatch              service::load_rationale_view        repo
──────────                  ─────────────              ───────────────────────────         ────
view_load {id: I, scope}
       │
       ├───────────────────► match TOOL_VIEW_LOAD
       │                          │
       │                          ├─ view_load_impl
       │                          │     │
       │                          │     ├─ repo.fetch_view(I, scope)
       │                          │     │     └─ Ok(NamedView { lens: "rationale", focus: "A", max_depth: 3, … })
       │                          │     │
       │                          │     ├─ lens match:
       │                          │     │     "rationale" → load_rationale_view(I, scope)
       │                          │     │                          │
       │                          │     │                          ├─ multimodal feature check (cfg)
       │                          │     │                          │
       │                          │     │                          ├────────► build_rationale_graph("A", min(3,5)=3, 50)
       │                          │     │                          │           └─ Ok(SubgraphResponse)
       │                          │     │                          │
       │                          │     │                          └─ wrap in RationaleViewPayload
       │                          │     │                                { subgraph, scores, source_count = edges.len() }
       │                          │     │
       │                          │     └─ Ok(McpResultEnvelope { ok: true, payload: RationaleViewPayload })
       │                          │
       │                          └─► serialize via ok_direct
       │
       ◄──────────────────── Ok(RationaleViewPayload)
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|---------|
| Unit (Rust) | `provenance_weight`, `edge_score`, `target_score`, `score_subgraph` | 12 unit tests in `corroboration.rs` (pure functions, no fixtures) |
| Unit (Rust) | `edges_by_kind` dedup, `rationale_subgraph` BFS, cycle handling, truncation | 9 unit tests in `generic_graph_repository.rs` using in-memory `HashMap` store |
| Integration (Rust) | `rationale_handler` query validation, id validation, response shape, feature-gate | 8 axum-test integration tests in `api_rationale_tests.rs` |
| Integration (Rust) | `view_load` rationale branch, dispatch table, max-depth clamp, multimodal feature gate | 6 mcp integration tests + 2 mcp regression tests in `mcp.rs` |
| Service (Rust) | `build_rationale_graph` end-to-end + `load_rationale_view` happy/sad paths | 6 service tests in `service.rs` |
| Unit (TS) | `fetchRationale` URL encoding, error mapping, zod parse | 2 Vitest tests in `client.test.ts` |
| Unit (TS) | `toCytoscapeElements` score / source-count / confidence-band bucketing | 3 Vitest tests in `adapter.test.ts` |
| Unit (TS) | corroboration stylesheet classes (presence + values) | 6 Vitest tests in `corroboration.stylesheet.test.ts` |
| Unit (TS) | dagre worker TB layout positions | 4 Vitest tests in `layout.dagre.worker.test.ts` |
| Hook (TS) | `useRationaleGraph` fetch, error, dedup, query encoding | 4 Vitest tests in `useRationaleGraph.test.ts` |
| Component (TS) | `RationaleView` render, click, debounce, error banner, mount/unmount styles | 9 RTL + MSW tests in `RationaleView.test.tsx` + `InteractiveGraph.test.tsx` |

**Totals**: 33 Rust tests + 30 TypeScript tests = 63 new tests. Regression budget: existing 531 Rust + 216 Vitest = 747 tests must stay green.

## TDD Sequence (RED gates)

**Batch 1 — Backend port + endpoint (24 tests)**
1. `provenance_weight` exhaustive match (compile test)
2. `edge_score` weight × confidence + clamp
3. `target_score` bucket-max + sum + clamp
4. `score_subgraph` map keys + empty handling
5. `edges_by_kind` filter + dedup + empty kinds short-circuit
6. `rationale_subgraph` BFS depth, max_depth=0, max_nodes truncation, cycle termination
7. `rationale_handler` query validation (depth / max_nodes out of range)
8. `rationale_handler` id validation (empty / oversized)
9. `rationale_handler` default params, content-type
10. `SubgraphResponse` serde backward-compat + new field presence
11. `RationaleViewPayload` serde round-trip

**Batch 2 — Named view dispatch (15 tests)**
12. `view_load` lens="rationale" returns `RationaleViewPayload`
13. `view_load` lens="callgraph" still returns `ContextualView` (regression)
14. `view_load` multimodal feature off with rationale lens → `lens_rationale_requires_multimodal_feature`
15. `view_load` max_depth > 5 clamped to 5 + `tracing::info!`
16. `view_load` scope mismatches preserved (3 tests)
17. `view_load` feature-gate-off preserved
18. `view_load` schema unchanged
19. `tool_schemas_list_twentyeight_tools` regression
20. `ExplorerService::load_rationale_view` happy + 3 sad paths
21. `build_rationale_graph` embeds scores

**Batch 3 — Frontend component + styles (24 tests)**
22. `fetchRationale` URL encoding + 404
23. `subgraphResponseSchema` zod round-trip with new field
24. `rationaleViewPayloadSchema` zod round-trip
25. `toCytoscapeElements` score_band / source_count_band / confidence_band
26. `corroboration.stylesheet.ts` 6 rules present + values
27. dagre worker TB positions root at top
28. `InteractiveGraph` accepts `layout: "dagre"` and registers extension
29. `useRationaleGraph` fetch + error + dedup + query encoding
30. `RationaleView` mount render + focus card + empty state + error banner
31. `RationaleView` click dispatches `onSelectObject`
32. `RationaleView` mount applies corroboration styles, unmount removes them

## Migration / Rollout

No data migration required. All changes are additive:

1. `cognicode-core` gains the `corroboration` module — pure functions, no DB impact.
2. `cognicode-explorer` port trait gains 2 methods (feature-gated) — existing implementations of `GraphRepository` continue to compile because the methods are default-implemented to return `Err(FeatureDisabled)` in non-`multimodal` builds.
3. `dto.rs` extends `SubgraphResponse` with an `#[serde(default)]` field — existing JSON consumers are unaffected.
4. New axum route is mounted under the existing `Router` chain — no prefix change.
5. `view_load` gains a `match` arm in its dispatch — existing lenses (`callgraph`, etc.) follow the unchanged path.
6. Frontend `RationaleView` is a new component, lazily loaded — no global stylesheet change.
7. `corroboration.stylesheet.ts` is a new file, lazy-loaded with the component — other views are unaffected.

**Rollback**: Remove the route + component + trait methods + DTO field. No existing code depends on the new symbols.

## Open Questions

- [ ] Should the source-count badge also be visible on non-focus nodes? (Locked: focus only in v1, can extend in v2)
- [ ] Should `view_load` accept a `lens` override for ad-hoc switching? (Locked: no — lens always from saved row)
- [ ] Do we want a separate MCP tool `view_load_rationale` for the rationale case? (Locked: no — single `view_load` with dispatch, surface stays at 28 tools)
- [ ] Is the `min(1.0, …)` clamp the right normalization, or should we use a soft cap (e.g., `1 - exp(-x)`)? (Locked: hard clamp in v1; revisit if user feedback shows inflation)
