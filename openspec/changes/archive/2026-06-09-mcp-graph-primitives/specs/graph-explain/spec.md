# Spec: graph-explain

> New capability. Companion to proposal `sdd/mcp-graph-primitives/proposal`.
> Consumed infrastructure: `CallGraphProjection::explain_path` (new method
> that wraps `dijkstra` and walks edge pairs).

## Purpose

MCP tool that **explains why two symbols are connected** — returns the
shortest confidence-weighted path between `from` and `to`, plus per-hop
edge metadata `(DependencyType, confidence)` and a per-hop
"rationale" string. Pure narration primitive: agent gets enough
information to summarize the connection in human-readable form, which
`has_path` (boolean) and `shortest_path` (just the path) cannot answer.

## Requirements

### Requirement 1: Tool constant + schema

`TOOL_GRAPH_EXPLAIN: &str = "graph_explain"` MUST be `pub` at
`crates/cognicode-explorer/src/mcp.rs`. The schema entry produced by
`build_tool_schemas()` MUST be a `Tool` whose `name == "graph_explain"`
and whose `input_schema` is a JSON object with `properties.from`
(string, required) and `properties.to` (string, required). The
`required` array MUST list `["from","to"]`. No optional fields are
introduced in this slice (`max_paths` deferred).

#### Scenario: Schema appears in tool list and matches the constant

- GIVEN a fresh `build_tool_schemas()` and `TOOL_NAMES` evaluation
- WHEN the schemas and names are enumerated
- THEN a schema with `name == "graph_explain"` exists AND the schema
  count is exactly 17 AND `TOOL_NAMES` contains `TOOL_GRAPH_EXPLAIN`

#### Scenario: Schema declares from and to as required

- GIVEN the `graph_explain` schema entry
- WHEN the JSON schema is parsed
- THEN `properties.from.type == "string"` AND
  `properties.to.type == "string"` AND `required` is a permutation of
  `["from","to"]`

### Requirement 2: Argument struct

`GraphExplainArgs { from: String, to: String }` MUST be `pub` at
`crates/cognicode-explorer/src/mcp.rs`, `#[derive(Deserialize, Debug)]`.
Missing `from` or `to` MUST surface as a deserialization error caught
and reported as `"missing required arg"`.

#### Scenario: Round-trips a fully-specified JSON payload

- GIVEN JSON `{"from":"A","to":"C"}`
- WHEN deserialized via `serde_json::from_value::<GraphExplainArgs>(v)`
- THEN `args.from == "A"` AND `args.to == "C"`

#### Scenario: Missing `to` fails deserialization

- GIVEN JSON `{"from":"A"}`
- WHEN deserialized
- THEN deserialization fails AND error mentions `"to"` or
  `"missing field"`

### Requirement 3: Dispatch arm

`ExplorerMcpHandler::dispatch` MUST contain a match arm for
`TOOL_GRAPH_EXPLAIN` that:
1. Deserializes args; on failure returns `is_error == true` with text
   containing `"missing required arg"` AND the tool name.
2. Calls `service.explain_path(graph, &from, &to)`.
3. On `graph == None` returns `is_error == true` with text containing
   `"graph analysis unavailable"`.
4. On `from == to` and both present, returns the explanation with one
   self-hop of cost `0.0` and one edge of the same node (`from →
   from`), `DependencyType::Calls` rendered as `"Calls"`,
   `confidence = 1.0`, `rationale` describing the self-path.
5. On missing endpoint or unreachable pair, returns `is_error == false`
   with payload `{"found": false, "hops": [], "total_cost": 0.0, "summary": "..."}`.
6. On success returns `is_error == false` with payload from
   `ExplainResultDto` via `ok_direct::<ExplainResultDto>`.

#### Scenario: Two-hop path is explained with per-hop metadata

- GIVEN handler with graph `A → B → C` (confidence 1.0 on both edges)
- WHEN `graph_explain` is called with `{"from":"A","to":"C"}`
- THEN `is_error == false` AND payload parses to
  `{"found": true, "hops": [{"from":"A","to":"B","dependency_type":"Calls","confidence":1.0,"rationale":"A calls B"}, {"from":"B","to":"C","dependency_type":"Calls","confidence":1.0,"rationale":"B calls C"}], "total_cost": 0.0, "summary": "A → B → C (2 hops, total cost 0.0)"}`
  AND `hops.len() == 2` AND each hop has a non-empty `rationale`

#### Scenario: Confidence below 1.0 is preserved per hop

- GIVEN handler with graph `A → B` (conf 0.9), `A → C → B` (0.5, 0.5)
- WHEN `graph_explain` is called with `{"from":"A","to":"B"}`
- THEN the chosen path is `A → B` (cheaper total cost) AND
  `hops[0].confidence == 0.9` AND `total_cost == 0.1`

#### Scenario: Unreachable pair returns `found: false` with empty hops

- GIVEN handler with graph `A → B` only
- WHEN `graph_explain` is called with `{"from":"A","to":"Z"}`
- THEN `is_error == false` AND `payload.found == false` AND
  `payload.hops == []` AND `payload.total_cost == 0.0` AND
  `payload.summary` is a non-empty human-readable message

#### Scenario: Missing endpoint returns `found: false`

- GIVEN handler with graph `A → B`
- WHEN `graph_explain` is called with `{"from":"A","to":"missing"}`
- THEN `is_error == false` AND `payload.found == false`

#### Scenario: Self-path `from == to` returns one self-hop

- GIVEN handler with graph containing symbol `A` (any edges, even none)
- WHEN `graph_explain` is called with `{"from":"A","to":"A"}`
- THEN `is_error == false` AND `payload.found == true` AND
  `payload.hops.len() == 1` AND
  `payload.hops[0] == {from:"A",to:"A",dependency_type:"Calls",confidence:1.0,rationale:"A → A (self)"}`

#### Scenario: Missing `from` argument yields a clear error

- GIVEN handler with any graph
- WHEN `graph_explain` is called with `{"to":"B"}`
- THEN `is_error == true` AND text contains `"missing required arg"` AND
  mentions `"graph_explain"`

#### Scenario: Missing `to` argument yields a clear error

- GIVEN handler with any graph
- WHEN `graph_explain` is called with `{"from":"A"}`
- THEN `is_error == true` AND text contains `"missing required arg"` AND
  mentions `"graph_explain"`

#### Scenario: Graph unavailable returns a clear error

- GIVEN a handler built with `new(service)` (`graph == None`)
- WHEN `graph_explain` is called with `{"from":"A","to":"B"}`
- THEN `is_error == true` AND text contains `"graph analysis unavailable"`
  AND mentions `"graph_explain"`

### Requirement 4: DTO contract

The result DTO `ExplainResultDto { found: bool, hops:
Vec<ExplainHopDto>, total_cost: f64, summary: String }` MUST be
defined in `application::dto::impact_dto`, `Serialize + Deserialize +
Debug + Clone`. `ExplainHopDto { from: String, to: String,
dependency_type: String, confidence: f64, rationale: String }` MUST be
the per-hop DTO. `summary` MUST be a one-line human-readable string of
the form `"<from> → <to1> → <to2> → ... → <destination> (N hops, total
cost C.CC)"` constructed server-side. `rationale` MUST be of the form
`"<from> <verb> <to>"` where `verb` is derived from
`DependencyType` (`Calls → "calls"`, `Implements → "implements"`,
`Extends → "extends"`, `Uses → "uses"`, `References → "references"`,
`Overrides → "overrides"`, fallback `"depends on"`).

#### Scenario: DTO round-trips through JSON

- GIVEN an `ExplainResultDto` with 2 hops and `found: true`
- WHEN serialized to JSON and deserialized back
- THEN `found`, `total_cost`, `summary`, and `hops` are preserved
  including the `rationale` strings

#### Scenario: Summary string is one line and contains the path

- GIVEN an explanation with hops `A→B→C` and `total_cost = 0.1`
- WHEN `ExplainResultDto::from_path(...)` builds the summary
- THEN `summary == "A → B → C (2 hops, total cost 0.10)"` AND it does
  not contain `\n`

#### Scenario: `DependencyType::Implements` maps to "implements" rationale

- GIVEN an edge `A → B` with `DependencyType::Implements`
- WHEN the hop is built
- THEN `rationale == "A implements B"`

### Requirement 5: Verb mapping coverage

The rationale verb mapping MUST cover all variants of `DependencyType`
defined in `cognicode-core`. If a new variant is added later, the
mapping MUST default to `"depends on"` and MUST NOT panic. A unit test
MUST exercise each variant.

#### Scenario: Unknown DependencyType uses fallback verb

- GIVEN a `DependencyType` value (existing variant) for which the
  helper has no explicit verb
- WHEN the rationale is built
- THEN the verb is `"depends on"` AND no panic occurs

## Acceptance Criteria

| #   | Criterion                                                                | Verifies       |
| --- | ------------------------------------------------------------------------ | -------------- |
| AC1 | `TOOL_GRAPH_EXPLAIN` constant, schema, and dispatch arm all present      | R1, R3         |
| AC2 | `ExplainResultDto` + `ExplainHopDto` added to `impact_dto.rs`            | R4, R5         |
| AC3 | Per-hop `(DependencyType, confidence)` preserved                          | R3, R4         |
| AC4 | `found: false` envelope returned for missing/unreachable (not `is_error`) | R3         |
| AC5 | `cargo test -p cognicode-explorer` passes with new dispatch tests        | R3             |
| AC6 | Existing 14 tools remain unchanged; `TOOL_NAMES` now has 17 entries      | R1             |

## Edge Cases (exhaustive — all MUST have ≥1 test)

| ID  | Case                              | Expected behavior                              |
| --- | --------------------------------- | ---------------------------------------------- |
| E1  | Empty graph                       | `found: false`, `hops: []`                     |
| E2  | Missing endpoint                  | `found: false`, `hops: []`                     |
| E3  | Unreachable pair                  | `found: false`, `hops: []`                     |
| E4  | Direct edge `A → B`               | 1 hop, `total_cost == 1.0 - conf`              |
| E5  | Multi-hop path                    | Each hop has its own metadata                  |
| E6  | Self-path `from == to`            | 1 self-hop, `total_cost == 0.0`                |
| E7  | NaN confidence on traversed edge  | Cost `0.0`; `confidence` rendered as `1.0`     |
| E8  | Cycle on the path                 | Visited set prevents infinite loop             |
| E9  | Multiple shortest paths           | Any one of them is returned (deterministic)    |
| E10 | Missing `from` or `to`            | `is_error == true` with `"missing required arg"` |
| E11 | Graph unavailable (None)          | `is_error == true` with `"graph analysis unavailable"` |
| E12 | Unknown DependencyType            | Rationale uses fallback `"depends on"`         |

## Out of Scope (locked)

- `max_paths` multi-path support
- Per-hop source location / `EvidenceBlock` enrichment
- Per-hop direction labels on reverse paths
- Cycle highlighting in the explanation
- Natural-language generation beyond a templated summary
- Visualization of the path
- Caching of explanations
- Async explanation over large graphs
