# Spec: graph-subgraph

> New capability. Companion to proposal `sdd/mcp-graph-primitives/proposal`.
> Consumed infrastructure: `CallGraphProjection::extract_subgraph` (read-only).

## Purpose

MCP tool that returns a **neighborhood subgraph** around a root symbol — the
nodes and edges reachable within `max_depth` hops, with `(DependencyType,
confidence)` metadata per edge. Direction may be `incoming` (predecessors),
`outgoing` (successors), or `both` (union, two-pass BFS). Self-contained
primitive for "show me everything around X" refactoring questions that the
existing flat predecessor/successor lists cannot answer.

## Requirements

### Requirement 1: Tool constant + schema

`TOOL_GRAPH_SUBGRAPH: &str = "graph_subgraph"` MUST be `pub` at
`crates/cognicode-explorer/src/mcp.rs`. The schema entry produced by
`build_tool_schemas()` MUST be a `Tool` whose `name == "graph_subgraph"`
and whose `input_schema` is a JSON object with `properties.root` (string,
required), `properties.direction` (string, enum `[incoming, outgoing,
both]`, default `"outgoing"`), `properties.max_depth` (integer, optional,
default `DEFAULT_SUBGRAPH_DEPTH = 3`). The `direction` and `max_depth`
fields MUST be `optional` in the schema. A `required` array MUST list
`["root"]` only.

#### Scenario: Schema appears in tool list and matches the constant

- GIVEN a fresh `build_tool_schemas()` and `TOOL_NAMES` evaluation
- WHEN the schemas and names are enumerated
- THEN a schema with `name == "graph_subgraph"` exists AND the schema
  count is exactly 17 AND `TOOL_NAMES` contains `TOOL_GRAPH_SUBGRAPH`

#### Scenario: Schema declares root as required and direction/max_depth optional

- GIVEN the `graph_subgraph` schema entry
- WHEN the JSON schema is parsed
- THEN `properties.root` is `type: "string"` AND `required == ["root"]`
  AND `properties.direction.enum == ["incoming","outgoing","both"]`
  AND `properties.max_depth` has no `required` membership

### Requirement 2: Argument struct

`GraphSubgraphArgs { root: String, direction: Option<String>, max_depth:
Option<usize> }` MUST be `pub` at `crates/cognicode-explorer/src/mcp.rs`,
`#[derive(Deserialize, Debug)]`. `serde(default)` MUST NOT be applied at
the struct level; missing `root` MUST surface as a clear "missing
required arg" error, consistent with existing tools.

#### Scenario: Round-trips a fully-specified JSON payload

- GIVEN JSON `{"root":"A","direction":"both","max_depth":2}`
- WHEN deserialized via `serde_json::from_value::<GraphSubgraphArgs>(v)`
- THEN `args.root == "A"` AND `args.direction == Some("both")` AND
  `args.max_depth == Some(2)`

#### Scenario: Missing `root` fails deserialization

- GIVEN JSON `{}`
- WHEN deserialized
- THEN deserialization fails AND the error mentions `"root"` or
  `"missing field"`

### Requirement 3: Dispatch arm

`ExplorerMcpHandler::dispatch` MUST contain a match arm for
`TOOL_GRAPH_SUBGRAPH` that:
1. Deserializes args; on failure returns `is_error == true` with text
   containing `"missing required arg"` AND the tool name.
2. Resolves `direction` with default `"outgoing"`.
3. Resolves `max_depth` with default `DEFAULT_SUBGRAPH_DEPTH = 3`.
4. Calls `service.graph_subgraph(graph, &root, &direction, max_depth)`.
5. On `graph == None` returns `is_error == true` with text containing
   `"graph analysis unavailable"`.
6. On success serializes the result via `ok_direct::<T>` and returns
   `is_error == false`.

#### Scenario: Outgoing direction at depth 2 returns the 3-node subgraph

- GIVEN handler with graph `A → B → C`, `A → D`, no graph = None
- WHEN `graph_subgraph` is called with `{"root":"A","direction":"outgoing","max_depth":2}`
- THEN `is_error == false` AND payload parses to an object
  `{nodes: ["A","B","C","D"], edges: [{from:"A",to:"B",type:"Calls",confidence:1.0}, {from:"B",to:"C",...}, {from:"A",to:"D",...}]}`
  AND `A` is in `nodes`

#### Scenario: Incoming direction returns predecessors

- GIVEN handler with graph `D → A → C`, `B → C`
- WHEN `graph_subgraph` is called with `{"root":"C","direction":"incoming","max_depth":2}`
- THEN `is_error == false` AND payload `nodes` equals `["A","B","C","D"]`
  (any order) AND `edges` contains `(A→C)`, `(B→C)`, `(D→A)`

#### Scenario: `direction: both` is the union of incoming + outgoing

- GIVEN handler with graph `D → A → C`, `B → C`
- WHEN `graph_subgraph` is called with `{"root":"A","direction":"both","max_depth":2}`
- THEN `nodes` equals `["A","B","C","D"]` (union of predecessors and
  successors of `A`) AND `edges` contains all three `(D→A)`, `(A→C)`,
  `(B→C)` entries

#### Scenario: Default `max_depth` is 3 (not 5 like impact tools)

- GIVEN handler with a chain of length 7: `a1 → a2 → ... → a7`
- WHEN `graph_subgraph` is called with `{"root":"a7","direction":"incoming"}`
  and no `max_depth`
- THEN `nodes` length is `4` (`a7` + 3 closest predecessors `a4, a5, a6`)
  AND `a1, a2, a3` are NOT in `nodes`

#### Scenario: `max_depth: 0` returns just the root

- GIVEN handler with graph `A → B`
- WHEN `graph_subgraph` is called with `{"root":"A","max_depth":0}`
- THEN `nodes == ["A"]` AND `edges == []`

#### Scenario: Unknown `root` returns subgraph with just the root id

- GIVEN handler with graph `A → B`
- WHEN `graph_subgraph` is called with `{"root":"missing","max_depth":5}`
- THEN `is_error == false` AND `nodes == ["missing"]` AND `edges == []`
  (root is always echoed back, even when unknown)

#### Scenario: Invalid `direction` value is rejected

- GIVEN handler with any graph
- WHEN `graph_subgraph` is called with `{"root":"A","direction":"sideways"}`
- THEN `is_error == true` AND text contains `"invalid direction"`

#### Scenario: Missing `root` argument yields a clear error

- GIVEN handler with any graph
- WHEN `graph_subgraph` is called with `{}`
- THEN `is_error == true` AND text contains `"missing required arg"` AND
  mentions `"graph_subgraph"`

#### Scenario: Graph unavailable returns a clear error

- GIVEN a handler built with `new(service)` (`graph == None`)
- WHEN `graph_subgraph` is called with `{"root":"x"}`
- THEN `is_error == true` AND text contains `"graph analysis unavailable"`
  AND mentions `"graph_subgraph"`

### Requirement 4: DTO contract

The result DTO `SubgraphResultDto { nodes: Vec<String>, edges:
Vec<SubgraphEdgeDto> }` MUST be defined in
`application::dto::impact_dto`, `Serialize + Deserialize + Debug + Clone`.
`SubgraphEdgeDto { from: String, to: String, dependency_type: String,
confidence: f64 }` MUST be the edge DTO; `dependency_type` MUST be the
`Debug` rendering of `DependencyType` (e.g. `"Calls"`, `"Implements"`).
The root MUST appear in `nodes`. Edge list MUST NOT contain duplicates;
each edge MUST appear at most once. Order of `nodes` and `edges` is not
asserted.

#### Scenario: DTO round-trips through JSON

- GIVEN a `SubgraphResultDto` with 2 nodes and 1 edge
- WHEN serialized to JSON and deserialized back
- THEN field count, types, and values are preserved
  (nodes as strings, edges with from/to/dependency_type/confidence)

#### Scenario: Root appears in `nodes` even when it has no edges

- GIVEN a graph with isolated symbol `X`
- WHEN `SubgraphResultDto` is built from `extract_subgraph(X, ...)`
- THEN `nodes` contains `"X"` AND `edges` is empty

### Requirement 5: Result-DTO `#[serde(rename)]` for MCP-friendly field names

The JSON-serialized field names MUST be `nodes` and `edges` at the
top-level, and `from`, `to`, `dependency_type`, `confidence` on each
edge — using `#[serde(rename = "...")]` if the Rust field names differ
from these public names. The DTO MUST NOT require post-construction
mapping before `ok_direct::<SubgraphResultDto>`.

#### Scenario: JSON field names match the spec exactly

- GIVEN a `SubgraphResultDto` with one edge `(A→B, Calls, 0.9)`
- WHEN serialized to JSON
- THEN the JSON is `{"nodes":["A","B"],"edges":[{"from":"A","to":"B","dependency_type":"Calls","confidence":0.9}]}`

## Acceptance Criteria

| #   | Criterion                                                                | Verifies       |
| --- | ------------------------------------------------------------------------ | -------------- |
| AC1 | `TOOL_GRAPH_SUBGRAPH` constant, schema, and dispatch arm all present      | R1, R3         |
| AC2 | `SubgraphResultDto` + `SubgraphEdgeDto` added to `impact_dto.rs`          | R4, R5         |
| AC3 | Default `max_depth == 3` constant `DEFAULT_SUBGRAPH_DEPTH` introduced     | R1, R3         |
| AC4 | All 3 direction values (`incoming`/`outgoing`/`both`) yield correct sets  | R3             |
| AC5 | `cargo test -p cognicode-explorer` passes with new dispatch tests        | R3             |
| AC6 | Existing 14 tools remain unchanged; `TOOL_NAMES` now has 17 entries      | R1             |

## Edge Cases (exhaustive — all MUST have ≥1 test)

| ID  | Case                            | Expected behavior                              |
| --- | ------------------------------- | ---------------------------------------------- |
| E1  | Empty graph                     | `nodes == [root]`, `edges == []`               |
| E2  | Unknown root                    | `nodes == [root]`, `edges == []`               |
| E3  | `max_depth == 0`                | Only the root node                             |
| E4  | `max_depth == usize::MAX`       | Full reachable neighborhood                    |
| E5  | Dense graph (high fan-in/out)   | Termination; no duplicate edges                |
| E6  | Self-loop on root               | `edges` contains `(root→root)`                 |
| E7  | Cycle reachable from root       | Visited set prevents duplication; terminates   |
| E8  | Missing `direction`             | Defaults to `"outgoing"`                       |
| E9  | Invalid `direction`             | `is_error == true` with `"invalid direction"`  |
| E10 | Missing `root`                  | `is_error == true` with `"missing required arg"` |
| E11 | Graph unavailable (None)        | `is_error == true` with `"graph analysis unavailable"` |
| E12 | Direction `both` on a DAG with isolated root | `nodes == [root]`, `edges == []`    |

## Out of Scope (locked)

- `max_nodes` response cap (deferred; default depth=3 mitigates bloat)
- Edge provenance / `EvidenceBlock` enrichment (projection stores only
  `(DependencyType, f64)`)
- Async / streaming response
- Persisting subgraph results to disk
- Cycle-aware path planning within the subgraph
- Subgraph diffing between two roots
- Visualization (DOT, Mermaid, etc.)
