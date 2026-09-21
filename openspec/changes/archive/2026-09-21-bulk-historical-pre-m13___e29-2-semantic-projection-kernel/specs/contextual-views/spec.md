# Delta for Contextual Views

> This delta overlays E29.2 renderer-neutral `SemanticProjection`
> fidelity onto the existing Contextual Views Phase 1 contract. It
> modifies the canonical contextual-views spec (`## MODIFIED Requirements`
> for requirements 1–6). Canonical requirements 1–6 are copied in
> full — including the 400 / 404 / orphan / truncation / dedup /
> registry-listing scenarios — and amended with the new
> semantic-projection behavior. Nothing from the canonical is
> dropped.

## MODIFIED Requirements

### Requirement: 1. Contextual Graph Endpoint Contract

The contextual endpoint MUST return a semantic `GraphTopology` projection for the requested focus and context. It MUST preserve exact nodes, edge endpoints, parent-edge relationships, and edge kinds from evidence. It MUST expose capability status, confidence, provenance, and explicit truncation.

The handler MUST continue to expose `GET /api/graph/:id/contextual` returning a `ContextualGraphResponse` JSON payload, scoped to the query-param contract below. The query-param contract and HTTP-status surface from the canonical Phase 1 spec are preserved.

| Query Param | Type | Default | Purpose |
|---|---|---|---|
| `level` | string | `file` | Reserved for future C4 levels; MUST be `file` in Phase 1 |
| `depth` | integer (1-2) | 1 | BFS depth for same-level neighbors |
| `max_nodes` | integer (50-500) | 200 | Cap for children + neighbors combined |

The handler MUST respond `400` for invalid params, `404` if `id` is unknown, `200` otherwise.

(Previously: the endpoint returned ad-hoc focus, parent, children, and same-level sections and could drop parent-edge and edge-kind fidelity.)

#### Scenario: Valid request returns full payload

- GIVEN a symbol id `S` that exists, lives in file `F`, has callers `[C1, C2]` and callees `[K1]`
- WHEN client calls `GET /api/graph/S/contextual?level=file&depth=1&max_nodes=200`
- THEN response status is `200`
- AND `focusNode.id` equals `S`
- AND `parent.node.id` equals the file-node id for `F`
- AND `children.nodes` contains every sibling symbol in `F` (minus `S` itself)
- AND `sameLevel.nodes` contains `[C1, C2, K1]`
- AND `level` equals `"file"`
- AND `truncated` equals `false`
- AND the response body carries a `SemanticProjection` envelope with `capability_status = supported`, `confidence ∈ [0,1]`, `provenance` for each edge, and the parent edge retains its `LivesIn` kind plus exact endpoints

#### Scenario: Unknown symbol id

- GIVEN symbol id `S` does not exist in the repository
- WHEN client calls `GET /api/graph/S/contextual`
- THEN response status is `404`
- AND body is a Problem Details JSON with `error="symbol_not_found"`
- AND `capability_status` on the projection envelope is `unsupported`

#### Scenario: Invalid query params

- GIVEN a valid symbol id
- WHEN client calls `GET /api/graph/S/contextual?depth=5` or `?max_nodes=10`
- THEN response status is `400`
- AND body explains which param violated its bound

#### Scenario: Symbol with no `lives_in` edge

- GIVEN symbol `S` has no `lives_in` edge (orphan)
- WHEN client calls `GET /api/graph/S/contextual`
- THEN `parent` is `null`
- AND `children` is `null`
- AND `focusNode` and `sameLevel` are still populated
- AND the projection envelope reports `parent_edge_present = false` honestly — no inferred file or hierarchy edge is emitted

#### Scenario: Truncation when siblings exceed cap

- GIVEN file `F` contains 300 symbols; `max_nodes=200`
- WHEN client calls the endpoint
- THEN `truncated` equals `true`
- AND `children.nodes.length` + `sameLevel.nodes.length` ≤ 200
- AND `truncationReason` equals `"max_nodes_exceeded"`
- AND retained nodes and edges keep their exact identities and kinds (no silent capping of unrelated structure)

#### Scenario: Context preserves topology metadata

- GIVEN focus S has parent file F, sibling K, and call neighbor C
- WHEN the contextual projection is requested
- THEN S, F, K, and C retain their exact identities
- AND the parent and neighbor edges retain exact kinds and endpoints

### Requirement: 2. `ContextualGraphResponse` DTO Shape

The response MUST conform to the canonical `ContextualGraphResponse` JSON shape and MUST additionally carry a `SemanticProjection` envelope. The DTO MUST reuse existing `GraphNode` and `GraphEdge` types and MUST NOT redefine them.

```json
{
  "focusNode":  { /* GraphNode */ },
  "parent":     { "node": { /* GraphNode */ }, "edge": { /* GraphEdge */ } } | null,
  "children":   { "nodes": [ /* GraphNode */ ], "edges": [ /* GraphEdge */ ] } | null,
  "sameLevel":  { "nodes": [ /* GraphNode */ ], "edges": [ /* GraphEdge */ ] },
  "level":      "file",
  "truncated":  false,
  "truncationReason": null,
  "projection": {
    "capability_status": "supported",
    "confidence":        0.0,
    "provenance":        [ /* per-edge */ ],
    "truncated":         false,
    "truncation_reason": null
  }
}
```

#### Scenario: All sections present

- GIVEN a symbol with file parent, siblings, and call neighbors
- WHEN the handler builds the response
- THEN all four sections (`focusNode`, `parent`, `children`, `sameLevel`) are non-null
- AND each section conforms to the canonical shape above
- AND `projection` is non-null with `capability_status = supported`

#### Scenario: Reused DTO types

- GIVEN the response contains canonical `GraphNode` and `GraphEdge` values
- WHEN the response is serialized
- THEN `GraphNode.id` and `GraphEdge.source`/`target` fields appear in the JSON
- AND `GraphEdge.kind` is preserved (no kind-erasure)
- AND no duplicated field schema is introduced

### Requirement: 3. `ContextualPanel` React Component

The system MUST provide a `ContextualPanel` component that renders the `ContextualGraphResponse` in four visual regions: focus card, parent breadcrumb, children list, neighbor minigraph. The component MUST render the projection envelope verbatim: capability status, confidence, provenance, and truncation MUST surface in the UI without converting unsupported or truncated results into complete-looking visualizations. The component MUST NOT synthesize nodes, parent edges, sibling relations, or edge kinds that are absent from the projection.

| Region | Rendered From | Behavior |
|---|---|---|
| Focus card | `focusNode` | Always visible; shows id, kind, file path |
| Parent breadcrumb | `parent.node` | Visible only if non-null; click → new focus |
| Children list | `children.nodes` | Scrollable; click row → new focus |
| Neighbor minigraph | `sameLevel.nodes/edges` | Cytoscape canvas; click node → new focus |

#### Scenario: Click neighbor navigates to new focus

- GIVEN ContextualPanel is rendered with focus `S` and neighbor `C1`
- WHEN user clicks node `C1` in the minigraph
- THEN the hook refetches `/api/graph/C1/contextual`
- AND the panel re-renders with `C1` as the new `focusNode`

#### Scenario: Truncation banner

- GIVEN response has `truncated=true`
- WHEN the panel renders
- THEN a visible warning banner shows `"Showing top N — refine with max_nodes or focus deeper"`
- AND the banner quotes `truncation_reason` from the projection envelope

#### Scenario: No parent (orphan symbol)

- GIVEN `parent` is `null` in the response
- WHEN the panel renders
- THEN the parent breadcrumb region is not rendered
- AND no layout collapse gap appears (region collapses to zero height)

#### Scenario: No children (file with only the focus)

- GIVEN `children.nodes` is empty
- WHEN the panel renders
- THEN the children list shows `"No sibling symbols in this file"`

#### Scenario: Empty evidenced context

- GIVEN a focus has no evidenced neighbors
- WHEN the context is rendered
- THEN it shows an honest empty state and does not add visual neighbors

#### Scenario: Unsupported context

- GIVEN the projection reports `capability_status = unsupported`
- WHEN the context is rendered
- THEN it communicates unsupported status rather than showing speculative structure

### Requirement: 4. `useContextualGraph` SWR Hook

The hook MUST accept `(symbolId: string, opts: ContextualOptions)` and return `{ data, error, isLoading, mutate }`. The hook MUST call `GET /api/graph/:id/contextual` with query params from `opts`. It MUST use SWR's `dedupingInterval` of 5000ms to avoid refetch storms when the user clicks rapidly.

#### Scenario: Successful fetch

- GIVEN the endpoint returns 200 with a valid payload
- WHEN the hook is invoked with `(S, { depth: 1, maxNodes: 200 })`
- THEN `data` equals the response body
- AND `isLoading` flips from `true` to `false`
- AND `data.projection.capability_status` is `supported`

#### Scenario: 404 propagates as error

- GIVEN the endpoint returns 404
- WHEN the hook is invoked
- THEN `data` is `null`
- AND `error.status` equals `404`

#### Scenario: Rapid clicks deduplicate

- GIVEN user clicks `S`, then `C1`, then `C2` within 100ms
- WHEN all three calls are dispatched
- THEN only the latest request (`C2`) is in-flight
- AND `data` reflects `C2`

### Requirement: 5. TDD Red Gate (Blocking)

The system MUST NOT pass review until unit tests for the DTO serialization AND integration tests for the endpoint exist AND are failing (RED) before implementation. Tests are the source of truth for the contract, including the projection envelope's `capability_status`, `confidence`, `provenance`, and `truncated` fields.

#### Scenario: Red tests exist before any handler code

- GIVEN the change is opened
- WHEN the reviewer inspects the PR
- THEN test files for `ContextualGraphResponse` (serde) and `contextual_handler` (handler) exist
- AND these tests are failing at the commit where the handler is first introduced
- AND no production code touches the new routes until the RED phase is observed in CI

### Requirement: 6. `available_views` listing is registry-driven

The endpoint `GET /api/objects/:object_id/views` MUST return the `ViewDescriptor` list produced by `ViewRegistry::list_for(object.type)`. The list MUST include every built-in view registered via `register_view!` whose `applies_to` matches the object's type, in alphabetical id order. The wire shape `Vec<ViewDescriptor> { id, title }` is unchanged for Phase 1; the richer descriptor shape (with `view_kind`, `renderer_kind`, `is_builtin`) ships in a follow-up.

The list MUST remain honest: a built-in view MUST NOT be exposed when its underlying projection reports `capability_status = unsupported` for the object type — the registry gates both kinds of unavailability consistently.

#### Scenario: Symbol listing returns 4 built-ins

- GIVEN a `Symbol` object
- WHEN `GET /api/objects/<symbol_id>/views` runs after Phase 1
- THEN the response is a JSON array of 4 elements with ids `["call-graph", "overview", "quality", "source"]` (alphabetical)
- AND each element has the same `{ id, title }` shape the pre-change endpoint produced

#### Scenario: File listing returns 1 view

- GIVEN a `File` object
- WHEN `GET /api/objects/<file_id>/views` runs
- THEN the response is a JSON array of 1 element: `[{ id: "quality", title: "Quality" }]`
- AND no `Symbol`-only view (`call-graph`, `source`) is present

#### Scenario: Existing test suite stays green

- GIVEN `crates/cognicode-explorer/src/api_views_tests.rs` (or equivalent)
- WHEN `cargo test -p cognicode-explorer views_endpoint` runs after Phase 1
- THEN every existing test passes byte-identical

(Previously: this endpoint was populated by a hardcoded `match object_type` mapping in the service layer. The mapping is replaced by `ViewRegistry::list_for`; the wire shape is preserved for Phase 1.)

## ADDED Requirements

### Requirement: Projection-backed structural views

Call, dependency, impact, use-case, and data-flow contextual views MUST consume their corresponding semantic projection and preserve each relation kind. Renderers MAY be adapted ONLY enough to consume the projection envelope (capability status, confidence, provenance, truncation); they MUST NOT redesign UI presentation — that work belongs to E29.3.

#### Scenario: Dependency edge is not call edge

- GIVEN A depends on B but no call edge exists
- WHEN dependency context is rendered
- THEN the dependency relation is shown with its kind and no call relation appears

#### Scenario: Orphan remains honest

- GIVEN S has no evidenced parent edge
- WHEN contextual projection is requested
- THEN parent is absent, status remains explicit, and no parent or sibling structure is inferred

#### Scenario: Truncated context

- GIVEN context exceeds its configured limit
- WHEN projection is requested
- THEN retained evidence remains exact and the response reports `truncated=true` with a reason
