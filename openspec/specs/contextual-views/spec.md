# Spec: Contextual Graph (Contextual Views — Phase 1)

## Purpose

Add file-level contextual projection for any graph node. A single
bundled endpoint returns focus + parent (file via `lives_in`) +
children (siblings via `find_symbols_by_file`) + same-level call
neighbors (BFS). Frontend renders via a new `ContextualPanel`
component.

## Domain

`contextual-graph` — NEW capability (no existing spec to delta
against; this is a full spec).

---

## ADDED Requirements

### Requirement: 1. Contextual Graph Endpoint Contract

The system MUST expose `GET /api/graph/:id/contextual` returning a
`ContextualGraphResponse` JSON payload.

| Query Param | Type | Default | Purpose |
|---|---|---|---|
| `level` | string | `file` | Reserved for future C4 levels; MUST be `file` in Phase 1 |
| `depth` | integer (1-2) | 1 | BFS depth for same-level neighbors |
| `max_nodes` | integer (50-500) | 200 | Cap for children + neighbors combined |

The handler MUST respond `400` for invalid params, `404` if `id` is
unknown, `200` otherwise.

#### Scenario: Valid request returns full payload

- GIVEN a symbol id `S` that exists, lives in file `F`, has callers
  `[C1, C2]` and callees `[K1]`
- WHEN client calls `GET /api/graph/S/contextual?level=file&depth=1&max_nodes=200`
- THEN response status is `200`
- AND `focusNode.id` equals `S`
- AND `parent.node.id` equals the file-node id for `F`
- AND `children.nodes` contains every sibling symbol in `F` (minus `S`
  itself)
- AND `sameLevel.nodes` contains `[C1, C2, K1]`
- AND `level` equals `"file"`
- AND `truncated` equals `false`

#### Scenario: Unknown symbol id

- GIVEN symbol id `S` does not exist in the repository
- WHEN client calls `GET /api/graph/S/contextual`
- THEN response status is `404`
- AND body is a Problem Details JSON with `error="symbol_not_found"`

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

#### Scenario: Truncation when siblings exceed cap

- GIVEN file `F` contains 300 symbols; `max_nodes=200`
- WHEN client calls the endpoint
- THEN `truncated` equals `true`
- AND `children.nodes.length` + `sameLevel.nodes.length` ≤ 200
- AND `truncationReason` equals `"max_nodes_exceeded"`

### Requirement: 2. `ContextualGraphResponse` DTO Shape

The response MUST conform to the following JSON shape:

```json
{
  "focusNode":  { /* GraphNode */ },
  "parent":     { "node": { /* GraphNode */ }, "edge": { /* GraphEdge */ } } | null,
  "children":   { "nodes": [ /* GraphNode */ ], "edges": [ /* GraphEdge */ ] } | null,
  "sameLevel":  { "nodes": [ /* GraphNode */ ], "edges": [ /* GraphEdge */ ] },
  "level":      "file",
  "truncated":  false,
  "truncationReason": null
}
```

The DTO MUST reuse existing `GraphNode` and `GraphEdge` types. It
MUST NOT redefine them.

#### Scenario: All sections present

- GIVEN a symbol with file parent, siblings, and call neighbors
- WHEN the handler builds the response
- THEN all four sections (`focusNode`, `parent`, `children`,
  `sameLevel`) are non-null
- AND each section conforms to the shape above

#### Scenario: Reused DTO types

- WHEN the response is serialized
- THEN `GraphNode.id` and `GraphEdge.source`/`target` fields appear
  in the JSON
- AND no duplicated field schema is introduced

### Requirement: 3. `ContextualPanel` React Component

The system MUST provide a `ContextualPanel` component that renders
the `ContextualGraphResponse` in four visual regions: focus card,
parent breadcrumb, children list, neighbor minigraph.

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
- THEN a visible warning banner shows `"Showing top N — refine
  with max_nodes or focus deeper"`

#### Scenario: No parent (orphan symbol)

- GIVEN `parent` is `null` in the response
- WHEN the panel renders
- THEN the parent breadcrumb region is not rendered
- AND no layout collapse gap appears (region collapses to zero height)

#### Scenario: No children (file with only the focus)

- GIVEN `children.nodes` is empty
- WHEN the panel renders
- THEN the children list shows `"No sibling symbols in this file"`

### Requirement: 4. `useContextualGraph` SWR Hook

The hook MUST accept `(symbolId: string, opts: ContextualOptions)`
and return `{ data, error, isLoading, mutate }`.

The hook MUST call `GET /api/graph/:id/contextual` with query params
from `opts`. It MUST use SWR's `dedupingInterval` of 5000ms to avoid
refetch storms when the user clicks rapidly.

#### Scenario: Successful fetch

- GIVEN the endpoint returns 200 with a valid payload
- WHEN the hook is invoked with `(S, { depth: 1, maxNodes: 200 })`
- THEN `data` equals the response body
- AND `isLoading` flips from `true` to `false`

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

The system MUST NOT pass review until unit tests for the DTO
serialization AND integration tests for the endpoint exist AND are
failing (RED) before implementation. Tests are the source of truth
for the contract.

#### Scenario: Red tests exist before any handler code

- GIVEN the change is opened
- WHEN the reviewer inspects the PR
- THEN test files for `ContextualGraphResponse` (serde) and
  `contextual_handler` (handler) exist
- AND these tests are failing at the commit where the handler is
  first introduced
- AND no production code touches the new routes until the RED phase
  is observed in CI

## Out of Scope (Explicit Non-Requirements)

- C4 abstraction edges (`part_of`, `belongs_to`, `deployed_as`,
  `in_system`)
- Multi-level traversal (Component / Container / System)
- Persisted named contextual views
- MCP `graph_contextual` tool (deferred to Phase 2)

### Requirement: 6. `available_views` listing is registry-driven

The endpoint `GET /api/objects/:object_id/views` MUST return the
`ViewDescriptor` list produced by
`ViewRegistry::list_for(object.type)`. The list MUST include every
built-in view registered via `register_view!` whose `applies_to`
matches the object's type, in alphabetical id order. The wire
shape `Vec<ViewDescriptor> { id, title }` is unchanged for Phase 1;
the richer descriptor shape (with `view_kind`, `renderer_kind`,
`is_builtin`) ships in a follow-up.

#### Scenario: Symbol listing returns 4 built-ins

- GIVEN a `Symbol` object
- WHEN `GET /api/objects/<symbol_id>/views` runs after Phase 1
- THEN the response is a JSON array of 4 elements with ids
  `["call-graph", "overview", "quality", "source"]` (alphabetical)
- AND each element has the same `{ id, title }` shape the
  pre-change endpoint produced

#### Scenario: File listing returns 1 view

- GIVEN a `File` object
- WHEN `GET /api/objects/<file_id>/views` runs
- THEN the response is a JSON array of 1 element: `[{ id:
  "quality", title: "Quality" }]`
- AND no `Symbol`-only view (`call-graph`, `source`) is present

#### Scenario: Existing test suite stays green

- GIVEN `crates/cognicode-explorer/src/api_views_tests.rs` (or
  equivalent)
- WHEN `cargo test -p cognicode-explorer views_endpoint` runs
  after Phase 1
- THEN every existing test passes byte-identical

(Previously: this endpoint was populated by a hardcoded
`match object_type` mapping in the service layer. The mapping is
replaced by `ViewRegistry::list_for`; the wire shape is preserved
for Phase 1.)

## Coverage

- **Happy paths**: covered (scenarios for full payload, click
  navigation, dedup, red gate)
- **Edge cases**: covered (orphan symbol, empty children, truncation)
- **Error states**: covered (404, 400, hook error propagation)
