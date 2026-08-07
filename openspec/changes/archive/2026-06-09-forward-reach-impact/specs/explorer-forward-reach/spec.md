# Spec: explorer-forward-reach

> New capability. Companion to proposal `sdd/forward-reach-impact/proposal`.
> This spec is a **cross-layer summary** of the forward-successor behavior
> introduced by the change. The authoritative per-layer scenarios live
> in the `callgraph-petgraph-projection`, `impact-analysis-service`, and
> `explorer-impact-tools` deltas. This spec exists to make the
> cross-cutting intent grep-able and to enforce consistent direction
> semantics across all three layers.

## Purpose

Expose "what does X affect?" — i.e. the **successors** of a symbol
within a bounded forward depth — to external agents. The
`impact_radius` capability already answers the reverse question
("what depends on X?"). `explorer-forward-reach` is its symmetric
counterpart and completes the directional pair at all three layers:
projection, service, MCP.

Direction semantics are **mandatory and consistent across layers**:

| Direction | Predecessors (reverse) | Successors (forward) |
| --------- | ---------------------- | -------------------- |
| Question  | "what depends on X?"   | "what does X affect?" |
| Method    | `find_impact_radius`   | `find_forward_reach`  |
| Service   | `impact_radius`        | `forward_radius`      |
| MCP tool  | `impact_radius`        | `impact_forward_radius` |
| BFS dir   | `Direction::Incoming`  | `Direction::Outgoing` |
| Root      | excluded from result   | excluded from result  |
| Default depth | 5 (`DEFAULT_IMPACT_RADIUS_DEPTH`) | 5 (same constant) |

## Requirements

### Requirement: Cross-layer direction symmetry

The projection method, service method, and MCP tool MUST operate on
the same direction (`Direction::Outgoing` at the petgraph level). They
MUST use the same visited-set guard pattern (a `HashSet<NodeIndex>`
per BFS invocation). They MUST return identical symbol-id sets for any
identical `(graph, root, max_depth)` triple. They MUST exclude the
root from the result.

#### Scenario: Same graph yields same result across all three layers

- GIVEN graph `A → B → C`, `A → D`
- WHEN `CallGraphProjection::from_call_graph(&g).find_forward_reach(A, 2)`
  is called
- AND `ImpactAnalysisService::new().forward_radius(&g, &A, 2)` is called
- AND the MCP tool `impact_forward_radius` is dispatched with
  `{"root": "A", "max_depth": 2}`
- THEN all three results contain the same set `{B, C, D}` (order not
  asserted) AND `A` is in none of them

#### Scenario: Default depth is 5 at the MCP layer

- GIVEN a chain of 7 nodes `a1 → a2 → a3 → a4 → a5 → a6 → a7`
- WHEN `impact_forward_radius` is dispatched with `{"root": "a1"}`
  (no `max_depth`)
- THEN the result contains exactly 5 successors (the 5 closest),
  not 6 and not all 6

### Requirement: Cycle termination is the same contract at every layer

Cycles MUST terminate at every layer via the visited-set guard. The
BFS MUST NOT revisit a node. The root MUST be excluded from the
result. This contract MUST hold identically at the projection, the
service, and the MCP layers.

#### Scenario: Cycle `A → B → C → A` terminates, root excluded

- GIVEN graph `A → B → C → A`
- WHEN `find_forward_reach(A, usize::MAX)`,
  `forward_radius(&A, usize::MAX)`, and
  `impact_forward_radius` with `max_depth: 100` are each called
- THEN all three return `{B, C}` AND `A` is absent from all three
  results AND all three calls terminate in finite time

### Requirement: Failure modes are consistent across layers

Missing root, `max_depth == 0`, and empty graph MUST return `vec![]`
/ `[]` at the projection and service layers, and an empty JSON array
at the MCP layer. They MUST NOT panic and MUST NOT return an
`is_error == true` result. The only error path at the MCP layer is
`graph == None` (handler-level invariant), which yields
`is_error == true` with text containing `"impact analysis unavailable"`.

#### Scenario: Missing root at every layer

- GIVEN a graph that does NOT contain `m`
- WHEN the projection `find_forward_reach(m, 10)`,
  the service `forward_radius(&m, 10)`, and
  the MCP `impact_forward_radius` with `{"root": "m"}` are called
- THEN all three return an empty result
- AND none panic
- AND the MCP result is `is_error == false`

#### Scenario: Graph unavailable is MCP-only

- GIVEN a handler with `graph == None` and a real graph passed to the
  projection/service directly
- WHEN `impact_forward_radius` is dispatched on the handler
- THEN `is_error == true` AND the text contains
  `"impact analysis unavailable"`
- AND the projection and service still work (they don't depend on
  the handler's `graph` field)

### Requirement: No new dependencies, no schema overhaul

This capability MUST be implemented with zero new external crates and
zero new trait definitions. The MCP layer reuses the existing
`ok_direct<T: Serialize>` helper (introduced by the parent
`explorer-impact-tools` spec) and the existing
`DEFAULT_IMPACT_RADIUS_DEPTH` constant. The service layer reuses
`CallGraphProjection::from_call_graph` and the same
`HashSet<NodeIndex>` visited-set pattern that
`find_impact_radius` uses.

#### Scenario: Cargo.toml diff is empty for both crates

- GIVEN pre-change `crates/cognicode-core/Cargo.toml` and
  `crates/cognicode-explorer/Cargo.toml`
- WHEN the spec is implemented
- THEN `git diff` for both `Cargo.toml` files is empty

## Acceptance Criteria

| #   | Criterion                                                                                  | Verifies         |
| --- | ------------------------------------------------------------------------------------------ | ---------------- |
| AC1 | `CallGraphProjection::find_forward_reach(root, max_depth)` exists and passes 7–9 unit tests | projection delta |
| AC2 | `ImpactAnalysisService::forward_radius(graph, root, max_depth)` exists and passes 3 unit tests | service delta    |
| AC3 | `TOOL_IMPACT_FORWARD_RADIUS` constant exists, dispatch arm works, 5 unit tests pass        | MCP delta         |
| AC4 | `mcp_tool_names_match_spec` integration test asserts 14 tools                              | tool count        |
| AC5 | Direction symmetry: all three layers return identical sets for the same `(graph, root, max_depth)` | cross-layer |
| AC6 | Cycle termination, root exclusion, and missing-root semantics are identical at all three layers | cross-layer |
| AC7 | `cargo test --all-targets -p cognicode-core -p cognicode-explorer` green with zero pre-change regressions | all         |
| AC8 | `cargo clippy --all-targets -p cognicode-core -p cognicode-explorer` clean                 | all              |
| AC9 | `git diff crates/cognicode-core/Cargo.toml` empty; `git diff crates/cognicode-explorer/Cargo.toml` empty | deps         |

## TDD Acceptance — First Failing Test (RED gate)

The implementation MUST NOT begin until the projection's
`test_find_forward_reach_direct_successor` fails to compile. This is
the deepest RED gate in the slice. The implementation order is:

1. **RED at projection**: `CallGraphProjection::find_forward_reach`
   does not exist → projection test fails to compile.
2. **GREEN at projection**: implement BFS; projection tests pass.
3. **RED at service**: `ImpactAnalysisService::forward_radius` does
   not exist → service test fails to compile.
4. **GREEN at service**: implement delegation; service tests pass.
5. **RED at MCP**: `TOOL_IMPACT_FORWARD_RADIUS` does not exist →
   MCP dispatch test fails to compile.
6. **GREEN at MCP**: implement constant + schema + dispatch arm;
   MCP tests pass; `mcp_tool_names_match_spec` asserts 14.

If the projection test compiles, the slice is no longer in TDD mode
and the implementation MUST stop until the projection method is
removed again.

## Out of Scope (locked)

- Backward reach (predecessors) — handled by `impact_radius`, not
  modified here.
- Multi-root forward queries (single root only).
- Confidence-weighted forward reach (no cost model on success).
- Caching the BFS result across calls (rebuild O(V+E) per call).
- New external MCP tools beyond the 6 impact tools.
- HTTP/non-stdio transports.
- Async BFS, parallel BFS, or thread-pool dispatch.
- Generic graph algorithms not requested (e.g. dominance frontiers,
  betweenness centrality, transitive closure).
- Schema migration of existing tool outputs.
- Modifying `ImpactAnalyzer` (count-based) or `CycleDetector`.
