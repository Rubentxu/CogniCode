# mcp-tools-envelope Specification

## Purpose

Specify how each of the 17 MCP tools in `cognicode-explorer/src/mcp.rs` adopts
the `McpResultEnvelope<T>` wrapper. The wrapper is uniform; this spec lists
the per-tool wire shape (tool name, payload type, raw-vs-ExplorerResult).

## Requirements

### Requirement: Explorer tools wrap in envelope

The eight explorer tools MUST emit a successful response via `envelope_ok`:

1. `explorer_open_workspace` — payload: `WorkspaceSummary` (ExplorerResult)
2. `explorer_spotter_search` — payload: `Vec<SpotterResult>` (ExplorerResult)
3. `explorer_inspect_object` — payload: `InspectableObjectSummary` (ExplorerResult)
4. `explorer_get_views` — payload: `Vec<ViewDescriptor>` (ExplorerResult)
5. `explorer_get_view` — payload: `ContextualView` (ExplorerResult)
6. `explorer_get_lenses` — payload: `Vec<LensDescriptor>` (ExplorerResult)
7. `explorer_apply_lens` — payload: `LensResult` (ExplorerResult)
8. `explorer_query_moldql` — payload: `MoldQLResultDto` (ExplorerResult)

For each, the `tool_name` in the envelope MUST equal the tool's `TOOL_*`
constant.

#### Scenario: explorer_open_workspace emits envelope with correct tool_name

- GIVEN the handler is bound to a valid workspace
- WHEN `tools/call` is invoked with `name: "explorer_open_workspace"` and valid args
- THEN the success `Content::text` JSON has `tool_name: "explorer_open_workspace"`
- AND `payload` is a `WorkspaceSummary`
- AND `version`, `timestamp`, `provenance`, `suggested_follow_ups` are present

#### Scenario: explorer_spotter_search preserves payload type

- GIVEN the search service returns a non-empty list
- WHEN the tool is dispatched
- THEN the envelope's `payload` is the array of search hits
- AND no fields of the hits are silently dropped or renamed

### Requirement: Impact tools wrap in envelope via envelope_ok_direct

The six impact tools MUST emit a successful response via `envelope_ok_direct`:

1. `impact_radius` — payload: `Vec<String>` (symbol ids)
2. `impact_forward_radius` — payload: `Vec<String>`
3. `impact_has_path` — payload: `bool`
4. `impact_shortest_path` — payload: `Vec<String>`
5. `impact_detect_cycles` — payload: `Vec<Vec<String>>`
6. `impact_component` — payload: `Vec<String>`

#### Scenario: impact_radius wraps Vec<String> payload

- GIVEN the call graph is loaded and `root` is a valid symbol id
- WHEN `tools/call` is invoked with `name: "impact_radius"`
- THEN the success `Content::text` JSON has `tool_name: "impact_radius"`
- AND `payload` is a JSON array of strings
- AND the envelope fields are present and non-null/non-absent

#### Scenario: impact_has_path wraps bool payload

- GIVEN the call graph is loaded and `from`/`to` resolve to nodes
- WHEN the tool is invoked and the service returns `true`
- THEN `payload` in the envelope is the JSON boolean `true` (not `"true"`, not `1`)

### Requirement: Graph tools wrap in envelope

The three graph tools MUST emit a successful response via `envelope_ok_direct`:

1. `graph_subgraph` — payload: `SubgraphDto`
2. `graph_cluster` — payload: `ClusterDto`
3. `graph_explain` — payload: `ExplainDto`

#### Scenario: graph_subgraph emits envelope

- GIVEN the call graph is loaded and `root` is valid
- WHEN `tools/call` is invoked with `name: "graph_subgraph"`
- THEN the success result's JSON has `tool_name: "graph_subgraph"`
- AND `payload` is a `SubgraphDto` object (recognizable fields: `nodes`, `edges`)
- AND envelope fields are present

#### Scenario: graph_explain emits envelope

- GIVEN the call graph is loaded and `from`/`to` resolve
- WHEN the tool is invoked
- THEN the envelope's `tool_name` is `"graph_explain"`
- AND `payload` is the explain result object

### Requirement: Provenance populated where natively available

When a tool's underlying service result carries confidence or provenance
information, the dispatch arm MUST pass it as the `provenance` argument. For
tools without native confidence, `provenance` MUST be `None`.

#### Scenario: Tools without native confidence use None

- GIVEN `explorer_spotter_search` returns a list with no per-hit confidence
- WHEN the dispatch arm calls `envelope_ok(TOOL_SPOTTER_SEARCH, &result, None)`
- THEN the envelope's `provenance` is JSON `null`

#### Scenario: Tools with confidence forward it

- GIVEN a tool's service returns aggregate confidence `0.87`
- WHEN the dispatch arm computes `Some(ProvenanceMetadata { confidence: Some(0.87), source: Some("call_graph".into()) })`
- THEN that metadata appears under the `provenance` key in the envelope

### Requirement: All 17 tools reachable by name

The `TOOL_NAMES` constant MUST list all 17 tools, and `tool_names()` MUST
return that slice. A consumer iterating `tool_names()` MUST be able to
dispatch each one through the handler without hitting the `Unknown tool`
error path.

#### Scenario: TOOL_NAMES length is 17

- GIVEN `TOOL_NAMES` is the canonical list
- WHEN `TOOL_NAMES.len()` is asserted in a test
- THEN it equals 17

#### Scenario: Every TOOL_NAMES entry dispatches successfully

- GIVEN the handler is constructed with a valid workspace
- WHEN each of the 17 tool names is dispatched (with minimal valid args)
- THEN none of them returns the `Unknown tool: {name}` error

## TDD RED Gate

For each of the 17 tools, a dispatch test MUST exist that:
1. Calls the tool with valid args
2. Asserts the response is a success result
3. Deserializes the success text as `serde_json::Value`
4. Asserts `.tool_name == TOOL_*` constant
5. Asserts `.payload` is non-null and matches the expected shape

For the six impact tools, the test MUST also assert `payload` is the raw
value, not an `ExplorerResult`-shaped object.

## Edge Cases

| Edge | Expected Behavior |
|------|-------------------|
| Tool called with a missing required arg | Existing per-tool error via `err(...)`; envelope NOT emitted |
| Tool called when service returns `Err` | `err(e.to_string())`; envelope NOT emitted |
| Tool called with empty payload (e.g., `impact_radius` finds nothing) | `payload: []` inside envelope |
| `tool_name` in JSON differs from `TOOL_*` constant | Bug in dispatch arm; spec requires equality |

## Out of Scope

- Tool signature or arg struct changes
- DTO field additions or renames
- Service-layer behavior changes
- New tools beyond the existing 17
- Per-tool `provenance` sources beyond what the service already returns
- Auto-detection of confidence by tool name
