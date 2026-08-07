# federated-graph-service Specification (NEW)

## Purpose

`FederatedGraphService` multiplexes N space-scoped `GraphRepository` instances behind a single query interface. Each result is wrapped in a `FederatedNode` that carries the originating `SpaceId`, so downstream consumers (brain tools, frontend) can render provenance per space. Defined in `cognicode-explorer/src/federation/`. Gated by `multimodal` feature.

## Domain Types

| Type | File | Definition |
|------|------|------------|
| `FederatedNode` | `crates/cognicode-explorer/src/federation/federated_node.rs` | `pub struct FederatedNode { pub node: GraphNode, pub space_id: SpaceId }` |
| `FederatedNodeId` | `crates/cognicode-explorer/src/federation/federated_node_id.rs` | Newtype `pub struct FederatedNodeId(pub String)` — wire format `{space_id}::{local_node_id}` |
| `FederatedGraphService` | `crates/cognicode-explorer/src/federation/federated_graph_service.rs` | Owns `HashMap<SpaceId, Arc<dyn GraphRepository>>`; exposes search/find methods that merge across spaces |

## Requirements

### Requirement: FederatedNodeId Format

`FederatedNodeId` MUST be `{space_id}::{local_node_id}` where `space_id` is the `SpaceId.0` and `local_node_id` is `GraphNode.id.0`. The `::` separator is reserved; neither segment may contain `::`. Construction MUST validate the format via `try_new`.

#### Scenario: Valid format
- GIVEN `FederatedNodeId::try_new("repo-a::src/main.rs:main:1")`
- THEN `id.space_id() == Some(SpaceId("repo-a"))` AND `id.local_id() == Some("src/main.rs:main:1")`

#### Scenario: Missing separator rejected
- GIVEN `FederatedNodeId::try_new("no-separator-here")`
- THEN it MUST return `Err(FederatedNodeIdError::MissingSeparator)`

#### Scenario: Empty space segment rejected
- GIVEN `FederatedNodeId::try_new("::main:1")`
- THEN it MUST return `Err(FederatedNodeIdError::EmptySpaceSegment)`

### Requirement: FederatedNode Wrapper

`FederatedNode` MUST carry `node: GraphNode` and `space_id: SpaceId`. `federated_id()` MUST return the `FederatedNodeId` derived by joining `space_id.0` + `"::"` + `node.id.0`. `Display` MUST print the `FederatedNodeId` string verbatim.

#### Scenario: federated_id roundtrip
- GIVEN a `GraphNode { id: NodeId("main:1"), kind: NodeKind::Symbol(...), ... }` in space `SpaceId("repo-a")`
- WHEN `federated_id()` is called
- THEN the result equals `FederatedNodeId("repo-a::main:1".into())`

#### Scenario: Display produces wire format
- GIVEN a `FederatedNode` for space `repo-a` and local id `main:1`
- WHEN `format!("{node}")` runs
- THEN the output equals `"repo-a::main:1"`

### Requirement: FederatedGraphService Construction

`FederatedGraphService::new() -> Self` MUST produce an empty service with no spaces. `add_space(SpaceId, Arc<dyn GraphRepository>)` MUST register a per-space repository (idempotent — re-adding replaces the repo). `spaces() -> Vec<SpaceId>` MUST list the registered ids in insertion order.

#### Scenario: Empty service
- GIVEN `FederatedGraphService::new()`
- THEN `spaces().is_empty()`

#### Scenario: Add space is idempotent
- GIVEN a service with one space `repo-a`
- WHEN `add_space(SpaceId("repo-a"), repo2)` runs
- THEN `spaces().len() == 1` AND the stored repo is `repo2`

### Requirement: Federated Search

`federated_search(query, node_kinds, limit, cursor) -> ExplorerResult<FederatedSearchPage>` MUST fan out to every registered repository in parallel (`tokio::join_all`), merge results, tag each item with its `space_id`, and return a paginated `FederatedSearchPage { items: Vec<FederatedNode>, raw_total: u64, next_cursor: Option<String>, raw_rank: f64 }`. The total `raw_total` is the sum of per-space totals. Cursor encodes `(space_id, page_token)` pairs.

#### Scenario: Search merges two spaces
- GIVEN two repos: `repo-a` returns 3 nodes for `"Auth"`, `repo-b` returns 2 nodes for `"Auth"`
- WHEN `federated_search("Auth", &[], 10, None)` runs
- THEN `items.len() == 5` AND every item has a populated `space_id` AND `raw_total == 5`

#### Scenario: Empty service returns empty page
- GIVEN `FederatedGraphService::new()`
- WHEN `federated_search("x", &[], 10, None)` runs
- THEN the result MUST be `Ok(FederatedSearchPage { items: vec![], raw_total: 0, next_cursor: None, raw_rank: 0.0 })`

### Requirement: Federated Node Lookup

`get_node(FederatedNodeId) -> ExplorerResult<Option<FederatedNode>>` MUST parse the id, route to the correct space's repository, and wrap the result. `find_outgoing_edges(FederatedNodeId) -> ExplorerResult<Vec<GraphEdge>>` MUST do the same for edges.

#### Scenario: Routing to correct space
- GIVEN a service with `repo-a` (returns a node for `"main:1"`) and `repo-b` (returns nothing for `"main:1"`)
- WHEN `get_node(FederatedNodeId("repo-a::main:1"))` runs
- THEN the result is `Some(FederatedNode { node, space_id: SpaceId("repo-a") })`

#### Scenario: Unknown federated id
- GIVEN a service with only `repo-a`
- WHEN `get_node(FederatedNodeId("repo-b::main:1"))` runs
- THEN the result is `Ok(None)` (not an error)

### Requirement: Per-Space Namespace Isolation

The local `NodeId` returned inside a `FederatedNode` MUST be the **raw** local id as stored in the space's repo (e.g. `NodeId("main:1")`). The space prefix lives in the `FederatedNodeId` wrapper, not in the local id. The existing `NodeId` type MUST NOT be modified.

#### Scenario: Local id is unprefixed
- GIVEN a `FederatedNode` from `repo-a` for local `main:1`
- THEN `node.node.id.0 == "main:1"` (no `repo-a::` prefix)

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| One space's repo returns `Err` | The error propagates as a top-level `Err(ExplorerError)`; partial results are NOT returned |
| Two spaces return the same `raw_rank` for the same query | Results from `repo-a` come first (insertion order), then `repo-b` |
| `limit` is smaller than total matches | Items are interleaved round-robin across spaces (best-effort fairness) |
| `cursor` is malformed | The call returns `Err(ExplorerError::InvalidCursor)` |
| Federated search with `node_kinds` empty | All kinds are accepted (backward-compatible) |
| Two spaces have the same `SpaceId` | The second `add_space` call replaces the first (idempotent) |

## Out of Scope

- Cross-space edge creation (edges are space-local)
- Auto-federation (spaces are added manually via `brain_add_space`)
- Distributed query execution (assumes in-process repos)
- Result ranking across spaces (results are concatenated, not re-ranked)

## TDD RED Gate

Before implementation: (1) `FederatedNodeId::try_new` for valid / missing-separator / empty-segment; (2) `FederatedNode::federated_id` roundtrip; (3) `FederatedGraphService::add_space` idempotency + `spaces()` ordering; (4) mock-backed `federated_search` with 2 mock repos asserting merged items + per-item `space_id`; (5) `get_node` routing; (6) concurrent `federated_search` from 2 tasks asserting no panic. RED gate fails if any test passes before the module exists or the `multimodal` feature is off.

## Dependencies

- `federated-spaces` — `SpaceId` newtype
- `generic-graph-model` — `GraphNode`, `GraphEdge`, `NodeId`, `NodeKind`, `GraphRepository` trait
- `multimodal` feature gate

## Multimodal Feature Gate

Entire `federation/` module is `#[cfg(feature = "multimodal")]`. With the feature off, `FederatedGraphService` is unreachable and `brain_*` tools see the single-space behavior (default space). Default build stays byte-for-byte unchanged.
