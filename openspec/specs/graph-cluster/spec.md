# Spec: graph-cluster

> New capability. Companion to proposal `sdd/mcp-graph-primitives/proposal`.
> Consumed infrastructure: `CallGraphProjection::strongly_connected_components`
> and `CallGraphProjection::connected_components` (both pre-existing, no new
> projection logic).

## Purpose

MCP tool that exposes cluster detection on the call graph: **strongly
connected components** (Tarjan, detects mutual cycles / tightly-coupled
refactoring candidates) and **undirected connected components** (groups
symbols by reachability ignoring direction). Self-contained primitive
for "how does this graph partition?" — answers the structural question
that complement finding and path narration cannot.

## Requirements

### Requirement 1: Tool constant + schema

`TOOL_GRAPH_CLUSTER: &str = "graph_cluster"` MUST be `pub` at
`crates/cognicode-explorer/src/mcp.rs`. The schema entry produced by
`build_tool_schemas()` MUST be a `Tool` whose `name == "graph_cluster"`
and whose `input_schema` is a JSON object with `properties.method`
(string, enum `[scc, connected]`, default `"scc"`). The `method` field
MUST be optional in the schema. The `required` array MUST be empty
(`method` is not required since it has a default).

#### Scenario: Schema appears in tool list and matches the constant

- GIVEN a fresh `build_tool_schemas()` and `TOOL_NAMES` evaluation
- WHEN the schemas and names are enumerated
- THEN a schema with `name == "graph_cluster"` exists AND the schema
  count is exactly 17 AND `TOOL_NAMES` contains `TOOL_GRAPH_CLUSTER`

#### Scenario: Schema declares `method` as optional with default scc

- GIVEN the `graph_cluster` schema entry
- WHEN the JSON schema is parsed
- THEN `properties.method.enum == ["scc","connected"]` AND
  `required == []`

### Requirement 2: Argument struct

`GraphClusterArgs { method: Option<String> }` MUST be `pub` at
`crates/cognicode-explorer/src/mcp.rs`, `#[derive(Deserialize, Debug)]`.
An empty JSON `{}` MUST deserialize successfully (since `method` is
optional).

#### Scenario: Round-trips a fully-specified JSON payload

- GIVEN JSON `{"method":"connected"}`
- WHEN deserialized via `serde_json::from_value::<GraphClusterArgs>(v)`
- THEN `args.method == Some("connected")`

#### Scenario: Empty JSON deserializes with `method == None`

- GIVEN JSON `{}`
- WHEN deserialized
- THEN `args.method == None` AND deserialization succeeds

### Requirement 3: Dispatch arm

`ExplorerMcpHandler::dispatch` MUST contain a match arm for
`TOOL_GRAPH_CLUSTER` that:
1. Deserializes args; on failure returns `is_error == true` with text
   containing `"missing required arg"` AND the tool name.
2. Resolves `method` with default `"scc"`.
3. On `method == "scc"` calls
   `service.graph_cluster(graph, "scc")` returning SCCs.
4. On `method == "connected"` calls
   `service.graph_cluster(graph, "connected")` returning undirected
   components.
5. On any other method returns `is_error == true` with text containing
   `"invalid method"`.
6. On `graph == None` returns `is_error == true` with text containing
   `"graph analysis unavailable"`.
7. On success serializes via `ok_direct::<ClusterResultDto>`.

#### Scenario: SCC method detects a mutual cycle

- GIVEN handler with graph `A → B → A` plus an isolated `C`
- WHEN `graph_cluster` is called with `{"method":"scc"}`
- THEN `is_error == false` AND payload parses to a list of two entries:
  one cluster `["A","B"]` (size 2) AND one cluster `["C"]` (size 1,
  singleton)

#### Scenario: Connected method treats directed edges as undirected

- GIVEN handler with graph `A → B`, `B → A` (mutual cycle) and `C → D`
- WHEN `graph_cluster` is called with `{"method":"connected"}`
- THEN `is_error == false` AND payload is a list of two clusters:
  `["A","B"]` (size 2) AND `["C","D"]` (size 2). Note: `A→B` alone (no
  reverse) would still put A,B in one connected cluster because
  connectivity is undirected.

#### Scenario: Default method is `scc`

- GIVEN handler with graph `A → B → A` plus a DAG section `E → F`
- WHEN `graph_cluster` is called with `{}` (no `method`)
- THEN payload contains `["A","B"]` as one cluster AND
  `["E"]` and `["F"]` as two singletons (3 clusters total under SCC)

#### Scenario: DAG returns one cluster per node under SCC

- GIVEN handler with graph `A → B → C` only
- WHEN `graph_cluster` is called with `{"method":"scc"}`
- THEN payload length is 3 AND each cluster has size 1

#### Scenario: Empty graph returns an empty list

- GIVEN handler with `CallGraph::new()` (no symbols)
- WHEN `graph_cluster` is called with `{"method":"scc"}`
- THEN `is_error == false` AND payload parses to an empty list `[]`

#### Scenario: Self-loop is a singleton cluster

- GIVEN handler with graph containing `A → A` only
- WHEN `graph_cluster` is called with `{"method":"scc"}`
- THEN payload is `[["A"]]` (self-loops are size-1 SCCs)

#### Scenario: Invalid `method` value is rejected

- GIVEN handler with any graph
- WHEN `graph_cluster` is called with `{"method":"biconnected"}`
- THEN `is_error == true` AND text contains `"invalid method"` AND
  mentions `"graph_cluster"`

#### Scenario: Graph unavailable returns a clear error

- GIVEN a handler built with `new(service)` (`graph == None`)
- WHEN `graph_cluster` is called with `{"method":"scc"}`
- THEN `is_error == true` AND text contains `"graph analysis unavailable"`
  AND mentions `"graph_cluster"`

### Requirement 4: DTO contract

The result DTO `ClusterResultDto` MUST be
`Vec<ClusterDto>` (a list of clusters). `ClusterDto { members:
Vec<String>, size: usize }` MUST be defined in
`application::dto::impact_dto`, `Serialize + Deserialize + Debug + Clone`.
`size` MUST equal `members.len()`. Symbol ids MUST be converted to
`String` via `symbol_id.as_str().to_string()`. Order of clusters and
order of `members` within a cluster are not asserted.

#### Scenario: DTO round-trips through JSON

- GIVEN a `ClusterResultDto` with two clusters: `[["A","B"],["C"]]`
- WHEN serialized to JSON and deserialized back
- THEN the structure is preserved (list of `{members, size}` objects)

#### Scenario: `size` matches `members.len()`

- GIVEN a cluster built from `vec![A, B, A]`
- WHEN `ClusterDto::from_scc(scc)` is constructed
- THEN `size == 3` AND `members == ["A","B","A"]`

## Acceptance Criteria

| #   | Criterion                                                                | Verifies       |
| --- | ------------------------------------------------------------------------ | -------------- |
| AC1 | `TOOL_GRAPH_CLUSTER` constant, schema, and dispatch arm all present       | R1, R3         |
| AC2 | `ClusterResultDto` + `ClusterDto` added to `impact_dto.rs`               | R4             |
| AC3 | Both `scc` and `connected` methods work via the same dispatch arm        | R3             |
| AC4 | Default method is `"scc"`                                                | R3             |
| AC5 | `cargo test -p cognicode-explorer` passes with new dispatch tests        | R3             |
| AC6 | Existing 14 tools remain unchanged; `TOOL_NAMES` now has 17 entries      | R1             |

## Edge Cases (exhaustive — all MUST have ≥1 test)

| ID  | Case                            | Expected behavior                              |
| --- | ------------------------------- | ---------------------------------------------- |
| E1  | Empty graph                     | `payload == []`                                |
| E2  | Single node, no edges           | One singleton cluster                          |
| E3  | Mutual cycle `A↔B`              | One cluster of size 2 (SCC) or 2 (connected)   |
| E4  | Self-loop on one node           | One singleton cluster                          |
| E5  | DAG of N nodes                  | N singleton clusters (SCC)                     |
| E6  | `method: "connected"` on mutual cycle | One cluster of size 2                    |
| E7  | `method: "connected"` with isolated node `Z` | `Z` is a singleton cluster       |
| E8  | Missing `method`                | Defaults to `"scc"`                            |
| E9  | Invalid `method` value          | `is_error == true` with `"invalid method"`     |
| E10 | Graph unavailable (None)        | `is_error == true` with `"graph analysis unavailable"` |
| E11 | Multiple disjoint cycles        | One cluster per cycle (SCC); length matches count |

## Out of Scope (locked)

- New projection logic (Tarjan/BFS already exist in `CallGraphProjection`)
- Hierarchical / nested clustering
- Cluster statistics (density, internal edge count, etc.)
- Persisting cluster results
- Visualization
- Louvain / modularity-based community detection
- Biconnected components
