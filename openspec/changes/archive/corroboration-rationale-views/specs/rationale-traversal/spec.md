# rationale-traversal Specification (NEW)

## Purpose

A new REST endpoint and repository method that extract the **rationale sub-graph** around a focus node: the multimodal chain `Code → Justifies → Decision → Cites → Doc` plus `Decision → Resolves → Issue` and the reverse `CorroboratedBy` reachability. The traversal is BFS over a fixed allow-list of `EdgeKind` variants — multimodal only — distinct from the existing code-only `subgraph` endpoint that walks `Dependency(Calls/Implements/Uses)`. The output shape is `SubgraphResponse` reused with an optional `corroboration_scores: HashMap<String, f64>` extension. This is the data primitive consumed by `rationale-view-component` and the `lens="rationale"` named-view branch.

## Domain

| Term | Definition |
|------|------------|
| Rationale edge allow-list | `EdgeKind::Justifies`, `EdgeKind::Cites`, `EdgeKind::Resolves`, `EdgeKind::CorroboratedBy` — the 4 multimodal edge kinds traversed. `EdgeKind::Dependency(_)` is excluded. |
| Max depth | Number of BFS hops from the focus node. Default 3, hard cap 5. |
| Max nodes | Total node cap across the entire sub-graph. Default 50, hard cap 200. |
| Truncation | When traversal would exceed `max_nodes`, return the first `max_nodes` nodes visited (BFS order), drop edges whose endpoints were not visited, and set `truncated: true` + `truncation_reason: "max_nodes_exceeded"`. |
| Focus node | The starting node id (a `NodeId` string). The focus is always echoed in `nodes[0]` even when unknown or isolated. |

## Files

| File | Change |
|------|--------|
| `crates/cognicode-explorer/src/ports/graph_repository.rs` | Add `edges_by_kind(&self, node: &NodeId, kinds: &[EdgeKind])` and `rationale_subgraph(&self, focus: &NodeId, max_depth: u32, max_nodes: usize)` to the port trait |
| `crates/cognicode-explorer/src/adapters/generic_graph_repository.rs` | Implement both port methods on the in-memory adapter |
| `crates/cognicode-explorer/src/api.rs` | Add `async fn rationale_handler(...)` and route `GET /api/graph/:id/rationale` |
| `crates/cognicode-explorer/src/dto.rs` | Add `corroboration_scores: HashMap<String, f64>` field to `SubgraphResponse` with `#[serde(default, skip_serializing_if = "HashMap::is_empty")]` |
| `crates/cognicode-explorer/src/api_rationale_tests.rs` | New test module: 14 RED tests for endpoint + port methods |

## Requirements

### Requirement: Repository `edges_by_kind` port method

`GraphRepository::edges_by_kind(node: &NodeId, kinds: &[EdgeKind]) -> Result<Vec<GraphEdge>, ExplorerError>` MUST be added to the port trait. The method MUST return every edge in the store whose `source == *node` AND whose `kind` is in `kinds`, regardless of confidence. The returned edges MUST be deduped on `(source, target, kind)`. The method MUST be `async` and feature-gated behind the `multimodal` Cargo feature. An empty `kinds` slice MUST return `Ok(vec![])` without touching the store.

#### Scenario: Returns all multimodal edges for a node

- GIVEN a graph with `Code A` connected to `Decision D` via `Justifies` and to `Doc X` via `Cites`
- WHEN `edges_by_kind(A, &[Justifies, Cites])` runs
- THEN the result has length 2 AND both edges appear AND no `Dependency(_)` edges appear

#### Scenario: Unknown node returns empty

- GIVEN no graph entry for `NodeId("missing", NodeKind::Decision)`
- WHEN `edges_by_kind(&missing, &[Justifies])` runs
- THEN the result is `Ok(vec![])`

#### Scenario: Empty kind filter returns empty without I/O

- GIVEN any graph
- WHEN `edges_by_kind(&n, &[])` runs
- THEN the result is `Ok(vec![])` AND no store call is dispatched (asserted via mock expectation)

#### Scenario: Dedup collapses duplicate (source, target, kind) triples

- GIVEN two `Justifies` edges with identical `(A, D, Justifies)` but different `confidence` values
- WHEN `edges_by_kind(A, &[Justifies])` runs
- THEN the result has length 1 AND the higher-confidence edge is retained

### Requirement: Repository `rationale_subgraph` port method

`GraphRepository::rationale_subgraph(focus: &NodeId, max_depth: u32, max_nodes: usize) -> Result<(Vec<GraphNode>, Vec<GraphEdge>), ExplorerError>` MUST be added to the port trait. The method MUST BFS-traverse starting from `focus`, following **outgoing and incoming** multimodal edges of the 4 allowed kinds only. The BFS MUST stop at `max_depth` (root is depth 0) AND MUST stop when `max_nodes` is reached. The output `(nodes, edges)` MUST satisfy: `nodes[0] == focus`; every edge has both endpoints in `nodes`; no duplicate node ids; no duplicate edges on `(source, target, kind)`. The method MUST be `async` and feature-gated behind `multimodal`. `max_depth == 0` MUST return `(vec![focus_node], vec![])` even when the focus is unknown.

#### Scenario: Depth 2 reachability across Justifies + Cites

- GIVEN a chain `Code A --Justifies--> Decision D --Cites--> Doc X` plus a sibling `Doc Y --Cites--> D`
- WHEN `rationale_subgraph(A, 2, 50)` runs
- THEN `nodes` equals `[{A}, {D}, {X}, {Y}]` (any order) AND `edges` contains the 3 multimodal edges

#### Scenario: Focus is always in nodes even if unknown

- GIVEN no entry for `NodeId("missing", SymbolKind::Function)` in the graph
- WHEN `rationale_subgraph(&missing, 3, 50)` runs
- THEN `nodes == [{missing, _, _, _, _}]` AND `edges == []`

#### Scenario: max_depth 0 returns only the focus

- GIVEN a chain `A --Justifies--> B --Cites--> C`
- WHEN `rationale_subgraph(A, 0, 50)` runs
- THEN `nodes == [{A}]` AND `edges == []`

#### Scenario: max_nodes truncation drops edges with missing endpoints

- GIVEN a star `Code A --Justifies--> D_i` for `i = 1..60`, plus `Doc X_i --Cites--> D_i`
- WHEN `rationale_subgraph(A, 3, 50)` runs
- THEN `nodes.len() == 50` AND every `edge.source` and `edge.target` is in `nodes` AND `truncated` is set externally by the handler (the port returns a flag in a tuple, see design)

#### Scenario: Cycle is handled without infinite loop

- GIVEN `A --Justifies--> B --CorroboratedBy--> A`
- WHEN `rationale_subgraph(A, 5, 50)` runs
- THEN the result terminates AND `nodes == [A, B]` (or `[B, A]`) AND `edges == [{A,B,Justifies}, {B,A,CorroboratedBy}]`

### Requirement: REST endpoint `GET /api/graph/:id/rationale`

The route `GET /api/graph/:id/rationale` MUST be registered in `crates/cognicode-explorer/src/api.rs` via a function with the signature:

```rust
async fn rationale_handler(
    State(state): State<Arc<ExplorerState>>,
    Path(id): Path<String>,
    Query(params): Query<RationaleParams>,
) -> Result<Json<SubgraphResponse>, ApiError>
```

`RationaleParams { max_depth: Option<u32>, max_nodes: Option<usize> }` MUST be `#[derive(Deserialize)]`. Defaults: `max_depth = 3`, `max_nodes = 50`. Validation: `max_depth ∈ 1..=5`; `max_nodes ∈ 1..=200`. Out-of-range MUST return `400` with body `{"error":"invalid_query","message":"<param> out of range [<min>..=<max>]"}`. The handler MUST percent-decode `id`, reject empty / whitespace-only / >512-char ids with `400 invalid_id`, and call `state.service.rationale_subgraph(focus, max_depth, max_nodes)`. The response MUST be a `SubgraphResponse` with `corroboration_scores: HashMap<String, f64>` populated for the rationale edges. The endpoint MUST be feature-gated behind the `multimodal` feature — on a build without it, the route MUST return `404` from the router (not 500).

#### Scenario: Default params return depth-3, max-50 subgraph

- GIVEN a focus `Code A` with a 4-hop rationale chain
- WHEN `GET /api/graph/A/rationale` (no query) is issued
- THEN status is `200` AND `nodes.len() <= 50` AND every edge is one of `Justifies|Cites|Resolves|CorroboratedBy`

#### Scenario: max_depth=2 caps traversal

- GIVEN the same 4-hop chain
- WHEN `GET /api/graph/A/rationale?max_depth=2` is issued
- THEN `nodes` excludes the 3rd- and 4th-hop nodes

#### Scenario: max_nodes=5 truncates with reason

- GIVEN a focus with 30 reachable multimodal nodes
- WHEN `GET /api/graph/A/rationale?max_nodes=5` is issued
- THEN `truncated == true` AND `truncation_reason == "max_nodes_exceeded"` AND `nodes.len() == 5` AND no edge has a missing endpoint

#### Scenario: max_depth out of range returns 400

- GIVEN any focus
- WHEN `GET /api/graph/A/rationale?max_depth=0` or `?max_depth=6` is issued
- THEN status is `400` AND body explains the bound

#### Scenario: max_nodes out of range returns 400

- GIVEN any focus
- WHEN `GET /api/graph/A/rationale?max_nodes=0` or `?max_nodes=201` is issued
- THEN status is `400`

#### Scenario: Empty / oversized id returns 400

- GIVEN the route is bound
- WHEN `GET /api/graph//rationale` (empty id) or `GET /api/graph/<513 chars>/rationale` is issued
- THEN status is `400` AND body is `{"error":"invalid_id","message":"symbol id must be non-empty and ≤512 chars"}`

#### Scenario: Response Content-Type is application/json

- GIVEN any valid request
- WHEN the response is received
- THEN `Content-Type` header equals `application/json`

#### Scenario: Feature-gate off: route returns 404

- GIVEN a build of `cognicode-explorer` without `--features multimodal`
- WHEN `GET /api/graph/A/rationale` is issued
- THEN status is `404` AND the route is NOT mounted (the rest of the API still works)

### Requirement: `SubgraphResponse.corroboration_scores` field

`SubgraphResponse` in `dto.rs` MUST gain a new field:

```rust
#[serde(default, skip_serializing_if = "HashMap::is_empty")]
pub corroboration_scores: HashMap<String, f64>,
```

The map MUST be keyed by `GraphEdge.id` and MUST map to a score in `0.0..=1.0`. The field MUST be optional in JSON: when the map is empty, the key MUST NOT appear in the serialized response. This extension MUST be backward-compatible — existing clients that do not know about the field MUST keep parsing the response without error.

#### Scenario: Empty map omitted from JSON

- GIVEN a `SubgraphResponse` with `corroboration_scores: HashMap::new()`
- WHEN serialized
- THEN the JSON does NOT contain the key `corroboration_scores`

#### Scenario: Non-empty map serialized

- GIVEN `corroboration_scores: { "e1": 0.8, "e2": 1.0 }`
- WHEN serialized
- THEN the JSON contains `"corroboration_scores":{"e1":0.8,"e2":1.0}`

#### Scenario: Round-trip preserves scores

- GIVEN a response with 3 edges and 2 score entries
- WHEN `serde_json::to_string` then `from_str` runs
- THEN the deserialized map equals the original AND every score ∈ `0.0..=1.0`

## TDD RED Gate

These tests MUST be written FIRST and MUST FAIL before any implementation lands:

| Test | File | Asserts |
|------|------|---------|
| `edges_by_kind_returns_multimodal_edges` | `generic_graph_repository.rs` (test) | Length and kind-filter correctness |
| `edges_by_kind_unknown_node_returns_empty` | `generic_graph_repository.rs` | `Ok(vec![])` |
| `edges_by_kind_empty_filter_skips_store` | `generic_graph_repository.rs` (mock) | No I/O when `kinds == &[]` |
| `edges_by_kind_dedups_by_triple` | `generic_graph_repository.rs` | Length 1 with higher confidence kept |
| `rationale_subgraph_depth_2_reachability` | `generic_graph_repository.rs` | Chain + sibling reach |
| `rationale_subgraph_unknown_focus_echoed` | `generic_graph_repository.rs` | `nodes == [focus]`, `edges == []` |
| `rationale_subgraph_max_depth_zero` | `generic_graph_repository.rs` | Only focus |
| `rationale_subgraph_max_nodes_truncation` | `generic_graph_repository.rs` | Length cap + edge integrity |
| `rationale_subgraph_cycle_terminates` | `generic_graph_repository.rs` | `[A, B]`, no panic |
| `rationale_endpoint_default_params` | `api_rationale_tests.rs` | 200, `nodes.len() <= 50` |
| `rationale_endpoint_max_depth_clamped` | `api_rationale_tests.rs` | Depth=2 excludes hop 3+ |
| `rationale_endpoint_max_nodes_truncation` | `api_rationale_tests.rs` | `truncated: true`, no dangling edges |
| `rationale_endpoint_invalid_params_400` | `api_rationale_tests.rs` | Out-of-range → 400 with bound message |
| `rationale_endpoint_invalid_id_400` | `api_rationale_tests.rs` | Empty / oversized id → 400 |
| `rationale_endpoint_feature_gate_off_404` | `api_rationale_tests.rs` (no `--features multimodal`) | Route not mounted |
| `subgraph_response_serde_omits_empty_scores` | `dto.rs` (test) | JSON does not contain the key |
| `subgraph_response_serde_round_trip` | `dto.rs` (test) | Map preserved with valid scores |

## Out of Scope (locked)

- Cycle-aware path planning within the rationale sub-graph
- Server-side layout computation
- Async / streaming response
- Persisting rationale sub-graphs to disk
- Materialized corroboration scores (scoring logic lives in `corroboration-scoring` spec; here we only carry the map)
- Sub-graph diffing between two focuses
- Direction filtering (incoming / outgoing / both) — the rationale case is always bidirectional; consumers filter post-hoc
- Authn / authz on the rationale endpoint (no auth in v1)

