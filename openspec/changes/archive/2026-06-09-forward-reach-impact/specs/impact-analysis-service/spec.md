# Delta for impact-analysis-service

> Companion to proposal `sdd/forward-reach-impact/proposal`. Service is
> a zero-state delegation wrapper. No DTO additions, no new dependencies.

## ADDED Requirements

### Requirement: `forward_radius(root, max_depth)`

`pub fn forward_radius(&self, graph: &CallGraph, root: &SymbolId, max_depth: usize) -> Vec<SymbolId>`
MUST return the **successors** of `root` reachable within `max_depth`
forward hops. MUST delegate to
`CallGraphProjection::find_forward_reach` after building the projection
via `CallGraphProjection::from_call_graph(graph)`. MUST return `vec![]`
(no panic) when `root` is missing, `max_depth == 0`, or `graph` is empty.
The `root` itself MUST NOT appear in the result. The method MUST be
`&self` and take `graph: &CallGraph` immutably.

Direction semantics: `impact_radius` is **predecessors** (reverse BFS).
`forward_radius` is **successors** (forward BFS). The two methods MUST
be sibling, symmetric counterparts on the same stateless service.

#### Scenario: Bounded successor traversal

- GIVEN graph `A → B → C`, `A → D`
- WHEN `forward_radius(A, 1)` AND `forward_radius(A, 2)`
- THEN result equals `{B, D}` for depth 1 AND `{B, C, D}` for depth 2
  (any order)

#### Scenario: Mirrors `find_forward_reach` exactly

- GIVEN the same `CallGraph` built once and consumed twice
- WHEN `ImpactAnalysisService::new().forward_radius(&g, &A, 3)` AND
  `CallGraphProjection::from_call_graph(&g).find_forward_reach(A, 3)`
  are called
- THEN both return the same set of symbol ids (order not asserted)

#### Scenario: Zero depth returns empty

- GIVEN any non-empty graph
- WHEN `forward_radius(any, 0)`
- THEN result is `vec![]`

#### Scenario: Missing root returns empty (no panic)

- GIVEN `CallGraph` that does not contain `m`
- WHEN `forward_radius(&m, 10)`
- THEN result is `vec![]` and no panic occurs

#### Scenario: Empty graph returns empty

- GIVEN `CallGraph::new()` (0 nodes)
- WHEN `forward_radius(any_id, 5)`
- THEN result is `vec![]` and no panic occurs

#### Scenario: `usize::MAX` depth sentinel

- GIVEN graph `A → B → C → D`
- WHEN `forward_radius(A, usize::MAX)`
- THEN result equals `{B, C, D}` and the call terminates

## TDD Acceptance — Behavior-First Tests

The implementation MUST add exactly 3 unit tests (mandated by proposal):

1. `test_forward_radius_mirrors_find_forward_reach` — service result
   equals projection result for the same graph/root/depth.
2. `test_forward_radius_empty_graph_returns_empty` — `CallGraph::new()`
   + `forward_radius(any, 5)` returns `vec![]`.
3. `test_forward_radius_missing_symbol_returns_empty` — graph lacks `m`
   + `forward_radius(&m, 10)` returns `vec![]` (no panic).

These tests MUST be added inside the existing
`#[cfg(test)] mod tests` in
`crates/cognicode-core/src/application/services/impact_analysis.rs`
(Requirement 10 in the main spec already mandates test coverage — this
delta extends the test list).

## Out of Scope (locked, inherited from main spec)

- No modification to `ImpactAnalyzer` (stays count-based).
- No DTO additions (reuses existing `Vec<String>` serializer at MCP).
- No MCP tool exposure (lives in `explorer-impact-tools` delta).
- No new dependencies; no new traits; no async API.
- No mutation of `CallGraphProjection` or `CallGraph`.
