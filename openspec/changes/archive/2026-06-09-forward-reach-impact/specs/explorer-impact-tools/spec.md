# Delta for explorer-impact-tools

> Companion to proposal `sdd/forward-reach-impact/proposal`. Adds one
> new MCP tool (`impact_forward_radius`) and bumps the tool count from
> 13 to 14. Touches constants, schema list, dispatch arm, and one
> integration test.

## MODIFIED Requirements

### Requirement: Tool list contract — 14 total, `impact_*` prefix unique

`build_tool_schemas()` MUST return exactly 14 tools. Six tool
constants MUST be exposed at `crates/cognicode-explorer/src/mcp.rs`
(5 existing + 1 new):

- `TOOL_IMPACT_RADIUS: &str = "impact_radius"`
- `TOOL_IMPACT_HAS_PATH: &str = "impact_has_path"`
- `TOOL_IMPACT_SHORTEST_PATH: &str = "impact_shortest_path"`
- `TOOL_IMPACT_DETECT_CYCLES: &str = "impact_detect_cycles"`
- `TOOL_IMPACT_COMPONENT: &str = "impact_component"`
- `TOOL_IMPACT_FORWARD_RADIUS: &str = "impact_forward_radius"` (NEW)

`TOOL_NAMES` MUST list all 14 in the original order followed by the
6 impact tools last. The 6 impact tool names MUST NOT collide with the
8 existing tool names. The `mcp_tool_names_match_spec` integration
test MUST be updated to assert the 14-name set (was 13).

(Previously: 13 tools, 5 impact constants, integration test asserted 13)

#### Scenario: `tool_names()` returns 14 entries

- GIVEN a fresh `cognicode_explorer::mcp::tool_names()` call
- WHEN evaluated
- THEN `actual.len() == 14`
- AND `actual` contains every existing `TOOL_*` constant
- AND `actual` contains all 6 `TOOL_IMPACT_*` constants (5 existing
  + `TOOL_IMPACT_FORWARD_RADIUS`)

#### Scenario: Schema count matches the constant list

- GIVEN a fresh `build_tool_schemas()` call
- WHEN evaluated
- THEN the returned `Vec<Tool>` has length 14
- AND every name in the vector is also in `TOOL_NAMES`
- AND no two schemas share the same `name`
- AND `TOOL_IMPACT_FORWARD_RADIUS` is present

### Requirement: `impact_radius` tool — `DEFAULT_IMPACT_RADIUS_DEPTH` constant shared

The `impact_radius` MCP tool MUST continue to default `max_depth` to
the project-wide constant `DEFAULT_IMPACT_RADIUS_DEPTH: usize = 5`.
The new `impact_forward_radius` tool MUST default `max_depth` to the
**same** constant. The constant MUST be `pub` and live alongside
`TOOL_IMPACT_RADIUS` in `crates/cognicode-explorer/src/mcp.rs`.

(Previously: only `impact_radius` referenced the constant; `forward`
  tools reuse the same default — no new constant is introduced)

#### Scenario: `DEFAULT_IMPACT_RADIUS_DEPTH` is `5`

- GIVEN `pub const DEFAULT_IMPACT_RADIUS_DEPTH: usize`
- WHEN referenced from the dispatch arms
- THEN its value equals `5`

### Requirement: `impact_has_path` tool — unchanged

The `impact_has_path` MCP tool MUST be unchanged. Its scenarios and
edge cases from the main spec continue to apply without modification.

### Requirement: `impact_shortest_path` tool — unchanged

The `impact_shortest_path` MCP tool MUST be unchanged. Its scenarios
and edge cases from the main spec continue to apply without
modification.

### Requirement: `impact_detect_cycles` tool — unchanged

The `impact_detect_cycles` MCP tool MUST be unchanged. Its scenarios
and edge cases from the main spec continue to apply without
modification.

### Requirement: `impact_component` tool — unchanged

The `impact_component` MCP tool MUST be unchanged. Its scenarios and
edge cases from the main spec continue to apply without modification.

## ADDED Requirements

### Requirement: `impact_forward_radius` tool

The `impact_forward_radius` MCP tool MUST delegate to
`ImpactAnalysisService::forward_radius(&self, graph, root, max_depth)`.
It MUST accept two arguments:

- `root` (string, required): symbol id to analyze.
- `max_depth` (integer, optional): when omitted, the tool MUST default
  to `DEFAULT_IMPACT_RADIUS_DEPTH` (5). When omitted, the tool MUST
  behave identically to `impact_radius`'s default-depth branch (same
  constant, same semantics: finite depth window).

It MUST return the symbol ids (as strings) of all **successors** of
`root` reachable within `max_depth` forward hops. The result MUST be
serialized as a JSON array of strings via `ok_direct<T: Serialize>`.
The `root` itself MUST NOT appear in the result. When `graph` is
`None`, the tool MUST return `is_error == true` with text containing
`"impact analysis unavailable"`. The tool constant
`TOOL_IMPACT_FORWARD_RADIUS` MUST be `pub` and listed last in
`TOOL_NAMES` (after the 5 existing impact tools).

#### Scenario: Returns successors as a JSON string array

- GIVEN handler with graph `A → B → C`, `A → D`
- WHEN `impact_forward_radius` is called with
  `{"root": "A", "max_depth": 2}`
- THEN `is_error == false` AND the JSON payload parses to a
  `Vec<String>` containing exactly `["B", "C", "D"]` (order not
  asserted) AND `A` is NOT in the payload

#### Scenario: Missing `root` argument yields a clear error

- GIVEN handler with any graph
- WHEN `impact_forward_radius` is called with `{}` (no `root`)
- THEN `is_error == true` AND the text contains
  `"missing required arg"` AND the text mentions the tool name
  `"impact_forward_radius"`

#### Scenario: `max_depth` omitted defaults to 5

- GIVEN handler with graph chain
  `a1 → a2 → a3 → a4 → a5 → a6 → a7` (length 7)
- WHEN `impact_forward_radius` is called with `{"root": "a1"}` and
  no `max_depth`
- THEN `is_error == false`
- AND the returned array contains exactly 5 symbols (the 5 closest
  successors), not 6 and not all 6

#### Scenario: `max_depth == 0` returns an empty array

- GIVEN handler with graph `A → B`
- WHEN `impact_forward_radius` is called with
  `{"root": "A", "max_depth": 0}`
- THEN result is `is_error == false` AND payload parses to
  `Vec<String>` of length 0

#### Scenario: Unknown `root` returns an empty array (no panic)

- GIVEN handler with graph `A → B`
- WHEN `impact_forward_radius` is called with
  `{"root": "missing", "max_depth": 5}`
- THEN result is `is_error == false` AND payload parses to empty
  `Vec<String>`

#### Scenario: Cycle reachable, root excluded

- GIVEN handler with graph `A → B → C → A` (cycle includes root)
- WHEN `impact_forward_radius` is called with
  `{"root": "A", "max_depth": 100}` (or any large depth)
- THEN result is `is_error == false` AND payload parses to
  `Vec<String>` containing `["B", "C"]` (order not asserted) AND
  `A` is NOT in the payload AND the call terminates

#### Scenario: Disconnected successor returns empty

- GIVEN handler with graph `A → B` and an isolated `Z`
- WHEN `impact_forward_radius` is called with `{"root": "Z", "max_depth": 5}`
- THEN result is `is_error == false` AND payload parses to empty
  `Vec<String>`

#### Scenario: Empty graph returns an empty array

- GIVEN handler with `CallGraph::new()` (no symbols, no edges)
- WHEN `impact_forward_radius` is called with
  `{"root": "anything", "max_depth": 5}`
- THEN result is `is_error == false` AND payload parses to empty
  `Vec<String>`

#### Scenario: Graph unavailable returns a clear error

- GIVEN a handler built with `new(service)` (`graph == None`)
- WHEN `impact_forward_radius` is called with
  `{"root": "x", "max_depth": 1}`
- THEN `is_error == true` AND the text contains
  `"impact analysis unavailable"` AND the text mentions the tool name

### Requirement: `mcp_tool_names_match_spec` integration test asserts 14

The integration test `mcp_tool_names_match_spec` in
`crates/cognicode-explorer/tests/integration.rs` MUST be rewritten to
assert the 14-name set including `TOOL_IMPACT_FORWARD_RADIUS`. Any
companion assertions (length, membership) MUST use `14` and the full
6-impact-tool set.

(Previously: asserted 13 names, 5 impact tools)

#### Scenario: integration test asserts 14 tools

- GIVEN the updated integration test
- WHEN `cargo test -p cognicode-explorer --test integration` runs
- THEN `mcp_tool_names_match_spec` passes
- AND the assertion length equals `14`
- AND the assertion set contains `TOOL_IMPACT_FORWARD_RADIUS`

## TDD Acceptance — First Failing Test (RED gate)

The implementation MUST NOT begin until the following test fails to
compile:

```rust
// In crates/cognicode-explorer/src/mcp.rs #[cfg(test)] mod tests
#[test]
fn test_impact_forward_radius_direct_successor() {
    // GIVEN: handler with graph A → B
    let (handler, _dir) = build_test_handler_with_graph();
    add_edge(&handler, "A", "B");

    // WHEN:  impact_forward_radius dispatched with root=A, depth=1
    let req = call_tool_args(
        TOOL_IMPACT_FORWARD_RADIUS,
        serde_json::json!({"root": "A", "max_depth": 1}),
    );
    let result = dispatch(&handler, req).await;

    // THEN:  result is success and contains exactly ["B"]
    assert!(!result.is_error);
    let parsed: Vec<String> = serde_json::from_str(&first_text(&result))?;
    assert_eq!(parsed, vec!["B".to_string()]);
}
```

This test MUST fail to compile (`TOOL_IMPACT_FORWARD_RADIUS` does not
exist) before the implementation begins. The implementation is green
only when the RED test and the 4 sibling dispatch tests pass, AND the
integration test `mcp_tool_names_match_spec` asserts 14 tools.

## TDD Test Map — Behavior-First Order

| # | Test name | Verifies | Phase |
| - | --------- | -------- | ----- |
| 1 | `test_impact_forward_radius_direct_successor` | R-direct, RED gate | red |
| 2 | `test_impact_forward_radius_transitive_successor` | R-transitive | red→green |
| 3 | `test_impact_forward_radius_max_depth_zero_returns_empty` | R-zero | red→green |
| 4 | `test_impact_forward_radius_unknown_root_returns_empty` | R-missing | red→green |
| 5 | `test_impact_forward_radius_cycle_terminates_root_excluded` | R-cycle | red→green |
| 6 | `test_impact_forward_radius_disconnected_returns_empty` | R-disconnected | red→green |
| 7 | `test_impact_forward_radius_empty_graph` | R-empty | red→green |
| 8 | `test_impact_forward_radius_default_max_depth_is_5` | R-default-5 | red→green |
| 9 | `test_impact_forward_radius_missing_root_arg` | R-missing-arg | red→green |
| 10 | `test_impact_forward_radius_graph_unavailable` | R-graph-none | red→green |
| 11 | `test_tool_names_contains_impact_forward_radius` | R14-set | red→green |
| 12 | `test_tool_schemas_list_fourteen_tools` | R14-count | red→green |
| 13 | `mcp_tool_names_match_spec` (integration, 13→14) | R14-integration | red→green |

> Tests 1–10 are the 5 dispatch unit tests mandated by the proposal
> (count: 5 explicit scenarios above; the cycle and disconnected
> tests are added for parity with the projection scenarios). Tests
> 11–12 are the 2 schema contract tests. Test 13 is the integration
> test. Total = 10 dispatch + 2 schema + 1 integration = 13 new tests.

## Edge Cases (exhaustive — each MUST have ≥1 test)

| ID  | Case                              | Expected behavior                                              |
| --- | --------------------------------- | -------------------------------------------------------------- |
| F1  | `graph == None`                   | `is_error == true` with `"impact analysis unavailable"`        |
| F2  | Missing required string arg       | `is_error == true` with `"missing required arg"` mentioning tool name |
| F3  | `max_depth == 0`                  | Returns `[]`                                                   |
| F4  | `max_depth` omitted               | Defaults to `5` (same as `impact_radius`)                      |
| F5  | `max_depth == usize::MAX`         | All reachable successors returned                              |
| F6  | Unknown symbol id                 | `[]` — never panic                                             |
| F7  | Empty graph                       | `[]` — no panic                                                |
| F8  | Cycle including root              | Visit-set terminates; root excluded from result                |
| F9  | Disconnected successor (no edges out of root) | `[]` — no global scan, no panic                    |
| F10 | Tool count regression             | `mcp_tool_names_match_spec` asserts 14 tools with new constant  |

## Out of Scope (locked, inherited from main spec)

- No modification to `ImpactAnalysisService` (consumed read-only).
- No modification to `CallGraphProjection` (consumed read-only).
- No new DTOs (reuses the `ok_direct<Vec<String>>` serializer).
- No `Cargo.toml` changes (no new dependencies).
- No new tool surface beyond the 6 impact tools.
- No HTTP or non-stdio transports.
- No concurrent dispatching semantics on the handler.
- No modification to non-impact tools' metadata.
- No changes to the 8 pre-existing tool signatures.
