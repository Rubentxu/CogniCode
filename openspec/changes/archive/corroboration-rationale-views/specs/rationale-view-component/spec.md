# rationale-view-component Specification (NEW)

## Purpose

A new React component, `RationaleView`, that fetches a rationale sub-graph from `GET /api/graph/:id/rationale` and renders it via the existing `InteractiveGraph` with a **dagre top-to-bottom** layout. The component reuses the existing multimodal node / edge stylesheet (decision = diamond, doc = round-octagon, issue = triangle, evidence = ellipse) and adds corroboration-intensity rendering on top (edge thickness proportional to score, source-count badge on the focus). It is the consumer-facing surface for `rationale-traversal` and `corroboration-styling`, and is integrated with `named-views-rationale` for save / load.

## Files

| File | Change |
|------|--------|
| `apps/explorer-ui/src/components/RationaleView/RationaleView.tsx` | New: top-level React component |
| `apps/explorer-ui/src/components/RationaleView/RationaleView.test.tsx` | New: 9 RED tests (RTL + MSW) |
| `apps/explorer-ui/src/components/RationaleView/useRationaleGraph.ts` | New: SWR hook wrapping `fetchRationale` |
| `apps/explorer-ui/src/components/RationaleView/useRationaleGraph.test.ts` | New: 4 RED tests |
| `apps/explorer-ui/src/components/RationaleView/RationaleView.module.css` | New: 4 layout classes |
| `apps/explorer-ui/src/components/RationaleView/index.ts` | New: barrel export |
| `apps/explorer-ui/src/api/schemas.ts` | Extend `subgraphResponseSchema` with `corroboration_scores: z.record(z.number().min(0).max(1)).default({})` |
| `apps/explorer-ui/src/api/client.ts` | Add `fetchRationale(id, params): Promise<SubgraphResponse>` |
| `apps/explorer-ui/src/api/client.test.ts` | +2 tests for the new fetcher |
| `apps/explorer-ui/src/components/InteractiveGraph/adapter.ts` | Extend `toCytoscapeElements` to read `corroboration_scores` and emit `data.score` per edge |
| `apps/explorer-ui/src/components/InteractiveGraph/adapter.test.ts` | +3 tests for score mapping |
| `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx` | Accept `layout: "dagre" \| "elk-layered" \| "elk-force" \| "elk-radial"` prop (new); pass through to the worker |
| `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.test.tsx` | +2 tests for dagre layout wiring |
| `apps/explorer-ui/src/components/Shell.tsx` | Add `RationaleView` as a 5th panel column at ≥1440px (or a route at `/explorer/rationale/:id`) |
| `apps/explorer-ui/src/mocks/fixtures.ts` | +1 rationale fixture (5 nodes, 6 edges, 2 with corroboration scores) |
| `apps/explorer-ui/src/mocks/handlers.ts` | +1 MSW handler for `GET /api/graph/:id/rationale` |
| `apps/explorer-ui/package.json` | +`cytoscape-dagre` (MIT, well-maintained) |

## Requirements

### Requirement: `RationaleView` component contract

`RationaleView` MUST be a React component with props `{ focusNodeId: string; maxDepth?: number; maxNodes?: number; onSelectObject?: (id: string) => void }`. It MUST fetch the rationale sub-graph on mount and on every change of `focusNodeId`. It MUST render three regions: (1) a `FocusCard` showing the focus node's id, kind, label, and corroboration summary (`sources: N • avg: 0.78`), (2) a Cytoscape canvas with the rationale sub-graph, (3) an `EmptyState` placeholder when the response is empty / unknown focus. The component MUST debounce rapid `focusNodeId` changes via SWR's `dedupingInterval: 300ms`. The component MUST be `React.lazy`-friendly (no top-level side effects, named export only).

#### Scenario: Renders focus card + graph for valid response

- GIVEN a successful 200 with 3 nodes and 4 edges
- WHEN `RationaleView` mounts with `focusNodeId="A"`
- THEN the focus card displays `id="A"` AND the cytoscape container has `data-testid="rationale-graph"` AND `data-node-count="3"`

#### Scenario: Empty / unknown focus renders placeholder

- GIVEN the endpoint returns `{ nodes: [focus], edges: [] }` (focus unknown, 1 node, 0 edges)
- WHEN `RationaleView` mounts
- THEN the placeholder `data-testid="rationale-empty"` is shown AND no cytoscape canvas is rendered

#### Scenario: Error from endpoint shows retry banner

- GIVEN the endpoint returns 500
- WHEN `RationaleView` mounts
- THEN a banner with `data-testid="rationale-error"` and text "Failed to load rationale — retry" is rendered

#### Scenario: Click on node dispatches callback

- GIVEN a rendered rationale sub-graph with 3 nodes
- WHEN the user taps node `B` on the canvas
- THEN `onSelectObject("B")` is called exactly once

#### Scenario: Debounce on rapid prop change

- GIVEN a focus change from `A` to `B` to `C` within 100ms
- WHEN all three props arrive
- THEN SWR deduplicates to a single in-flight request for `C` AND `data` reflects `C`

### Requirement: `useRationaleGraph` SWR hook

`useRationaleGraph(id: string, opts: RationaleOptions) -> { data, error, isLoading, mutate }` MUST be exported. It MUST call `GET /api/graph/:id/rationale?max_depth=...&max_nodes=...` and return the parsed `SubgraphResponse` (validated against the zod schema). The hook MUST expose `dedupingInterval: 300ms` and `focusThrottleInterval: 1000ms` to avoid refetch storms. On 404, the hook MUST return `data: null` and `error.status: 404` (matching the `contextual-views` hook contract).

#### Scenario: Successful fetch returns parsed data

- GIVEN MSW returns 200 with valid `SubgraphResponse`
- WHEN the hook is invoked with `(A, { maxDepth: 3, maxNodes: 50 })`
- THEN `data` is the parsed object AND `isLoading` flips from `true` to `false`

#### Scenario: 404 propagates as error

- GIVEN MSW returns 404
- WHEN the hook is invoked
- THEN `data` is `null` AND `error.status === 404`

#### Scenario: Query params are encoded

- GIVEN `opts = { maxDepth: 2, maxNodes: 100 }`
- WHEN the hook is invoked
- THEN the URL contains `?max_depth=2&max_nodes=100`

### Requirement: Dagre layout wiring

`RationaleView` MUST pass `layout: "dagre"` to `InteractiveGraph`. The dagre layout MUST be `rankdir: TB` (top-to-bottom), `nodesep: 50`, `ranksep: 80`. The `InteractiveGraph` component MUST accept the new `layout` prop and dispatch to the worker accordingly — for `layout === "dagre"` the worker MUST use `cytoscape-dagre` (NOT elkjs). The dagre worker MUST be a sibling of the existing elkjs worker (`layout.dagre.worker.ts`).

#### Scenario: dagre worker is used when layout="dagre"

- GIVEN `InteractiveGraph` is rendered with `layout="dagre"`
- WHEN the layout is invoked
- THEN the cytoscape instance registers the dagre extension AND the layout extension name equals `dagre` (assert via `cy.ext`.dagre existence)

#### Scenario: dagre TB layout positions root at top

- GIVEN a 3-node vertical chain `A → B → C`
- WHEN the dagre TB layout runs
- THEN `A.position.y < B.position.y < C.position.y` (top-to-bottom flow)

#### Scenario: dagre layout does not regress elkjs

- GIVEN the existing elkjs worker tests
- WHEN the new dagre worker is added
- THEN the elkjs tests still pass AND no new failures are introduced

### Requirement: Corroboration source-count badge

`RationaleView`'s `FocusCard` MUST display a `SourceCountBadge` whose value equals the count of distinct `provenance` values among the edges incident to the focus. The badge MUST be hidden when the count is `0`. The badge MUST use the same color tokens as the existing `ObjectInspector` (no new color palette).

#### Scenario: Badge shows count of distinct sources

- GIVEN a focus with 3 incident edges from 2 distinct prov values (`Manual`, `Extracted`)
- WHEN the focus card renders
- THEN the badge text equals `"2 sources"` AND it has `data-testid="source-count-badge"`

#### Scenario: Badge hidden when no edges

- GIVEN an isolated focus with 0 incident edges
- WHEN the focus card renders
- THEN the badge is NOT in the DOM

#### Scenario: Badge updates on focus change

- GIVEN the focus changes from `A` (2 sources) to `B` (0 sources)
- WHEN the card re-renders
- THEN the badge is removed from the DOM

## TDD RED Gate

These tests MUST be written FIRST and MUST FAIL before any implementation lands:

| Test | File | Asserts |
|------|------|---------|
| `rationale_view_renders_focus_card_and_graph` | `RationaleView.test.tsx` | 3 nodes, focus id visible |
| `rationale_view_renders_empty_state` | `RationaleView.test.tsx` | Placeholder shown |
| `rationale_view_renders_error_banner` | `RationaleView.test.tsx` | 500 → retry banner |
| `rationale_view_click_dispatches_callback` | `RationaleView.test.tsx` | `onSelectObject("B")` fires once |
| `rationale_view_debounces_rapid_focus_change` | `RationaleView.test.tsx` | Only `C` in flight |
| `use_rationale_graph_successful_fetch` | `useRationaleGraph.test.ts` | Data + loading flip |
| `use_rationale_graph_404_propagates` | `useRationaleGraph.test.ts` | `error.status === 404` |
| `use_rationale_graph_query_params_encoded` | `useRationaleGraph.test.ts` | URL has `max_depth=2&max_nodes=100` |
| `use_rationale_graph_dedupes_within_300ms` | `useRationaleGraph.test.ts` | One request for two rapid calls |
| `interactive_graph_accepts_dagre_layout` | `InteractiveGraph.test.tsx` | `cy.ext.dagre` is registered |
| `dagre_worker_positions_root_at_top` | `layout.dagre.worker.test.ts` | `A.y < B.y < C.y` for vertical chain |
| `adapter_emits_score_per_edge` | `adapter.test.ts` | `data.score` mirrors REST map |
| `adapter_unknown_score_falls_back_to_zero` | `adapter.test.ts` | `data.score === 0` for missing key |
| `adapter_passes_through_existing_styles` | `adapter.test.ts` | Multimodal `style_class` preserved |
| `fetch_rationale_encodes_params` | `client.test.ts` | URL + zod parse |
| `fetch_rationale_404_throws_api_error` | `client.test.ts` | `ApiError.status === 404` |

## Out of Scope (locked)

- Virtualization for >200 nodes (deferred — truncation at 200 is enough in v1)
- Animation on layout (deferred — instant layout is fine)
- Custom dagre algorithm options exposed to the user (hard-coded TB / 50 / 80 in v1)
- Rationale diffing between two focuses
- PDF / Mermaid export of the rationale view
- Drag-to-rearrange nodes
- Cross-pane linking (clicking a rationale node to open it in `ContextualPanel`)
