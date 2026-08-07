# Spec: graph-data-endpoint

> New capability. Companion to proposal `sdd/visualization-stack/proposal`.
> Backend route consumed by `interactive-graph`. No existing routes are
> modified.

## Purpose

A new REST endpoint that returns the **neighborhood subgraph** around a given
symbol id, shaped for direct consumption by Cytoscape.js. The endpoint is the
authoritative data source for `InteractiveGraph`; MSW handlers in the frontend
mirror its contract for offline development and tests.

## Requirements

### Requirement 1: Route registration

The route `GET /api/graph/:id/subgraph` MUST be registered in
`crates/cognicode-explorer/src/api.rs` via a function with the signature:

```rust
async fn get_subgraph(
    State(state): State<Arc<ExplorerState>>,
    Path(id): Path<String>,
    Query(params): Query<SubgraphParams>,
) -> Result<Json<SubgraphResponse>, ApiError>
```

The route MUST be mounted under the existing `Router` chain (no new top-level
prefix). `id` MUST be percent-decoded and validated as non-empty (max length
512 chars). The endpoint MUST respond with `Content-Type: application/json`
on success and on structured error responses.

#### Scenario: Route is mounted and reachable

- GIVEN a running `cognicode-explorer` test server
- WHEN `GET /api/graph/foo/subgraph` is issued
- THEN the response status is `200` (or `404` if `foo` is unknown) — the
  route is NOT `405 Method Not Allowed` and NOT a `404 NOT_FOUND` from the
  router itself (it is bound)

#### Scenario: Empty `id` is rejected

- GIVEN a request `GET /api/graph//subgraph` (empty id segment) or
  `GET /api/graph/%20%20/subgraph` (whitespace-only id)
- WHEN the route handler runs
- THEN it returns `400 Bad Request` with body
  `{"error":"invalid_id","message":"symbol id must be non-empty"}`

#### Scenario: Oversized `id` is rejected

- GIVEN a request with an `id` parameter of 513 characters
- WHEN the route handler runs
- THEN it returns `400 Bad Request` with body
  `{"error":"invalid_id","message":"symbol id exceeds 512 chars"}`

### Requirement 2: Query parameters

`SubgraphParams` MUST be a `#[derive(Deserialize, Debug)]` struct with the
following fields:

| Field        | Type            | Default                       | Validation                                |
|--------------|-----------------|-------------------------------|-------------------------------------------|
| `depth`      | `Option<usize>` | `3`                           | `1 ≤ depth ≤ 10`                          |
| `direction`  | `Option<String>`| `"both"`                      | One of `"incoming"`, `"outgoing"`, `"both"` |
| `max_nodes`  | `Option<usize>` | `500`                         | `1 ≤ max_nodes ≤ 5000`                    |

The handler MUST return `400 Bad Request` with a JSON error body
(`{"error":"invalid_query","message":"..."}`) when validation fails. The error
message MUST name the offending field.

#### Scenario: Defaults applied when params are absent

- GIVEN a request `GET /api/graph/foo/subgraph` (no query string)
- WHEN the handler runs
- THEN it uses `depth=3`, `direction="both"`, `max_nodes=500` (verified via
  integration test using a span/trace or by checking the response shape for
  the default behaviour)

#### Scenario: `depth` out of range is rejected

- GIVEN a request `?depth=0` or `?depth=11`
- WHEN the handler runs
- THEN the response is `400` AND the body contains `"error":"invalid_query"`
  AND the message mentions `"depth"`

#### Scenario: `direction` with an unknown value is rejected

- GIVEN a request `?direction=sideways`
- WHEN the handler runs
- THEN the response is `400` AND the body mentions `"direction"` AND the
  allowed values `"incoming" | "outgoing" | "both"`

#### Scenario: `max_nodes` out of range is rejected

- GIVEN a request `?max_nodes=0` or `?max_nodes=5001`
- WHEN the handler runs
- THEN the response is `400` AND the body mentions `"max_nodes"`

### Requirement 3: Response DTO

The success response MUST be `Json<SubgraphResponse>` with this exact JSON
shape (verified by a frontend zod schema round-trip):

```json
{
  "root": "foo",
  "nodes": [
    {
      "id": "foo",
      "label": "fn foo()",
      "kind": "function",
      "style_class": "function",
      "file": "src/foo.rs",
      "line": 12
    }
  ],
  "edges": [
    {
      "id": "foo->bar",
      "source": "foo",
      "target": "bar",
      "label": "calls",
      "style_class": "edge.calls",
      "confidence": 0.95
    }
  ]
}
```

`SubgraphResponse` MUST be defined in `crates/cognicode-explorer/src/dto.rs`,
`Serialize + Deserialize + Debug + Clone`. The root id MUST always appear
in `nodes` (even when it has no edges). Edge list MUST NOT contain
duplicates. The frontend zod schema `GraphNodeSchema` / `GraphEdgeSchema`
MUST be the canonical source of truth for field names and types — Rust
field names MAY differ but `#[serde(rename = "...")]` MUST align them.

#### Scenario: Root appears in `nodes` even with no edges

- GIVEN an isolated symbol `X` (no callers, no callees)
- WHEN `GET /api/graph/X/subgraph` succeeds
- THEN `response.nodes` contains an entry with `"id":"X"` AND
  `response.edges` is `[]`

#### Scenario: Edges reference only known nodes

- GIVEN any successful response
- WHEN the JSON is parsed
- THEN every `edge.source` and `edge.target` MUST appear in `nodes[*].id`
  AND no edge is duplicated

#### Scenario: JSON field names match the frontend zod schema

- GIVEN a `SubgraphResponse` with one node and one edge
- WHEN serialized to JSON and re-parsed by the frontend `GraphNodeSchema` /
  `GraphEdgeSchema` (Vitest)
- THEN the parsed result equals the original (round-trip; same field names
  and types — no extra required fields)

### Requirement 4: Style-class derivation

Each node MUST carry a `style_class` string chosen from this closed set
based on the `SymbolKind` of the underlying symbol:

| SymbolKind         | `style_class`   |
|--------------------|-----------------|
| `Function`         | `"function"`    |
| `Method`           | `"function"`    |
| `Module`           | `"module"`      |
| `Crate`            | `"module"`      |
| `Trait`            | `"module"`      |
| `External`         | `"external"`    |
| *(anything else)*  | `"function"` (default) |

Each edge MUST carry a `style_class` from this closed set based on
`DependencyType`:

| DependencyType    | `style_class`          |
|-------------------|------------------------|
| `Calls`           | `"edge.calls"`         |
| `Implements`      | `"edge.implements"`    |
| `Uses` / `UsesType` | `"edge.uses"`        |
| `Imports`         | `"edge.uses"`          |
| *(anything else)* | `"edge.calls"` (default) |

The mapping MUST live in a single `fn style_class_for(kind) -> &'static str`
helper; no inline `match` statements in the route handler.

#### Scenario: Function node maps to `"function"` style_class

- GIVEN a function symbol
- WHEN the response is built
- THEN its `style_class` equals `"function"`

#### Scenario: Module / crate / trait map to `"module"` style_class

- GIVEN symbols of kind `Module`, `Crate`, `Trait`
- WHEN the response is built
- THEN each carries `style_class: "module"`

#### Scenario: Edge of type `Calls` maps to `"edge.calls"`

- GIVEN an edge of `DependencyType::Calls`
- WHEN the response is built
- THEN its `style_class` equals `"edge.calls"`

#### Scenario: Unknown edge type falls back to `"edge.calls"`

- GIVEN an edge whose `DependencyType` is a custom variant not in the table
- WHEN the response is built
- THEN its `style_class` equals `"edge.calls"` (no panic)

### Requirement 5: Node / edge caps and truncation

If the requested neighborhood would produce more than `max_nodes` nodes, the
handler MUST truncate the result to `max_nodes` nodes (BFS order, root
first) and the response MUST include a `truncated: true` field plus a
`truncated_reason: "node_cap"` string. The truncated set MUST still satisfy
`edges` references the existing constraint in R3 (every edge source/target
is in `nodes`). When no truncation occurred, `truncated` MUST be `false`.

#### Scenario: Result under the cap sets `truncated: false`

- GIVEN a 5-node neighborhood with `max_nodes=500`
- WHEN the response is built
- THEN `truncated` is `false` AND `truncated_reason` is `null` (or absent)

#### Scenario: Result over the cap sets `truncated: true`

- GIVEN a 600-node reachable neighborhood with `max_nodes=500`
- WHEN the response is built
- THEN `truncated` is `true` AND `truncated_reason` is `"node_cap"` AND
  `nodes.length` is exactly `500` AND every edge's source/target is still
  present in the truncated `nodes` list

### Requirement 6: Error responses

| Condition                                  | Status | `error` code          |
|--------------------------------------------|--------|-----------------------|
| Unknown `id`                               | `404`  | `"symbol_not_found"`  |
| Graph analysis unavailable                 | `503`  | `"graph_unavailable"` |
| Internal error                             | `500`  | `"internal"`          |
| Invalid `id` (empty, oversized)            | `400`  | `"invalid_id"`        |
| Invalid query params                       | `400`  | `"invalid_query"`     |

Error bodies MUST be JSON of the form
`{"error":"<code>","message":"<human readable>"}`. The handler MUST NOT leak
internal types or stack traces in the message.

#### Scenario: Unknown id returns 404 with `symbol_not_found`

- GIVEN an indexed graph does NOT contain a symbol `"missing"`
- WHEN `GET /api/graph/missing/subgraph` is issued
- THEN status is `404` AND body is
  `{"error":"symbol_not_found","message":"..."}`

#### Scenario: Graph unavailable returns 503

- GIVEN `ExplorerState.graph == None`
- WHEN the handler runs
- THEN status is `503` AND body is
  `{"error":"graph_unavailable","message":"graph analysis not ready"}`

#### Scenario: Internal panic is converted to 500

- GIVEN an internal error in the graph service (forced via a test mock)
- WHEN the handler runs
- THEN status is `500` AND body is `{"error":"internal","message":"..."}`
  AND the response body does NOT contain the string `"panicked at"` or
  any Rust type names

## Acceptance Criteria

| #   | Criterion                                                              | Verifies |
| --- | ---------------------------------------------------------------------- | -------- |
| AC1 | Route is bound, rejects empty/oversized id                             | R1       |
| AC2 | Defaults match `depth=3`, `direction="both"`, `max_nodes=500`          | R2       |
| AC3 | All three out-of-range validations return 400 with named field         | R2       |
| AC4 | Response DTO round-trips through frontend zod schema                   | R3       |
| AC5 | Root always present; no duplicate edges; no dangling edge endpoints    | R3, R5   |
| AC6 | Style-class mapping is exhaustive and lives in a single helper         | R4       |
| AC7 | `truncated` flag set correctly under and over the cap                  | R5       |
| AC8 | All 6 error conditions return the documented status + body            | R6       |

## Edge Cases (exhaustive — all MUST have ≥1 test)

| ID  | Case                                          | Expected behavior                              |
| --- | --------------------------------------------- | ---------------------------------------------- |
| E1  | `id` is empty string                          | 400 `invalid_id`                               |
| E2  | `id` is whitespace-only                       | 400 `invalid_id`                               |
| E3  | `id` length > 512                             | 400 `invalid_id`                               |
| E4  | `depth=0` / `depth=11`                        | 400 `invalid_query` mentions `depth`           |
| E5  | `direction` is `"sideways"`                   | 400 `invalid_query` mentions `direction`       |
| E6  | `max_nodes=0` / `max_nodes=5001`              | 400 `invalid_query` mentions `max_nodes`       |
| E7  | Root has no edges                             | nodes has root, edges is `[]`, `truncated:false` |
| E8  | Root is unknown                               | 404 `symbol_not_found`                         |
| E9  | Graph unavailable                             | 503 `graph_unavailable`                        |
| E10 | Self-loop on root                             | edge source/target both == root id            |
| E11 | Cycle reachable from root                     | No duplicate edges; BFS terminates             |
| E12 | Reachable set exceeds `max_nodes`             | `truncated:true`, set size == `max_nodes`      |
| E13 | URL-encoded id (`%2F` etc.)                   | Decoded; treated as a normal id                |
| E14 | Duplicate query keys (`?depth=2&depth=5`)     | Last value wins (axum default), or 400 — pick one and document in handler |
| E15 | Very deep `depth=10` on a wide DAG            | Status 200, response respects `max_nodes` cap  |

## TDD RED Gate

Before implementation is considered started, the following tests MUST exist
and FAIL (RED).

| Test file                                                              | Requirement | Status |
|------------------------------------------------------------------------|-------------|--------|
| `crates/cognicode-explorer/tests/api_subgraph.rs::route_is_mounted`    | R1          | RED    |
| `...::empty_id_rejected`                                               | R1          | RED    |
| `...::oversized_id_rejected`                                           | R1          | RED    |
| `...::defaults_applied`                                                | R2          | RED    |
| `...::depth_out_of_range`                                              | R2          | RED    |
| `...::direction_unknown`                                               | R2          | RED    |
| `...::max_nodes_out_of_range`                                          | R2          | RED    |
| `apps/explorer-ui/src/api/schemas.test.ts::subgraph_response_round_trip`| R3        | RED    |
| `...::root_present_when_isolated`                                      | R3          | RED    |
| `...::edges_reference_known_nodes`                                     | R3          | RED    |
| `...::style_class_for_function`                                        | R4          | RED    |
| `...::style_class_for_module`                                          | R4          | RED    |
| `...::style_class_for_edge_calls`                                      | R4          | RED    |
| `...::unknown_edge_type_falls_back`                                    | R4          | RED    |
| `...::truncation_under_cap`                                            | R5          | RED    |
| `...::truncation_over_cap`                                             | R5          | RED    |
| `...::unknown_id_404`                                                  | R6          | RED    |
| `...::graph_unavailable_503`                                           | R6          | RED    |
| `...::internal_error_500`                                             | R6          | RED    |

## Out of Scope (locked)

- Server-side layout computation (delegated to `elkjs-layout` on the client)
- Streaming responses (single JSON document is sufficient for the
  `max_nodes=5000` cap)
- Authentication / authorization on the route (inherits whatever the
  existing API surface uses; this change adds no new auth requirements)
- Persisting / caching subgraph responses server-side
- Subgraph diffing between two roots
- Including edge provenance / `EvidenceBlock` metadata in the response
- CORS configuration (inherits existing `cors` layer)
