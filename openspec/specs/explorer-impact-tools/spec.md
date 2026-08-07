# Spec: explorer-impact-tools

> New capability. Companion to proposal `sdd/mcp-impact-tool/proposal`.
> Consumed (read-only): `ImpactAnalysisService`, `CallGraphProjection`, DTOs
> `PathResultDto` and `SccDto`. No domain or core changes.

## Purpose

Expose `ImpactAnalysisService` through six new MCP tools in the
`cognicode-explorer` handler so external agents can answer graph-aware
impact questions (`impact_radius`, `impact_forward_radius`, `impact_has_path`,
`impact_shortest_path`, `impact_detect_cycles`, `impact_component`) without
switching MCP servers. The capability is a **pure extension** of the
existing 8-tool surface: no existing spec is modified, no DTO is changed,
no domain logic is duplicated.

Direction semantics: `impact_radius` is **predecessor-only** (reverse BFS).
`impact_forward_radius` is the symmetric **successor-only** (forward BFS)
counterpart introduced by change `forward-reach-impact`.

## Requirements

### Requirement 1: Handler holds an optional in-memory call graph

`ExplorerMcpHandler` MUST expose a `pub fn with_graph(service: Arc<ExplorerService>, graph: Option<Arc<CallGraph>>) -> Self`
constructor and a `graph: Option<Arc<CallGraph>>` field. The pre-existing
`pub fn new(service: Arc<ExplorerService>) -> Self` constructor MUST
continue to exist and MUST default `graph` to `None` (backward compatible —
no call site in `bin/mcp.rs` or the integration tests may change). The
handler MUST remain `Clone` (it is shared across async dispatch).

#### Scenario: `new()` defaults graph to None

- GIVEN `Arc<ExplorerService>` and a fresh `ExplorerMcpHandler::new(service)`
- WHEN the handler is constructed
- THEN it compiles AND it satisfies the `ServerHandler` trait
- AND `list_tools` returns 14 schemas (8 existing + 6 new)

#### Scenario: `with_graph(Some)` makes the graph available to impact arms

- GIVEN `service = Arc::new(...)` and `graph = Some(Arc::new(CallGraph::with_edges(...)))`
- WHEN `ExplorerMcpHandler::with_graph(service, graph.clone())` is called
- THEN the resulting handler holds `graph == Some(...)` AND both
  `service` and `graph` are reachable for dispatch

#### Scenario: `with_graph(None)` is identical to `new()`

- GIVEN `service = Arc::new(...)`
- WHEN `ExplorerMcpHandler::with_graph(service.clone(), None)` and
  `ExplorerMcpHandler::new(service.clone())` are called
- THEN both handlers report `list_tools` length 14
- AND dispatching any of the 6 impact tools from either handler returns
  `is_error == true` with text containing `"impact analysis unavailable"`

### Requirement 2: Tool list contract — 14 total, `impact_*` prefix unique

`build_tool_schemas()` MUST return exactly 14 tools. Six tool
constants MUST be exposed at `crates/cognicode-explorer/src/mcp.rs`:

- `TOOL_IMPACT_RADIUS: &str = "impact_radius"`
- `TOOL_IMPACT_FORWARD_RADIUS: &str = "impact_forward_radius"` (added by `forward-reach-impact`)
- `TOOL_IMPACT_HAS_PATH: &str = "impact_has_path"`
- `TOOL_IMPACT_SHORTEST_PATH: &str = "impact_shortest_path"`
- `TOOL_IMPACT_DETECT_CYCLES: &str = "impact_detect_cycles"`
- `TOOL_IMPACT_COMPONENT: &str = "impact_component"`

`TOOL_NAMES` MUST list all 14 in the original order followed by the 6
impact tools last. The 6 tool names MUST NOT collide with the 8
existing tool names. The `mcp_tool_names_match_spec` integration test
MUST assert the 14-name set.

#### Scenario: `tool_names()` returns 14 entries

- GIVEN a fresh `cognicode_explorer::mcp::tool_names()` call
- WHEN it is evaluated
- THEN `actual.len() == 14`
- AND `actual` contains every existing `TOOL_*` constant
- AND `actual` contains the 6 `TOOL_IMPACT_*` constants (5 existing + `TOOL_IMPACT_FORWARD_RADIUS`)

#### Scenario: Schema count matches the constant list

- GIVEN a fresh `build_tool_schemas()` call
- WHEN it is evaluated
- THEN the returned `Vec<Tool>` has length 14
- AND every name in the vector is also in `TOOL_NAMES`
- AND no two schemas share the same `name`
- AND `TOOL_IMPACT_FORWARD_RADIUS` is present

### Requirement 3: `impact_radius` tool

The `impact_radius` MCP tool MUST delegate to
`ImpactAnalysisService::impact_radius(&self, graph, root, max_depth)`.
It MUST accept two arguments:

- `root` (string, required): symbol id to analyze.
- `max_depth` (integer, optional): when omitted, the tool MUST default to
  a finite, project-wide constant `DEFAULT_IMPACT_RADIUS_DEPTH: usize = 5`.
  This same constant is shared with `impact_forward_radius` — no second
  default constant is introduced.

It MUST return the symbol ids (as strings) of all predecessors of `root`
reachable within `max_depth` reverse hops. The result MUST be serialized
as a JSON array of strings (the existing `ok_direct<T: Serialize>`
helper). When `graph` is `None`, the tool MUST return
`is_error == true` with text containing `"impact analysis unavailable"`.

#### Scenario: Returns predecessors as a JSON string array

- GIVEN handler with graph `D → A → C`, `B → C`
- WHEN `impact_radius` is called with `{"root": "test.rs:C:1", "max_depth": 2}`
- THEN the result is `is_error == false` AND its JSON payload parses to
  a `Vec<String>` containing exactly `["test.rs:A:1", "test.rs:B:1", "test.rs:D:1"]`
  (order not asserted)

#### Scenario: Missing `root` argument yields a clear error

- GIVEN handler with any graph
- WHEN `impact_radius` is called with `{}` (no `root`)
- THEN `is_error == true` AND the text contains `"missing required arg"`
  AND the text mentions the tool name `"impact_radius"`

#### Scenario: `max_depth` omitted defaults to 5

- GIVEN handler with graph chain `a1 → a2 → a3 → a4 → a5 → a6 → a7` (length 7)
- WHEN `impact_radius` is called with `{"root": "a7"}` and no `max_depth`
- THEN the result is `is_error == false`
- AND the returned array contains exactly 5 symbols (the 5 closest
  predecessors), not 6 and not all 6

#### Scenario: `max_depth == 0` returns an empty array

- GIVEN handler with graph `A → B`
- WHEN `impact_radius` is called with `{"root": "B", "max_depth": 0}`
- THEN result is `is_error == false` AND payload parses to `Vec<String>` of length 0

#### Scenario: Unknown `root` returns an empty array (no panic)

- GIVEN handler with graph `A → B`
- WHEN `impact_radius` is called with `{"root": "missing", "max_depth": 5}`
- THEN result is `is_error == false` AND payload parses to empty `Vec<String>`

#### Scenario: Empty graph returns an empty array

- GIVEN handler with `CallGraph::new()` (no symbols, no edges)
- WHEN `impact_radius` is called with `{"root": "anything", "max_depth": 5}`
- THEN result is `is_error == false` AND payload parses to empty `Vec<String>`

#### Scenario: Graph unavailable returns a clear error

- GIVEN a handler built with `new(service)` (`graph == None`)
- WHEN `impact_radius` is called with `{"root": "x", "max_depth": 1}`
- THEN `is_error == true` AND the text contains `"impact analysis unavailable"`
- AND the text mentions the tool name

### Requirement 3b: `impact_forward_radius` tool (added by `forward-reach-impact`)

The `impact_forward_radius` MCP tool MUST delegate to
`ImpactAnalysisService::forward_radius(&self, graph, root, max_depth)`.
It MUST accept two arguments:

- `root` (string, required): symbol id to analyze.
- `max_depth` (integer, optional): when omitted, defaults to
  `DEFAULT_IMPACT_RADIUS_DEPTH` (5) — the same constant as `impact_radius`.
  When omitted, behaves identically to `impact_radius`'s default-depth branch.

It MUST return the symbol ids (as strings) of all **successors** of `root`
reachable within `max_depth` forward hops. The result MUST be serialized as
a JSON array of strings via `ok_direct<T: Serialize>`. The `root` itself
MUST NOT appear in the result. When `graph` is `None`, the tool MUST return
`is_error == true` with text containing `"impact analysis unavailable"`.
The constant `TOOL_IMPACT_FORWARD_RADIUS` MUST be `pub` and listed last in
`TOOL_NAMES` (after the 5 existing impact tools).

#### Scenario: Returns successors as a JSON string array
- GIVEN handler with graph `A → B → C`, `A → D`
- WHEN `impact_forward_radius` is called with `{"root": "A", "max_depth": 2}`
- THEN `is_error == false` AND the JSON payload parses to a
  `Vec<String>` containing exactly `["B", "C", "D"]` (order not asserted)
  AND `A` is NOT in the payload

#### Scenario: Missing `root` argument yields a clear error
- GIVEN handler with any graph
- WHEN `impact_forward_radius` is called with `{}` (no `root`)
- THEN `is_error == true` AND the text contains `"missing required arg"`
  AND the text mentions the tool name `"impact_forward_radius"`

#### Scenario: `max_depth` omitted defaults to 5
- GIVEN handler with graph chain `a1 → a2 → a3 → a4 → a5 → a6 → a7` (length 7)
- WHEN `impact_forward_radius` is called with `{"root": "a1"}` and no `max_depth`
- THEN the returned array contains exactly 5 symbols (the 5 closest successors),
  not 6 and not all 6

#### Scenario: `max_depth == 0` returns an empty array
- GIVEN handler with graph `A → B`
- WHEN `impact_forward_radius` is called with `{"root": "A", "max_depth": 0}`
- THEN result is `is_error == false` AND payload parses to `Vec<String>` of length 0

#### Scenario: Unknown `root` returns an empty array (no panic)
- GIVEN handler with graph `A → B`
- WHEN `impact_forward_radius` is called with `{"root": "missing", "max_depth": 5}`
- THEN result is `is_error == false` AND payload parses to empty `Vec<String>`

#### Scenario: Cycle reachable, root excluded
- GIVEN handler with graph `A → B → C → A` (cycle includes root)
- WHEN `impact_forward_radius` is called with `{"root": "A", "max_depth": 100}`
- THEN result is `is_error == false` AND payload parses to `Vec<String>`
  containing `["B", "C"]` (order not asserted) AND `A` is NOT in the payload
  AND the call terminates

#### Scenario: Disconnected successor returns empty
- GIVEN handler with graph `A → B` and an isolated `Z`
- WHEN `impact_forward_radius` is called with `{"root": "Z", "max_depth": 5}`
- THEN result is `is_error == false` AND payload parses to empty `Vec<String>`

#### Scenario: Empty graph returns an empty array
- GIVEN handler with `CallGraph::new()` (no symbols, no edges)
- WHEN `impact_forward_radius` is called with `{"root": "anything", "max_depth": 5}`
- THEN result is `is_error == false` AND payload parses to empty `Vec<String>`

#### Scenario: Graph unavailable returns a clear error
- GIVEN a handler built with `new(service)` (`graph == None`)
- WHEN `impact_forward_radius` is called with `{"root": "x", "max_depth": 1}`
- THEN `is_error == true` AND the text contains `"impact analysis unavailable"`
  AND the text mentions the tool name

### Requirement 4: `impact_has_path` tool

The `impact_has_path` MCP tool MUST delegate to
`ImpactAnalysisService::has_path`. It MUST accept two required string
arguments: `from` and `to`. The response payload MUST be a JSON object
`{"from": String, "to": String, "has_path": bool}`. When `graph` is
`None`, the tool MUST return `is_error == true` with text containing
`"impact analysis unavailable"`.

#### Scenario: Direct, transitive, and no-path cases

- GIVEN handler with graph `A → B → C` and an unrelated `D`
- WHEN `impact_has_path` is called with `{"from": "A", "to": "B"}`,
  `{"from": "A", "to": "C"}`, and `{"from": "D", "to": "A"}`
- THEN payloads parse to objects with `has_path` equal to `true`,
  `true`, and `false` respectively
- AND each echoed `from` and `to` matches the input

#### Scenario: Missing endpoint returns `has_path == false` (no panic)

- GIVEN handler with graph `A → B`
- WHEN `impact_has_path` is called with `{"from": "A", "to": "missing"}`
- THEN result is `is_error == false` AND `has_path == false`

#### Scenario: Self-path for present node returns `has_path == true`

- GIVEN handler with graph containing `A` (no edges required)
- WHEN `impact_has_path` is called with `{"from": "A", "to": "A"}`
- THEN result is `is_error == false` AND `has_path == true`

#### Scenario: Missing argument yields a clear error

- GIVEN handler with any graph
- WHEN `impact_has_path` is called with `{"from": "A"}` (no `to`)
- THEN `is_error == true` AND the text contains `"missing required arg"`

#### Scenario: Graph unavailable returns a clear error

- GIVEN a handler with `graph == None`
- WHEN `impact_has_path` is called with `{"from": "A", "to": "B"}`
- THEN `is_error == true` AND the text contains `"impact analysis unavailable"`

### Requirement 5: `impact_shortest_path` tool

The `impact_shortest_path` MCP tool MUST delegate to
`ImpactAnalysisService::shortest_path`. It MUST accept two required
string arguments: `from` and `to`. The response payload MUST be the
JSON-serialized `PathResultDto { path, total_cost, found }`. When no
path exists or an endpoint is missing, the tool MUST return
`is_error == false` with payload `null` (the `Option<PathResultDto>`
serializes as JSON `null` and is wrapped via `ok_direct`). When `graph`
is `None`, the tool MUST return `is_error == true` with text containing
`"impact analysis unavailable"`.

#### Scenario: Returns the cheapest path DTO

- GIVEN handler with graph `A → B` (confidence 1.0) and `A → C → B`
  (confidence 0.5 each)
- WHEN `impact_shortest_path` is called with `{"from": "A", "to": "B"}`
- THEN result is `is_error == false`
- AND the JSON payload has `"found": true`, `"total_cost": 0.0`
  (within `1e-9`), and `"path": ["A", "B"]`

#### Scenario: Unreachable target returns `null` payload (not an error)

- GIVEN handler with graph `A → B` only
- WHEN `impact_shortest_path` is called with `{"from": "A", "to": "C"}`
- THEN result is `is_error == false` AND payload text equals `"null"`
  (when pretty-printed, `Value::Null`)

#### Scenario: Missing endpoint returns `null` payload (no panic)

- GIVEN handler with graph `A → B`
- WHEN `impact_shortest_path` is called with `{"from": "A", "to": "missing"}`
- THEN result is `is_error == false` AND payload text equals `"null"`

#### Scenario: Self-path returns a single-element path

- GIVEN handler with graph containing `A` (no edges)
- WHEN `impact_shortest_path` is called with `{"from": "A", "to": "A"}`
- THEN result is `is_error == false` AND payload `"found"` is `true`
  AND `"path"` has length 1 AND `"total_cost"` equals `0.0` within `1e-9`

#### Scenario: Missing argument yields a clear error

- GIVEN handler with any graph
- WHEN `impact_shortest_path` is called with `{"to": "B"}` (no `from`)
- THEN `is_error == true` AND the text contains `"missing required arg"`

#### Scenario: Graph unavailable returns a clear error

- GIVEN a handler with `graph == None`
- WHEN `impact_shortest_path` is called with `{"from": "A", "to": "B"}`
- THEN `is_error == true` AND the text contains `"impact analysis unavailable"`

### Requirement 6: `impact_detect_cycles` tool

The `impact_detect_cycles` MCP tool MUST delegate to
`ImpactAnalysisService::detect_cycles`. It MUST accept no required
arguments. The response payload MUST be a JSON array of `SccDto`
objects (`{"members": [String], "size": usize}`). When `graph` is
`None`, the tool MUST return `is_error == true` with text containing
`"impact analysis unavailable"`.

#### Scenario: Returns SCCs of size >= 2

- GIVEN handler with disjoint cycles `A → B → A` and `X → Y → X`
- WHEN `impact_detect_cycles` is called with `{}`
- THEN result is `is_error == false`
- AND the payload parses to a `Vec<SccDto>` of length 2
- AND each member set equals `{"A", "B"}` and `{"X", "Y"}` (order not asserted)
- AND every `size` field equals its `members.len()`

#### Scenario: DAG returns an empty array (not an error)

- GIVEN handler with graph `A → B → C` (acyclic)
- WHEN `impact_detect_cycles` is called with `{}`
- THEN result is `is_error == false` AND payload parses to `[]`

#### Scenario: Self-loops are excluded

- GIVEN handler with a single node `A` (no edges)
- WHEN `impact_detect_cycles` is called with `{}`
- THEN result is `is_error == false` AND payload parses to `[]`
  (size-1 SCCs are filtered out — matches `CycleDetector` convention)

#### Scenario: Empty graph returns an empty array

- GIVEN handler with `CallGraph::new()` (no symbols, no edges)
- WHEN `impact_detect_cycles` is called with `{}`
- THEN result is `is_error == false` AND payload parses to `[]`

#### Scenario: Graph unavailable returns a clear error

- GIVEN a handler with `graph == None`
- WHEN `impact_detect_cycles` is called with `{}`
- THEN `is_error == true` AND the text contains `"impact analysis unavailable"`

### Requirement 7: `impact_component` tool

The `impact_component` MCP tool MUST delegate to
`ImpactAnalysisService::containing_component`. It MUST accept one
required string argument: `id`. The response payload MUST be one of:

- `null` (the `Option<Vec<SymbolId>>` serializes as JSON `null`) when
  `id` is missing from the graph.
- A JSON array of strings (the symbol ids in the undirected component)
  when `id` is present.

When `graph` is `None`, the tool MUST return `is_error == true` with
text containing `"impact analysis unavailable"`.

#### Scenario: Returns the undirected component

- GIVEN handler with disjoint components `A → B` and `C → D`
- WHEN `impact_component` is called with `{"id": "A"}`
- THEN result is `is_error == false`
- AND payload parses to `Vec<String>` containing exactly
  `["A", "B"]` (order not asserted)

#### Scenario: Missing id returns `null` (no panic)

- GIVEN handler with graph `A → B`
- WHEN `impact_component` is called with `{"id": "missing"}`
- THEN result is `is_error == false` AND payload text equals `"null"`

#### Scenario: Isolated node is its own component

- GIVEN handler with a single node `A` (no edges)
- WHEN `impact_component` is called with `{"id": "A"}`
- THEN result is `is_error == false` AND payload parses to
  `Vec<String>` of length 1 containing `"A"`

#### Scenario: Missing argument yields a clear error

- GIVEN handler with any graph
- WHEN `impact_component` is called with `{}` (no `id`)
- THEN `is_error == true` AND the text contains `"missing required arg"`

#### Scenario: Graph unavailable returns a clear error

- GIVEN a handler with `graph == None`
- WHEN `impact_component` is called with `{"id": "x"}`
- THEN `is_error == true` AND the text contains `"impact analysis unavailable"`

### Requirement 8: `ok_direct` response helper

A new private helper `fn ok_direct<T: serde::Serialize>(value: &T) -> CallToolResult`
MUST exist in `mcp.rs`. It MUST serialize `value` via
`serde_json::to_string_pretty` and wrap the result in
`CallToolResult::success(vec![Content::text(json)])`. On serialization
failure it MUST fall back to `err(...)` (same fallback as the existing
`ok<T>` helper). The existing `ok<T>` helper MUST NOT change. The new
helper exists because the 5 impact tools return raw `Serialize` payloads
(`Vec<String>`, `Vec<SccDto>`, the path/has-path/component objects)
that are NOT wrapped in `ExplorerResult<T>`.

#### Scenario: `ok_direct` pretty-prints a serializable value

- GIVEN a value `Vec<String>` equal to `["a", "b"]`
- WHEN `ok_direct(&value)` is called
- THEN the returned `CallToolResult` has `is_error == false`
- AND its first content item's text is a valid JSON array equal to
  `["a", "b"]` (after trimming whitespace)

#### Scenario: `ok_direct` accepts `Option<T>` and serializes `None` as `null`

- GIVEN `value: Option<PathResultDto> = None`
- WHEN `ok_direct(&value)` is called
- THEN the returned result is `is_error == false` AND the text equals
  `"null"` (after trimming whitespace)

#### Scenario: `ok` (existing) is byte-identical after this change

- GIVEN the pre-change signature `fn ok<T: serde::Serialize>(result: &ExplorerResult<T>) -> CallToolResult`
- WHEN the spec is implemented
- THEN `git diff` for the `ok` function body is empty
- AND every existing call site that used `ok(&service.x(...))` continues
  to compile and pass tests unchanged

### Requirement 9: Binary wiring clones `Arc<CallGraph>` before the repo consumes it

`crates/cognicode-explorer/src/bin/mcp.rs` MUST clone the
`Arc<CallGraph>` (one extra line per construction path) before
constructing `CallGraphRepository::new(graph)` so the same `Arc` can
also be passed to `ExplorerMcpHandler::with_graph(service, Some(graph.clone()))`.
Both the SQLite and the `--postgres` paths MUST be updated symmetrically.
The two graph-wiring paths (SQLite + `--postgres`) MUST each add exactly
one `Arc::clone` line — no other changes to the binary.

#### Scenario: SQLite path exposes the graph to the MCP handler

- GIVEN the SQLite branch in `bin/mcp.rs` (`open_graph(&db_path)?`)
- WHEN the binary is run against a real `.cognicode/cognicode.db`
- THEN `ExplorerMcpHandler::with_graph(service, Some(graph.clone()))`
  is constructed
- AND the handler's `list_tools` returns 13 tools
- AND a dispatched `impact_radius` call against a known symbol returns
  `is_error == false`

#### Scenario: `--postgres` path exposes the graph to the MCP handler

- GIVEN the `--postgres` branch (`open_graph_from_postgres(url).await?`)
- WHEN the binary is run with `--postgres <url>`
- THEN the same `Arc::clone` + `with_graph` wiring is applied
- AND the handler's `list_tools` returns 13 tools

#### Scenario: `git diff` for `bin/mcp.rs` is exactly two `Arc::clone` lines

- GIVEN the pre-change binary
- WHEN the spec is implemented
- THEN `git diff crates/cognicode-explorer/src/bin/mcp.rs` shows at
  most 2 added `Arc::clone(&graph)` lines and the same number of
  removed `graph` moves; the rest of the function bodies is byte-identical

### Requirement 10: No DTO additions, no core changes, no new dependencies

This change MUST NOT modify `crates/cognicode-core/`, MUST NOT modify
`crates/cognicode-explorer/Cargo.toml`, and MUST NOT add new
`[dependencies]` or `[dev-dependencies]`. The 5 new tools reuse the
existing `PathResultDto` and `SccDto` (defined in
`cognicode_core::application::dto::impact_dto`) and the existing
`CallGraphProjection` algorithms.

#### Scenario: `cognicode-core` is byte-identical

- GIVEN the pre-change `cognicode-core` crate
- WHEN the spec is implemented
- THEN `git diff crates/cognicode-core/` is empty

#### Scenario: `cognicode-explorer` `Cargo.toml` is byte-identical

- GIVEN the pre-change `Cargo.toml` for `cognicode-explorer`
- WHEN the spec is implemented
- THEN `git diff crates/cognicode-explorer/Cargo.toml` is empty

### Requirement 11: Test coverage — 19 TDD tests

The implementation MUST add exactly 19 tests across the explorer crate
distributed as follows:

- **16 unit dispatch tests** in `crates/cognicode-explorer/src/mcp.rs`
  `#[cfg(test)] mod tests` (1 per requirement scenario under R3–R7 plus
  1 for the `ok_direct` helper, but counting: 5 tools × ~3 scenarios
  each + 1 ok_direct round-trip = 16).
- **2 schema contract tests** in `src/mcp.rs` `#[cfg(test)]` (tool list
  count + 13-name set membership).
- **1 integration test** in `tests/integration.rs` (rewrite
  `mcp_tool_names_match_spec` to assert the 13-name set).

Every test MUST be behavior-first — the spec scenarios above are the
tests. No test may be marked `#[ignore]` without a documented rationale
in a code comment.

#### Scenario: All 19 tests are listed in the test map

- GIVEN the implementation
- WHEN `cargo test -p cognicode-explorer --no-run` runs
- THEN the test list contains exactly 19 new test functions
  (16 unit + 2 schema + 1 integration)
- AND the existing pre-change test count is unchanged minus any
  renames from R2 (the `mcp_tool_names_match_spec` rename is allowed)

## Acceptance Criteria

| #   | Criterion                                                                                  | Verifies         |
| --- | ------------------------------------------------------------------------------------------ | ---------------- |
| AC1 | `ExplorerMcpHandler::with_graph(service, graph)` exists; `new(service)` still compiles     | R1               |
| AC2 | `build_tool_schemas()` returns 13 entries; `TOOL_NAMES` has 13 entries                     | R2               |
| AC3 | `impact_radius` defaults `max_depth` to 5 and returns `Vec<String>`                       | R3               |
| AC4 | `impact_has_path` returns `{from, to, has_path}` shape                                     | R4               |
| AC5 | `impact_shortest_path` returns `PathResultDto` or JSON `null`                              | R5               |
| AC6 | `impact_detect_cycles` returns `Vec<SccDto>` with size-1 SCCs filtered                     | R6               |
| AC7 | `impact_component` returns `Vec<String>` or JSON `null`                                    | R7               |
| AC8 | `ok_direct` helper exists and is reused by all 5 impact arms                               | R8               |
| AC9 | `bin/mcp.rs` clones `Arc<CallGraph>` once per construction path                            | R9               |
| AC10| `git diff crates/cognicode-core/` is empty; `Cargo.toml` for `cognicode-explorer` is empty  | R10              |
| AC11| `cargo test -p cognicode-explorer` passes with 19 new tests + zero pre-change regressions | R11              |
| AC12| `cargo clippy --all-targets -p cognicode-explorer` clean                                   | R3–R11           |

## Edge Cases (exhaustive — each MUST have ≥1 test)

| ID  | Case                              | Expected behavior                                              |
| --- | --------------------------------- | -------------------------------------------------------------- |
| E1  | `graph == None`                   | All 5 tools return `is_error == true` with `"impact analysis unavailable"` |
| E2  | Missing required string arg       | `is_error == true` with `"missing required arg"` mentioning tool name |
| E3  | `max_depth == 0`                  | `impact_radius` returns `[]`                                   |
| E4  | `max_depth` omitted               | `impact_radius` defaults to `5`                                |
| E5  | `max_depth == usize::MAX`         | All reachable predecessors returned                            |
| E6  | Unknown symbol id                 | Empty list / `false` / `null` payload — never panic            |
| E7  | Empty graph                       | All 5 tools return empty/`null`/`[]` — no panic                |
| E8  | No path between two nodes         | `has_path=false`; `shortest_path=null`                         |
| E9  | Self-path `A → A`                 | `has_path=true`; `shortest_path` single-element path with cost 0.0 |
| E10 | Self-loop in `detect_cycles`      | Excluded (size-1 SCCs filtered)                                |
| E11 | DAG in `detect_cycles`            | `[]` (not an error)                                            |
| E12 | Disconnected graph component query | Returns only the queried node's component                      |
| E13 | Multiple disjoint cycles          | `detect_cycles` returns all of them as separate `SccDto`s      |
| E14 | NaN/±∞ edge confidence (input)    | `shortest_path` cost is finite and non-negative (sanitized at projection layer) |
| E15 | Malformed JSON arguments          | `is_error == true` with `"invalid args"`                       |
| E16 | Unknown tool name (regression)    | `is_error == true` with `"Unknown tool"`                       |

## Out of Scope (locked)

- Modifying `ImpactAnalysisService` (consumed read-only).
- Modifying `CallGraphProjection` (consumed read-only).
- Forward impact reach — now implemented (`forward-reach-impact`).
- Caching `CallGraphProjection` across calls (rebuild O(V+E) per call is
  accepted for <10K-node graphs).
- A separate MCP server binary for impact tools.
- HTTP or non-stdio transports.
- A `confidence_threshold` or scoring parameter on the MCP surface.
- Concurrent dispatching / multi-threading semantics on the handler.
- Generic graph algorithms not exposed by `ImpactAnalysisService`
  (e.g. minimum spanning tree, betweenness centrality).
- Enriching non-impact tools with metadata.
- Migrating from `CallGraphRepository` to a different adapter for the
  binary.
- Adding new DTOs — the 5 tools reuse `PathResultDto` and `SccDto`.
- Modifying any existing spec under `openspec/specs/**`.

## TDD Acceptance — First Failing Test (RED gate)

The implementation MUST NOT begin until the following test fails to
compile (red gate):

```rust
// In crates/cognicode-explorer/src/mcp.rs #[cfg(test)] mod tests
#[test]
fn test_handler_without_graph_returns_impact_unavailable() {
    // GIVEN: handler built via the legacy `new(service)` constructor
    //        (graph defaults to None).
    let (service, _dir) = build_test_service();
    let handler = ExplorerMcpHandler::new(service);

    // WHEN:  any of the 5 impact tools is dispatched.
    let req = call_tool_args(
        TOOL_IMPACT_RADIUS,
        serde_json::json!({"root": "src/a.rs:alpha:1", "max_depth": 1}),
    );
    let result = dispatch(&handler.service(), req).await;

    // THEN:  the tool reports an error and the message is clear.
    assert!(result.is_error);
    let text = first_text(&result);
    assert!(
        text.contains("impact analysis unavailable"),
        "expected 'impact analysis unavailable' in: {text}"
    );
}
```

This test (and the 18 sibling tests below) MUST fail to compile
before the implementation begins. The implementation is only green
when all 19 tests pass and the pre-change test suite is unaffected.

## TDD Test Map — Behavior-First Order

| # | Test name (proposed) | Verifies | Phase |
| - | -------------------- | -------- | ----- |
|  1 | `test_handler_without_graph_returns_impact_unavailable` | R1, E1 (RED gate) | red |
|  2 | `test_with_graph_some_makes_impact_arms_reachable` | R1 | red→green |
|  3 | `test_with_graph_none_matches_new_legacy` | R1 | red→green |
|  4 | `test_tool_schemas_list_thirteen_tools` | R2, AC2 | red→green |
|  5 | `test_tool_names_contains_impact_constants` | R2 | red→green |
|  6 | `test_impact_radius_returns_predecessors` | R3, E1 | red→green |
|  7 | `test_impact_radius_missing_root_arg` | R3, E2 | red→green |
|  8 | `test_impact_radius_default_max_depth_is_5` | R3, E4 | red→green |
|  9 | `test_impact_radius_zero_depth_returns_empty` | R3, E3 | red→green |
| 10 | `test_impact_radius_unknown_root_returns_empty` | R3, E6 | red→green |
| 11 | `test_impact_has_path_direct_transitive_unreachable` | R4, E8 | red→green |
| 12 | `test_impact_has_path_self_path` | R4, E9 | red→green |
| 13 | `test_impact_shortest_path_returns_cheapest` | R5 | red→green |
| 14 | `test_impact_shortest_path_unreachable_returns_null` | R5, E8 | red→green |
| 15 | `test_impact_shortest_path_self_path` | R5, E9 | red→green |
| 16 | `test_impact_detect_cycles_returns_sccs` | R6, E13 | red→green |
| 17 | `test_impact_detect_cycles_dag_returns_empty` | R6, E11 | red→green |
| 18 | `test_impact_component_returns_members` | R7, E12 | red→green |
| 19 | `test_impact_component_missing_id_returns_null` | R7, E6 | red→green |
| 20 | `test_ok_direct_serializes_pretty_json` | R8 | red→green |
| 21 | `mcp_tool_names_match_spec` (integration, rewritten for 13 tools) | R2 | red→green |

> Tests 6–20 are the 14 dispatch + helper unit tests in `src/mcp.rs`;
> tests 4–5 are the 2 schema contract tests; test 21 is the
> integration test. Total = 14 + 2 + 1 = 17, with tests 1–3 covering
> the handler-field invariant — rounding out the 19 mandated by the
> proposal (count: 3 handler + 14 dispatch + 1 helper + 2 schema + 1
> integration = 21 proposed; the spec mandates ≥ 19 — implementation
> may trim E1 helpers that duplicate cross-cutting coverage).
