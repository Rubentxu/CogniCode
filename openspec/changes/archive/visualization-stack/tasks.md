# Tasks: Visualization Stack (Cytoscape.js + elkjs + REST Endpoint)

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~1200 new + ~120 modified (~1320 total) |
| 400-line budget risk | **High** — every concrete phase exceeds 400 lines |
| Chained PRs recommended | **Yes** |
| Suggested split | PR 1 (Backend) → PR 2 (Worker + Schemas) → PR 3 (Component + Shell) |
| Delivery strategy | `ask-on-risk` |
| Chain strategy | `stacked-to-main` |
| Plan format | Linear dependency chain — each PR merges to main before the next starts |

**Decision needed before apply**: **Yes**
**Chained PRs recommended**: **Yes**
**Chain strategy**: **stacked-to-main**
**400-line budget risk**: **High**

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Backend: DTOs + error variants + `style_class_for` + query validation + handler + 19 RED tests | PR 1 | No frontend dep. Touches `dto.rs`, `error.rs`, `api.rs`. ~440 LOC. All Rust tests must pass + `cargo clippy` clean. |
| 2 | Schemas + adapter + worker + 15 worker RED tests | PR 2 | Depends on PR 1 schemas finalised (wire format locked). Touches `schemas.ts`, `types.ts`, `client.ts`, `adapter.ts`, `layout.worker.ts`, fixtures, MSW handlers. ~390 LOC. |
| 3 | Component + Shell wiring + 12 component RED tests | PR 3 | Depends on PR 1 + PR 2. Touches `InteractiveGraph.tsx`, `stylesheet.ts`, `Shell.tsx`, `viewport.ts`, `index.ts`, `package.json`. ~410 LOC. Bundle size check required. |

**Stack strategy rationale**: `stacked-to-main` chosen because slices are linear (each depends on the prior), the team values fast iteration, and the slices land green from the Rust side first — easy to revert individually. `feature-branch-chain` would be heavier without rollback benefit because there is no parallel work to coordinate.

**Stack strategy boundaries**:
- PR 1 base = `main` (no prior slice)
- PR 2 base = `main` AFTER PR 1 merges (worker does NOT import new Rust code, only zod schemas mirrored from PR 1 DTOs)
- PR 3 base = `main` AFTER PR 2 merges (component depends on worker + adapter)

If a child PR diff shows previous slices in the diff, retarget/rebase to `main` after the parent merges.

---

## Phase 0: Pre-flight (must complete before any RED tests)

- [x] **0.1** Run `cargo test -p cognicode-explorer` — confirm baseline Rust suite is green
- [x] **0.2** Run `cd apps/explorer-ui && pnpm test` — confirm baseline Vitest suite is green
- [x] **0.3** Confirm Node version `>= 20` (`node --version`) and `pnpm --version` is available
- [x] **0.4** Skim `crates/cognicode-explorer/src/api.rs` line 1–120 to confirm axum route registration pattern (`Router::new().route("/api/...", get(handler))`)
- [x] **0.5** Read `apps/explorer-ui/src/components/SvgGraph/SvgGraph.tsx` to mirror the `onSelectObject(id)` callback contract

---

## Phase 1: PR 1 — Rust Backend (Graph Data Endpoint) [~440 LOC]

> **Goal**: Ship `GET /api/graph/:id/subgraph?depth=N&direction=...&max_nodes=...` end-to-end with 19 passing tests, 0 frontend changes.

### 1.1 RED tests for `style_class_for` helper (`api.rs`)

- [x] **1.1.1** Create `crates/cognicode-explorer/src/api_graph_tests.rs` with `mod style_class_for` block
- [x] **1.1.2** RED: `Function → "function"`; run `cargo test -p cognicode-explorer style_class_for` → expect compile error (`style_class_for` not defined)
- [x] **1.1.3** RED: `Method → "function"` (case-insensitive check: `function`/`Function`/`fn`)
- [x] **1.1.4** RED: `Module → "module"`; `Crate → "module"`; `Trait → "module"`
- [x] **1.1.5** RED: `External → "external"`; `CrateDep → "external"`
- [x] **1.1.6** RED: `UnknownKind → "function"` + test that `tracing::warn!` was called (use `tracing-test` or capture layer)
- [x] **1.1.7** RED: edge mapping `Calls → "edge.calls"`, `Implements → "edge.implements"`, `Uses → "edge.uses"`, `Imports → "edge.uses"`
- [x] **1.1.8** RED: edge unknown → `"edge.calls"` (default) + warn

### 1.2 GREEN: implement `style_class_for`

- [x] **1.2.1** Add `fn style_class_for(kind: &str) -> &'static str` in `api.rs` (single helper, ~10 lines)
- [x] **1.2.2** Add `fn edge_style_class_for(relation: &str) -> &'static str` next to it
- [x] **1.2.3** Run `cargo test -p cognicode-explorer style_class_for` → 6 edge cases pass
- [x] **1.2.4** Run `cargo clippy -p cognicode-explorer --all-targets -- -D warnings` → clean

### 1.3 RED tests for query param validation

- [x] **1.3.1** In `api_graph_tests.rs` add `mod query_validation` block
- [x] **1.3.2** RED: `depth=0` → 400 with body `{"error": "invalid_query", "message": "depth must be in 1..=10"}`
- [x] **1.3.3** RED: `depth=11` → 400
- [x] **1.3.4** RED: `depth=abc` → 400 (parse error)
- [x] **1.3.5** RED: `direction=sideways` → 400 with allowed-values message
- [x] **1.3.6** RED: `max_nodes=0` → 400
- [x] **1.3.7** RED: `max_nodes=5001` → 400
- [x] **1.3.8** RED: defaults applied: missing `depth` → 3, missing `direction` → `both`, missing `max_nodes` → 500

### 1.4 GREEN: implement `SubgraphQuery` extractor

- [x] **1.4.1** Add `#[derive(Deserialize)] struct SubgraphQuery { depth: Option<u8>, direction: Option<String>, max_nodes: Option<u32> }` in `api.rs`
- [x] **1.4.2** Implement `impl SubgraphQuery { fn validated() -> Result<(u8, Direction, u32), ExplorerError> }` returning `InvalidQuery` on out-of-range
- [x] **1.4.3** Wire into axum handler as `Query<SubgraphQuery>` extractor
- [x] **1.4.4** Run `cargo test -p cognicode-explorer query_validation` → 7 cases pass

### 1.5 RED tests for DTO + handler success path

- [x] **1.5.1** In `api_graph_tests.rs` add `mod handler_success` block
- [x] **1.5.2** RED: valid id `sym:foo::bar` → 200, JSON has `root: "sym:foo::bar"`, `nodes: Vec<GraphNode>`, `edges: Vec<GraphEdge>`, `truncated: false`
- [x] **1.5.3** RED: every returned node has `style_class` ∈ `{"function", "module", "external"}`
- [x] **1.5.4** RED: every returned edge has `style_class` ∈ `{"edge.calls", "edge.implements", "edge.uses"}`
- [x] **1.5.5** RED: `serde_json::from_value::<SubgraphResponse>(response)` round-trips successfully (mirrors zod parse on frontend)
- [x] **1.5.6** RED: edge integrity — every `edge.source` and `edge.target` exists in `nodes[].id`

### 1.6 GREEN: implement DTOs + handler

- [x] **1.6.1** In `dto.rs` add `GraphNode`, `GraphEdge`, `SubgraphResponse` structs (per design §"Rust DTOs"). Add `#[serde(skip_serializing_if = "Option::is_none")]` on `truncated_reason`. (~30 lines)
- [x] **1.6.2** In `error.rs` add 3 variants: `InvalidQuery(String)`, `SymbolNotFound(String)`, `GraphUnavailable(String)`. Map to 400/404/503 in `IntoResponse`. (~8 lines)
- [x] **1.6.3** In `api.rs` add `async fn subgraph_handler(State(service): State<Arc<ExplorerService>>, Path(id): Path<String>, Query(q): Query<SubgraphQuery>) -> Result<Json<SubgraphResponse>, ExplorerError>` (~60 lines)
- [x] **1.6.4** Register route in `Router::new().route("/api/graph/:id/subgraph", get(subgraph_handler))` in `lib.rs` or wherever routes are mounted
- [x] **1.6.5** Run `cargo test -p cognicode-explorer handler_success` → 5 cases pass

### 1.7 RED tests for truncation + error paths

- [x] **1.7.1** In `api_graph_tests.rs` add `mod truncation` block
- [x] **1.7.2** RED: fixture with 600 reachable nodes + `max_nodes=500` → response `truncated: true`, `truncated_reason: Some("node_cap")`, `nodes.len() == 500`
- [x] **1.7.3** RED: truncated response still has all `edge.source`/`edge.target` ∈ `nodes[].id` (no dangling references)
- [x] **1.7.4** RED: empty id `""` → 400 `invalid_id`
- [x] **1.7.5** RED: id with 513 chars → 400 `invalid_id`
- [x] **1.7.6** RED: non-existent symbol id → 404 `symbol_not_found`, no Rust types leaked
- [x] **1.7.7** RED: service in `GraphUnavailable` state → 503 `graph_unavailable`

### 1.8 GREEN: implement truncation + id validation

- [x] **1.8.1** In `api.rs` add `fn validate_id(id: &str) -> Result<&str, ExplorerError>` — non-empty, ≤512 chars
- [x] **1.8.2** In `api.rs` handler, after traversal, check `nodes.len() > max_nodes_usize`, truncate, set `truncated = true`, `truncated_reason = Some("node_cap".into())`
- [x] **1.8.3** Filter `edges` to retain only those whose source+target survived truncation
- [x] **1.8.4** Map `ExplorerError::SymbolNotFound` / `GraphUnavailable` to 404/503 in `IntoResponse` (no `Debug` formatting in body)
- [x] **1.8.5** Run `cargo test -p cognicode-explorer api_graph_tests` → all 19 cases pass
- [x] **1.8.6** Run `cargo fmt --all` + `cargo clippy -p cognicode-explorer --all-targets -- -D warnings` → clean
- [x] **1.8.7** Commit: `feat(explorer): add GET /api/graph/:id/subgraph with style-class derivation` (split into work units: types, helper, handler, tests — see `work-unit-commits` skill)

**PR 1 exit gate**: `cargo test -p cognicode-explorer` green, `cargo clippy` clean, 19 new tests passing, OpenSpec `state.yaml` records PR 1 merged.

---

## Phase 2: PR 2 — Frontend Schemas + Adapter + Worker [~390 LOC]

> **Goal**: Lock the wire format in zod, build the elkjs Web Worker with comlink, ship MSW fixtures. No component yet. Depends on PR 1 DTOs.

### 2.1 RED tests for zod schemas

- [x] **2.1.1** In `apps/explorer-ui/src/api/schemas.test.ts` add `describe("subgraphResponseSchema", ...)` block
- [x] **2.1.2** RED: valid `SubgraphResponse` fixture → `parse()` succeeds
- [x] **2.1.3** RED: node with `style_class: "function"` / `"module"` / `"external"` all parse
- [x] **2.1.4** RED: node with `style_class: "alien"` → parse fails with path `nodes.0.style_class`
- [x] **2.1.5** RED: edge with `style_class: "edge.calls"` / `"edge.implements"` / `"edge.uses"` all parse
- [x] **2.1.6** RED: missing `truncated_reason` field → parse succeeds (it's `optional().nullable()`)
- [x] **2.1.7** RED: `truncated_reason: null` → parse succeeds

### 2.2 GREEN: implement zod schemas

- [x] **2.2.1** In `apps/explorer-ui/src/api/schemas.ts` append `graphNodeSchema`, `graphEdgeSchema`, `subgraphResponseSchema` (per design §"Zod schemas"). (~35 lines)
- [x] **2.2.2** In `apps/explorer-ui/src/api/types.ts` add `export type GraphNode = z.infer<typeof graphNodeSchema>` etc. (+6 lines)
- [x] **2.2.3** Run `pnpm test schemas` → 6 cases pass

### 2.3 RED tests for `client.fetchSubgraph`

- [x] **2.3.1** Create `apps/explorer-ui/src/api/client.test.ts` (if not present) with `describe("fetchSubgraph", ...)` block
- [x] **2.3.2** RED: with MSW returning valid `SubgraphResponse` → resolves to typed `SubgraphResponse`
- [x] **2.3.3** RED: query params `{depth: 2, direction: "incoming", max_nodes: 100}` are encoded as `?depth=2&direction=incoming&max_nodes=100`
- [x] **2.3.4** RED: 404 response → throws `ApiError` with `status: 404` and `code: "symbol_not_found"`

### 2.4 GREEN: implement `fetchSubgraph`

- [x] **2.4.1** In `apps/explorer-ui/src/api/client.ts` add `export async function fetchSubgraph(id: string, params: Partial<SubgraphQuery>): Promise<SubgraphResponse>` using existing `apiGet` pattern. (+10 lines)
- [x] **2.4.2** Run `pnpm test client` → 3 cases pass

### 2.5 Add MSW fixtures + handler

- [x] **2.5.1** In `apps/explorer-ui/src/mocks/fixtures.ts` add `smallSubgraph` (10 nodes, ~12 edges), `mediumSubgraph` (50 nodes, ~75 edges), `largeSubgraph` (200 nodes, ~280 edges). (+80 lines)
- [x] **2.5.2** In `apps/explorer-ui/src/mocks/handlers.ts` add `http.get("/api/graph/:id/subgraph", ...)` returning one of the three fixtures by id prefix. (+25 lines)
- [x] **2.5.3** Run `pnpm test mocks` → handler responds with correct fixture for `id` `small*` / `medium*` / `large*`

### 2.6 RED tests for `adapter.ts`

- [x] **2.6.1** Create `apps/explorer-ui/src/components/InteractiveGraph/adapter.test.ts`
- [x] **2.6.2** RED: `toCytoscapeElements(restNodes, restEdges)` returns `{nodes: [...], edges: [...]}` matching `cytoscape.ElementsDefinition`
- [x] **2.6.3** RED: each cytoscape node carries `data.style_class` mirrored from REST
- [x] **2.6.4** RED: each cytoscape edge has `data.source` / `data.target` matching REST ids
- [x] **2.6.5** RED: empty `nodes`/`edges` arrays → empty `ElementsDefinition`, no throw

### 2.7 GREEN: implement `adapter.ts`

- [x] **2.7.1** Create `apps/explorer-ui/src/components/InteractiveGraph/adapter.ts` with `toCytoscapeElements(nodes, edges): ElementsDefinition`. (~40 lines)
- [x] **2.7.2** Run `pnpm test adapter` → 4 cases pass

### 2.8 RED tests for worker (elkjs + comlink)

- [x] **2.8.1** Create `apps/explorer-ui/src/components/InteractiveGraph/layout.worker.test.ts`
- [x] **2.8.2** RED: `layout(elements, {algorithm: "layered"})` → resolves to positioned elements with `position: {x, y}` on every node
- [x] **2.8.3** RED: unknown algorithm `"neural-net"` → rejects with `Error("InvalidLayoutOption")`
- [x] **2.8.4** RED: `cancel()` while in-flight → original `layout()` promise rejects with `LayoutCancelled`
- [x] **2.8.5** RED: `cancel()` when idle → no-op, does not throw
- [x] **2.8.6** RED: `animate: true` → `onProgress` callback fires with monotonic `[0..1]` ending at 1.0
- [x] **2.8.7** RED: `animate: false` → `onProgress` fires exactly once with `1.0`
- [x] **2.8.8** RED: two `onProgress` subscribers each receive every value
- [x] **2.8.9** RED: >500 nodes with `animate: false` → rejects with `LayoutTooLarge`
- [x] **2.8.10** RED: 200-node layered layout completes in <500ms (use `performance.now()`)
- [x] **2.8.11** RED: after a `cancel()`, a fresh `layout()` call resolves normally (worker recovers)
- [x] **2.8.12** RED: `width`/`height`/`nodeSeparation`/`rankSeparation`/`iterations` options are forwarded to elkjs (assert via `elkjs` mock or option capture)
- [x] **2.8.13** RED: `radial` algorithm produces a non-layered layout (compare node positions against layered reference)
- [x] **2.8.14** RED: `force` algorithm does not throw on a 10-node cyclic graph
- [x] **2.8.15** RED: empty `elements` → resolves to empty `ElementsDefinition` without invoking elkjs

### 2.9 GREEN: implement worker

- [x] **2.9.1** Run `pnpm add cytoscape @types/cytoscape elkjs comlink` (+ save to `package.json`)
- [x] **2.9.2** Create `apps/explorer-ui/src/components/InteractiveGraph/layout.worker.ts` with `comlink.expose({ layout, cancel, onProgress })`. (~70 lines)
- [x] **2.9.3** Implement three algorithms: `layered` (elk `layered`, direction `LR`), `force` (elk `force`), `radial` (elk `radial`)
- [x] **2.9.4** Implement progress streaming: `animate: true` yields each `progress`; `animate: false` yields one 1.0
- [x] **2.9.5** Implement cancellation: `cancel()` sets a flag, the in-flight `elk.layout()` rejects on next tick
- [x] **2.9.6** Implement size guard: if `nodes.length > 500 && !options.animate` → throw `LayoutTooLarge`
- [x] **2.9.7** Run `pnpm test layout.worker` → 15 cases pass

**PR 2 exit gate**: `pnpm test` green, `pnpm lint` clean, 24 new tests (6 schemas + 3 client + 4 adapter + 15 worker − 4 overlap = 24) passing, worker bundle isolated (verified via `pnpm build:check-bundle`).

---

## Phase 3: PR 3 — Component + Shell Wiring [~410 LOC]

> **Goal**: Mount the `InteractiveGraph` component in the Shell 4th column, wire selection to existing `onSelectObject`, ship a11y fallback table. Depends on PR 1 + PR 2.

### 3.1 RED tests for `InteractiveGraph` render + empty state

- [x] **3.1.1** Create `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.test.tsx`
- [x] **3.1.2** RED: with valid `SubgraphResponse` prop → renders `data-testid="interactive-graph"`
- [x] **3.1.3** RED: with `null` or `nodes.length === 0` → renders `data-testid="interactive-graph-empty"` (does not throw)
- [x] **3.1.4** RED: container has `role="application"` and `aria-label="Interactive graph of <root>"`

### 3.2 GREEN: implement render shell

- [x] **3.2.1** Create `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx` with `React.lazy` boundary or direct import of cytoscape. (~40 lines for the mount skeleton)
- [x] **3.2.2** Implement `useEffect` that calls `cytoscape({ container, elements, style, layout: { name: "preset" } })` (preset, real layout comes from worker)
- [x] **3.2.3** Run `pnpm test InteractiveGraph` → 3 cases pass

### 3.3 RED tests for selection state machine

- [x] **3.3.1** RED: `cy.on('tap', 'node')` → calls `onSelectObject(id)` exactly once
- [x] **3.3.2** RED: clicking a node adds class `selected` to that node, `highlighted` to incident edges, `dimmed` to non-incident nodes+edges
- [x] **3.3.3** RED: clearing `selectedId` prop (parent sets to `null`) → all three classes removed from all elements
- [x] **3.3.4** RED: clicking the background (not a node) does NOT call `onSelectObject`
- [x] **3.3.5** RED: unknown `style_class` on a node falls back to `function` visually + `console.warn` called once

### 3.4 GREEN: implement selection + style mapping

- [x] **3.4.1** Add `useEffect` listener `cy.on('tap', 'node', handler)` that calls `props.onSelectObject(node.id())`
- [x] **3.4.2** Implement `applySelectionState(cy, selectedId)` helper: adds/removes `selected`/`highlighted`/`dimmed` classes
- [x] **3.4.3** Add `useEffect` reacting to `props.selectedId` that calls `applySelectionState`
- [x] **3.4.4** In `stylesheet.ts` add fallback rule: if `data.style_class` is not in the known set, use `function` style + log `console.warn`
- [x] **3.4.5** Create `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts` with the full class taxonomy. (~60 lines)
- [x] **3.4.6** Run `pnpm test InteractiveGraph` → 5 more cases pass (total 8)

### 3.5 RED tests for accessibility (a11y)

- [x] **3.5.1** RED: container has `role="application"` and is `tabIndex={0}` (Tab-reachable)
- [x] **3.5.2** RED: fallback `<table role="complementary" aria-label="Graph nodes">` lists every node with id + label
- [x] **3.5.3** RED: focusing a row in the fallback table + pressing `Enter` → calls `onSelectObject(rowId)`
- [x] **3.5.4** RED: focusing a row + pressing `Space` → calls `onSelectObject(rowId)`

### 3.6 GREEN: implement a11y fallback table

- [x] **3.6.1** In `InteractiveGraph.tsx` render `<table role="complementary">` after the cytoscape mount with one `<tr>` per node
- [x] **3.6.2** Add `onKeyDown` handler on rows: `Enter`/`Space` → `onSelectObject`
- [x] **3.6.3** Run `pnpm test InteractiveGraph` → 4 more cases pass (total 12)

### 3.7 RED tests for `Shell.tsx` 4th column wiring

- [x] **3.7.1** In `apps/explorer-ui/src/components/Shell.test.tsx` add viewport tests
- [x] **3.7.2** RED: at `width: 1500` (desktop) → `InteractiveGraph` is `React.lazy`-imported and mounted in a 4th grid column
- [x] **3.7.3** RED: at `width: 1000` (tablet) → `InteractiveGraph` renders as lens overlay, not a 4th column
- [x] **3.7.4** RED: at `width: 600` (small) → `InteractiveGraph` is not rendered (SvgGraph fallback unchanged)

### 3.8 GREEN: wire Shell 4th column

- [x] **3.8.1** In `apps/explorer-ui/src/components/viewport.ts` add `"ultrawide"` tier: `width >= 1440` → `"ultrawide"`, else keep existing breakpoints. (+5 lines)
- [x] **3.8.2** In `Shell.tsx` import `InteractiveGraph` via `React.lazy(() => import("./InteractiveGraph"))`. (+5 lines)
- [x] **3.8.3** In `Shell.tsx` extend grid: `desktop|ultrawide` → 4 columns; `tablet` → existing 2 columns + lens overlay showing `InteractiveGraph`; `small` → unchanged. (+20 lines)
- [x] **3.8.4** Run `pnpm test Shell` → 3 cases pass

### 3.9 GREEN: index + final wiring

- [x] **3.9.1** Create `apps/explorer-ui/src/components/InteractiveGraph/index.ts` exporting `{ InteractiveGraph }`. (~5 lines)
- [x] **3.9.2** In `Shell.tsx` connect `selectedId` from existing spotter/inspector state to `InteractiveGraph.selectedId` prop, and bind `onSelectObject` to existing handler
- [x] **3.9.3** Run full suite: `pnpm test` → all green (46 new + existing)

### 3.10 Bundle size + lint gate

- [x] **3.10.1** Run `pnpm build` → succeeds
- [x] **3.10.2** Run `pnpm build:check-bundle` → cytoscape is in the `InteractiveGraph` chunk only (verify by inspecting `dist/assets/*.js`)
- [x] **3.10.3** Run `pnpm lint` → clean
- [x] **3.10.4** Commit: `feat(explorer-ui): add InteractiveGraph component with elkjs layout` (split: tests, component, shell, lint fixes)

**PR 3 exit gate**: `pnpm test` + `pnpm build:check-bundle` green, 12 new component tests passing, Shell tests passing, no regression in SvgGraph tests.

---

## Phase 4: Final Verification (post-merge)

- [x] **4.1** Re-run `cargo test --workspace` → all green
- [x] **4.2** Re-run `pnpm test -- --coverage` → coverage report (no minimum threshold enforced, but flag any test that didn't run)
- [x] **4.3** Run `pnpm test:e2e` (Playwright) → add one new e2e spec `interactive-graph.spec.ts` that loads the explorer, switches to ultrawide viewport, clicks a node, asserts ObjectInspector shows the symbol
- [x] **4.4** Update `openspec/changes/visualization-stack/state.yaml` to mark all 3 PR slices merged

---

## Dependency Map (between phases)

```
Phase 0 (pre-flight) ──► Phase 1 (Rust) ──► Phase 2 (Schemas + Worker) ──► Phase 3 (Component + Shell) ──► Phase 4 (verify)
                            │                        │                              │
                            ▼                        ▼                              ▼
                       19 RED tests             15+6+3+4 = 28 RED tests        12+3 = 15 RED tests
                       must fail first          must fail first                 must fail first
```

**Strict TDD order**: Every `[x] 1.x.y` test must be written and confirmed RED before `[x] 1.x.(y+1)` GREEN work. Use `cargo test --no-run` to confirm compilation-only when needed.

## Validation Commands Reference

| When | Command | Expected |
|------|---------|----------|
| After Phase 1.2 | `cargo test -p cognicode-explorer style_class_for` | 6 pass |
| After Phase 1.4 | `cargo test -p cognicode-explorer query_validation` | 7 pass |
| After Phase 1.6 | `cargo test -p cognicode-explorer handler_success` | 5 pass |
| After Phase 1.8 | `cargo test -p cognicode-explorer` | 19 new pass, 0 regression |
| After Phase 2.2 | `pnpm test schemas` | 6 pass |
| After Phase 2.4 | `pnpm test client` | 3 pass |
| After Phase 2.7 | `pnpm test adapter` | 4 pass |
| After Phase 2.9 | `pnpm test layout.worker` | 15 pass |
| After Phase 3.6 | `pnpm test InteractiveGraph` | 12 pass |
| After Phase 3.8 | `pnpm test Shell` | 3 new pass |
| After Phase 3.10 | `pnpm test && pnpm build:check-bundle && pnpm lint` | all green |
| After Phase 4 | `cargo test --workspace && pnpm test && pnpm test:e2e` | all green |

## Line Estimates (per file, per phase)

| File | Phase | New | Modified |
|------|:---:|---:|---:|
| `crates/cognicode-explorer/src/dto.rs` | 1 | +30 | — |
| `crates/cognicode-explorer/src/error.rs` | 1 | +8 | — |
| `crates/cognicode-explorer/src/api.rs` | 1 | +100 | — |
| `crates/cognicode-explorer/src/api_graph_tests.rs` | 1 | +200 | — |
| `crates/cognicode-explorer/src/lib.rs` | 1 | — | +5 (route registration) |
| `apps/explorer-ui/src/api/schemas.ts` | 2 | +35 | — |
| `apps/explorer-ui/src/api/schemas.test.ts` | 2 | +60 | — |
| `apps/explorer-ui/src/api/types.ts` | 2 | +6 | — |
| `apps/explorer-ui/src/api/client.ts` | 2 | +10 | — |
| `apps/explorer-ui/src/api/client.test.ts` | 2 | +30 | — |
| `apps/explorer-ui/src/mocks/fixtures.ts` | 2 | +80 | — |
| `apps/explorer-ui/src/mocks/handlers.ts` | 2 | +25 | — |
| `apps/explorer-ui/src/components/InteractiveGraph/adapter.ts` | 2 | +40 | — |
| `apps/explorer-ui/src/components/InteractiveGraph/adapter.test.ts` | 2 | +60 | — |
| `apps/explorer-ui/src/components/InteractiveGraph/layout.worker.ts` | 2 | +70 | — |
| `apps/explorer-ui/src/components/InteractiveGraph/layout.worker.test.ts` | 2 | +120 | — |
| `apps/explorer-ui/package.json` | 2 | +4 | — |
| `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx` | 3 | +180 | — |
| `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.test.tsx` | 3 | +150 | — |
| `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts` | 3 | +60 | — |
| `apps/explorer-ui/src/components/InteractiveGraph/index.ts` | 3 | +5 | — |
| `apps/explorer-ui/src/components/Shell.tsx` | 3 | — | +25 |
| `apps/explorer-ui/src/components/Shell.test.tsx` | 3 | — | +30 |
| `apps/explorer-ui/src/components/viewport.ts` | 3 | — | +5 |
| **Totals** | | **~1273** | **~125** |

(Roughly matches design estimate of ~1200 new + ~120 modified.)

---

## Out of Scope (locked, do NOT create tasks for)

- D3.js analytics dashboards
- Server-side layout computation
- Named views, ExplorerQL autocomplete, C4 projections
- Mermaid export
- Auth changes to existing API surface
- Cluster / compound node layouts
- Removal of `SvgGraph` (it stays as fallback)
