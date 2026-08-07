# ask-router Specification (NEW)

## Purpose

The `ask-router` capability exposes a single MCP tool, `cognicode_ask`, that accepts a natural-language question and dispatches it to one or more primitive tool chains. It eliminates the need for agents to memorize the 17-tool MCP surface. This is a NEW capability — no existing spec is modified.

## Requirements

### Requirement: Tool Registration

The system MUST register a new MCP tool named `cognicode_ask` that appears in `tools/list` output. The tool schema MUST declare two parameters: a required `question: string` and an optional `context: object`. The tool MUST be added to the `TOOL_NAMES` constant and `build_tool_schemas()` without altering any of the existing 17 tools.

#### Scenario: Tool appears in list with correct schema

- GIVEN the MCP server is started
- WHEN a client calls `tools/list`
- THEN the response MUST include `cognicode_ask`
- AND the schema MUST include `question: string` (required) and `context: object` (optional)
- AND the schema MUST NOT add, remove, or rename parameters of the other 17 tools

#### Scenario: Missing question argument is rejected

- GIVEN the `cognicode_ask` tool is registered
- WHEN the tool is called with an empty args object `{}`
- THEN the dispatch MUST return a validation error
- AND the error MUST identify `question` as the missing required field

### Requirement: Pattern-Based Routing

The router MUST classify incoming questions using priority-ordered regex patterns (1 = highest). The first pattern whose regex matches the lowercased question wins. Match score (confidence) MUST be derived from regex match coverage: full match = 1.0, partial match = 0.7, keyword fallback = 0.5. Each pattern MUST map to a deterministic primitive chain. The router MUST be a pure function over `(question, context)` → `PatternId` with no side effects.

#### Scenario: Highest-priority pattern wins on overlap

- GIVEN a question containing both "path between" and "depends on"
- WHEN the router classifies the question
- THEN the chosen pattern MUST be the path-between pattern (priority 1)
- AND the match score MUST be ≥ 0.7

#### Scenario: Unmatched question returns low-confidence fallback

- GIVEN a question that matches no pattern (e.g. "tell me a joke")
- WHEN the router classifies the question
- THEN the router MUST fall back to pattern 8 (`what is|describe|explain`) with confidence 0.5
- AND the response MUST include a `suggested_follow_ups` entry of kind `"no_pattern_match"`

### Requirement: Internal Dispatch (No MCP Chaining)

The router MUST call `ExplorerService` and `CallGraph` methods directly using the same `Arc<ExplorerService>` and `Arc<CallGraph>` held by `ExplorerMcpHandler`. The router MUST NOT call the 17 MCP tools through the MCP protocol (no re-serialization, no recursive `tools/call`). The dispatch layer MUST be a thin async function that invokes service methods and merges their `serde_json::Value` outputs.

#### Scenario: Dispatch shares service instance

- GIVEN the ask router is initialized with `Arc::clone(&service)` and `Arc::clone(&graph)`
- WHEN a question is dispatched
- THEN the call to `ExplorerMcpHandler` MUST NOT traverse the MCP protocol
- AND the underlying service call MUST use the same handle

### Requirement: Result Envelope Shape

Every `cognicode_ask` response MUST be wrapped in `McpResultEnvelope<serde_json::Value>`. The payload MUST be a JSON object with exactly two top-level keys: `primary_result` (the key tool output) and `supporting` (an object of auxiliary results keyed by primitive name). `ProvenanceMetadata.source` MUST equal `Some("ask-router")` and `confidence` MUST equal the match score. The envelope MUST include at least one `FollowUp` in `suggested_follow_ups` for every successful response.

#### Scenario: Envelope has required fields

- GIVEN a question that matches pattern 1
- WHEN the router returns
- THEN the payload MUST be `{"primary_result": <...>, "supporting": {...}}`
- AND `provenance.source` MUST equal `"ask-router"`
- AND `provenance.confidence` MUST be in `[0.0, 1.0]`
- AND `suggested_follow_ups` MUST be non-empty

#### Scenario: Provenance is absent on dispatch failure

- GIVEN a question that triggers a graph-unavailable error
- WHEN the router returns the error envelope
- THEN `provenance.source` MUST still be `"ask-router"` (router-level provenance is preserved)
- AND `provenance.confidence` MUST be 0.0

### Requirement: Graph Availability Gating

Before dispatching any of the 9 graph-dependent patterns (priorities 1, 2, 3, 5, 6, 7 — see Pattern specs), the router MUST check `CallGraph` availability. When the graph is unavailable, the router MUST return a `graph_unavailable` error envelope that lists the available (non-graph) alternatives. The check MUST happen BEFORE any primitive call that would otherwise return a confusing empty-result error.

#### Scenario: Graph-dependent question with no graph

- GIVEN `Arc<CallGraph>` is `None` or empty
- WHEN a question matching pattern 1 ("path between X and Y") is asked
- THEN the router MUST return an error with `error.code = "graph_unavailable"`
- AND the error message MUST list the patterns that remain available (4, 8)
- AND the router MUST NOT call `impact_shortest_path`

#### Scenario: Non-graph question is not gated

- GIVEN the graph is unavailable
- WHEN a question matching pattern 4 ("risky/quality/smells") is asked
- THEN the router MUST dispatch normally using `spotter_search`, `get_view("quality")`, `inspect_object`
- AND no `graph_unavailable` error MUST be returned

### Requirement: Entity Extraction and Disambiguation

When the router detects an entity token in the question whose `spotter_search` results contain more than one candidate with `confidence ≥ 0.6`, the router MUST inject a `FollowUp` of kind `"entity_disambiguation"` into `suggested_follow_ups` containing the top-3 candidate symbols. The dispatch MUST proceed with the top-1 candidate and surface the others as disambiguation choices. When `spotter_search` returns zero results, the router MUST inject a `FollowUp` of kind `"no_entity_match"` and return an empty `primary_result`.

#### Scenario: Ambiguous entity surfaces disambiguation follow-up

- GIVEN a question "what does `User` call?" where `spotter_search("User")` returns 3 candidates with `confidence ≥ 0.6`
- WHEN the router dispatches
- THEN `suggested_follow_ups` MUST include a `FollowUp` of kind `"entity_disambiguation"`
- AND its `args` MUST contain exactly 3 candidate symbols
- AND the dispatch MUST use the top-1 candidate

#### Scenario: No match for entity token

- GIVEN a question "what does `NonsenseSymbol` call?" where `spotter_search` returns `[]`
- WHEN the router dispatches
- THEN `primary_result` MUST be `null`
- AND `suggested_follow_ups` MUST include a `FollowUp` of kind `"no_entity_match"`
- AND no graph tools MUST be invoked

### Requirement: Follow-Up Generation

After every successful dispatch, the router MUST emit 1-3 `FollowUp` entries. Follow-ups MUST be context-aware: a `what does X call?` response MUST include a follow-up `who calls X?` (inverse direction); a `path between X and Y` response MUST include a follow-up `what does X depend on?`. Follow-ups MUST be deterministic for a given `(pattern, primary_result)` pair.

#### Scenario: Inverse-direction follow-up after forward reach

- GIVEN a question "what does `foo()` call?" matching pattern 2
- WHEN the router returns
- THEN `suggested_follow_ups` MUST include a follow-up question `who calls foo()?`
- AND that follow-up MUST be marked with `kind = "related_inverse"`

#### Scenario: Path response includes dependency follow-up

- GIVEN a question "path between A and B" matching pattern 1
- WHEN the router returns
- THEN `suggested_follow_ups` MUST include a follow-up `what does A depend on?`
- AND a follow-up `what does B depend on?`

---

## Pattern Specifications

### Pattern 1: Path Between Two Entities

**Regex**: `connects.*→.*|path.*between|how.*depends` (priority 1, graph-dependent)
**Primitive chain**: `spotter_search(src)` → `spotter_search(dst)` → `impact_shortest_path(src, dst)` → `graph_explain(path)`
**Output shape**: `primary_result = { path: [...nodes], length: u32 }`; `supporting = { spotter_src, spotter_dst, explain: {...} }`

#### Scenario: Both entities resolve

- GIVEN a question "path between `parse` and `render`"
- AND `spotter_search("parse")` returns `[parse_fn]` and `spotter_search("render")` returns `[render_fn]`
- WHEN the router dispatches
- THEN `primary_result.path` MUST be a non-empty list of node ids
- AND `supporting.explain` MUST contain human-readable edge labels

#### Scenario: One entity does not resolve

- GIVEN a question where `spotter_search` returns `[]` for the destination
- WHEN the router dispatches
- THEN `primary_result` MUST be `null`
- AND a `no_entity_match` follow-up for the destination MUST be present

#### Scenario: No path exists in graph

- GIVEN two entities that resolve but `impact_shortest_path` returns `None`
- WHEN the router dispatches
- THEN `primary_result` MUST be `{ path: [], length: 0 }`
- AND a follow-up `try broader search radius` MUST be present

### Pattern 2: Forward Reach (What does X call?)

**Regex**: `calls →|what does.*call|forward` (priority 2, graph-dependent)
**Primitive chain**: `spotter_search(x)` → `impact_forward_radius(x, depth=2)`
**Output shape**: `primary_result = { root, edges: [...] }`

#### Scenario: Forward reach succeeds

- GIVEN a question "what does `validate()` call?" and the symbol resolves to one candidate
- WHEN the router dispatches
- THEN `primary_result.edges` MUST be a non-empty array
- AND each edge MUST have `from`, `to`, `kind` fields

#### Scenario: Leaf function with no outgoing calls

- GIVEN a question whose resolved entity has out-degree 0
- WHEN the router dispatches
- THEN `primary_result.edges` MUST be `[]`
- AND a follow-up `inspect the function body` MUST be present

### Pattern 3: Backward Reach (Who calls X?)

**Regex**: `→ calls|who calls|callers|depends on` (priority 3, graph-dependent)
**Primitive chain**: `spotter_search(x)` → `impact_radius(x, depth=2)` → `get_view("call-graph")`
**Output shape**: `primary_result = { root, edges: [...] }`; `supporting.view = "call-graph"`

#### Scenario: Backward reach surfaces callers

- GIVEN a question "who calls `format_date`?"
- WHEN the router dispatches
- THEN `primary_result.edges` MUST include incoming edges only
- AND `supporting.view` MUST equal `"call-graph"`

#### Scenario: No callers found

- GIVEN a question whose resolved entity has in-degree 0
- WHEN the router dispatches
- THEN `primary_result.edges` MUST be `[]`
- AND a follow-up `try `format_date` without namespace` MUST be present

### Pattern 4: Code Quality / Smells

**Regex**: `risky|quality|smells` (priority 4, NOT graph-dependent)
**Primitive chain**: `spotter_search(x)` → `get_view("quality")` → `inspect_object(x)`
**Output shape**: `primary_result = { smells: [...], score: f32 }`; `supporting = { view, object }`

#### Scenario: Quality view contains smells

- GIVEN a question "any smells in `parse_config`?" and the symbol resolves
- WHEN the router dispatches
- THEN `primary_result.smells` MUST be a non-empty array
- AND each smell MUST include `rule_id`, `severity`, `location`

#### Scenario: Clean code returns empty smells

- GIVEN a question whose `get_view("quality")` returns no smells for the symbol
- WHEN the router dispatches
- THEN `primary_result.smells` MUST be `[]`
- AND `primary_result.score` MUST equal `1.0`

### Pattern 5: Architecture Shape

**Regex**: `shape|architecture|cycles|structure` (priority 5, graph-dependent)
**Primitive chain**: `impact_detect_cycles()` → `graph_cluster()`
**Output shape**: `primary_result = { cycles: [...] }`; `supporting = { clusters: [...] }`

#### Scenario: Workspace contains cycles

- GIVEN a workspace with at least one cycle in the call graph
- WHEN the router dispatches a shape question
- THEN `primary_result.cycles` MUST be a non-empty array
- AND each cycle MUST be a list of node ids

#### Scenario: Acyclic workspace

- GIVEN a workspace with no cycles
- WHEN the router dispatches
- THEN `primary_result.cycles` MUST be `[]`
- AND `supporting.clusters` MUST contain ≥ 1 cluster

### Pattern 6: Workspace Overview

**Regex**: `where.*start|entry point|overview|workspace` (priority 6, graph-dependent)
**Primitive chain**: `open_workspace()` → `graph_cluster()` → `apply_lens("hotspots")`
**Output shape**: `primary_result = { hotspots: [...] }`; `supporting = { clusters, workspace_meta }`

#### Scenario: First-time overview request

- GIVEN a question "where should I start?"
- WHEN the router dispatches
- THEN `open_workspace` MUST be called before `graph_cluster`
- AND `primary_result.hotspots` MUST be sorted by `centrality` descending

#### Scenario: Lens application fails

- GIVEN `apply_lens("hotspots")` returns an error
- WHEN the router dispatches
- THEN the envelope MUST NOT fail entirely
- AND `supporting.lens_error` MUST contain the lens error message
- AND a follow-up `try lens 'complexity'` MUST be present

### Pattern 7: Component / Cluster Membership

**Regex**: `belongs|component|cluster` (priority 7, graph-dependent)
**Primitive chain**: `spotter_search(x)` → `impact_component(x)` → `inspect_object(component_id)`
**Output shape**: `primary_result = { component_id, members: [...] }`

#### Scenario: Entity belongs to a component

- GIVEN a question "what component does `db.rs` belong to?"
- WHEN the router dispatches
- THEN `primary_result.component_id` MUST be non-null
- AND `primary_result.members` MUST include the queried entity

#### Scenario: Entity belongs to no component

- GIVEN a question whose `impact_component` returns `None`
- WHEN the router dispatches
- THEN `primary_result.component_id` MUST be `null`
- AND a follow-up `run pattern 5 to see clusters` MUST be present

### Pattern 8: Generic Description (Fallback)

**Regex**: `what is|describe|explain` (priority 8, NOT graph-dependent)
**Primitive chain**: `spotter_search(x)` → `inspect_object(x)` → `get_view("overview")`
**Output shape**: `primary_result = { summary, kind, location }`; `supporting = { overview_view }`

#### Scenario: Generic description of a symbol

- GIVEN a question "what is `AuthService`?" and the symbol resolves
- WHEN the router dispatches
- THEN `primary_result.summary` MUST be a non-empty string
- AND `primary_result.kind` MUST be one of the valid symbol kinds

#### Scenario: Symbol resolves to a file, not a callable

- GIVEN a question whose `spotter_search` returns a file
- WHEN the router dispatches
- THEN `primary_result.kind` MUST equal `"file"`
- AND `inspect_object` MUST return file metadata, not a function signature

## Out of Scope

- Q5 "What changed?" — requires snapshot diff (Phase 4)
- Q8 "What justifies X?" — requires Decision graph (Phase 3)
- Embedding-based or LLM-based classification
- New DTOs in `dto.rs` — reuse `McpResultEnvelope<serde_json::Value>`
- Changes to existing 17 tool implementations or their schemas
- Persistence of conversation context across invocations
- Streaming or chunked responses

## TDD RED Gate

All scenarios above MUST have a failing test before implementation begins. The test suite MUST mock `ExplorerService` and `CallGraph` and exercise each pattern's primitive chain in isolation. The gate fails if any pattern lacks a corresponding test or if a test passes before the implementation exists.
