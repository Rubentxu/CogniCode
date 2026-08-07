# Spec: impact-analysis-service

> New capability. Companion to proposal `sdd/impact-analysis-service/proposal`.
> Consumed infrastructure: `CallGraphProjection` (read-only, no modification).

## Purpose

Application-layer service that coordinates `CallGraphProjection` algorithms
(`infrastructure::graph`) over the canonical `CallGraph` aggregate to answer
graph-aware impact queries. Pure extension — does NOT replace `ImpactAnalyzer`
(count-based) and does NOT expose MCP/UI endpoints.

Direction semantics: `impact_radius` is **predecessor-only** (reverse BFS) —
answers "what depends on X". Forward reach (`forward_reach`) is a future slice.

## Requirements

### Requirement 1: Service struct + construction

`ImpactAnalysisService` MUST live at `crates/cognicode-core/src/application/services/impact_analysis.rs`,
expose `pub fn new() -> Self`, and be re-exported via `application::services::mod`
(`pub mod impact_analysis;`). The service MUST hold zero state — every public
method takes `&CallGraph` and constructs a `CallGraphProjection` internally
via `CallGraphProjection::from_call_graph(graph)`.

#### Scenario: Default construction is stateless and idempotent
- GIVEN a fresh `ImpactAnalysisService::new()` and a `CallGraph` with 5 symbols
- WHEN `service.impact_radius(&graph, ...)` and `service.shortest_path(&graph, ...)` are called
- THEN both calls succeed AND the graph is not mutated AND constructing twice yields equivalent results

#### Scenario: Module is publicly reachable
- WHEN `use cognicode_core::application::services::impact_analysis::ImpactAnalysisService;`
- THEN the import compiles AND `ImpactAnalysisService::new()` is callable

### Requirement 2: `impact_radius(root, max_depth)`

`pub fn impact_radius(&self, graph: &CallGraph, root: &SymbolId, max_depth: usize) -> Vec<SymbolId>`
MUST return the **predecessors** of `root` reachable within `max_depth` reverse
hops (delegates to `CallGraphProjection::find_impact_radius`). MUST return
`vec![]` (no panic) when `root` is missing, `max_depth == 0`, or `graph` is
empty. The `root` itself MUST NOT appear in the result.

#### Scenario: Bounded predecessor traversal
- GIVEN graph `D → A → C`, `B → C` (edges)
- WHEN `impact_radius(C, 1)` and `impact_radius(C, 2)` are called
- THEN result equals `{A, B}` for depth 1 AND `{A, B, D}` for depth 2 (any order)

#### Scenario: Zero depth returns empty
- GIVEN any non-empty graph
- WHEN `impact_radius(any, 0)` is called
- THEN result is `vec![]`

#### Scenario: Missing root symbol returns empty (no panic)
- GIVEN projection that does not contain `m`
- WHEN `impact_radius(&m, 10)` is called
- THEN result is `vec![]` and no panic occurs

#### Scenario: Empty graph returns empty
- GIVEN `CallGraph::new()` with 0 nodes
- WHEN `impact_radius(any_id, 5)` is called
- THEN result is `vec![]`

#### Scenario: `usize::MAX` depth sentinel
- GIVEN graph `D → A → C`, `B → C`
- WHEN `impact_radius(C, usize::MAX)` is called
- THEN result equals `{A, B, D}` (all reachable predecessors)

### Requirement 3: `has_path(from, to)`

`pub fn has_path(&self, graph: &CallGraph, from: &SymbolId, to: &SymbolId) -> bool`
MUST return `true` iff a directed path exists in the graph. MUST return `false`
(no panic) when `from` or `to` is missing. A self-path `A → A` MUST return
`true` when `A` is present.

#### Scenario: Direct, transitive, no path
- GIVEN `A → B`, `A → B → C`, `A → B` only
- WHEN `has_path(A,B)`, `has_path(A,C)`, `has_path(B,A)` are called
- THEN result is `true`, `true`, `false`

#### Scenario: Missing endpoint returns false (no panic)
- GIVEN projection lacks `m`
- WHEN `has_path(known, &m)` and `has_path(&m, known)` are called
- THEN both return `false` and no panic occurs

#### Scenario: Self-path for present node
- GIVEN graph with `A` only (no edges)
- WHEN `has_path(A, A)` is called
- THEN result is `true`

### Requirement 4: `shortest_path(from, to)`

`pub fn shortest_path(&self, graph: &CallGraph, from: &SymbolId, to: &SymbolId) -> Option<PathResultDto>`
MUST delegate to `CallGraphProjection::dijkstra` with cost = `1.0 - confidence`,
returning a `PathResultDto { path: Vec<SymbolId>, total_cost: f64, found: bool }`.
MUST return `None` when either endpoint is missing or no path exists. The path
MUST start at `from` and end at `to` with no duplicates.

#### Scenario: Shortest confidence-weighted path wins
- GIVEN `A → B` (confidence 0.9), `A → C → B` (0.5, 0.5)
- WHEN `shortest_path(A, B)` is called
- THEN result is `Some(PathResultDto { path: [A, B], total_cost: 0.1, found: true })`

#### Scenario: Unreachable target returns None
- GIVEN `A → B` only
- WHEN `shortest_path(A, C)` is called
- THEN result is `None`

#### Scenario: Missing endpoint returns None (no panic)
- GIVEN projection lacks `m`
- WHEN `shortest_path(known, &m)` is called
- THEN result is `None` and no panic occurs

#### Scenario: NaN confidence produces cost 0.0 (free edge)
- GIVEN an edge with `f64::NAN` confidence
- WHEN traversing that edge during `shortest_path`
- THEN the effective cost is `0.0` and traversal terminates

### Requirement 5: `detect_cycles()`

`pub fn detect_cycles(&self, graph: &CallGraph) -> Vec<Vec<SymbolId>>`
MUST return all non-trivial strongly connected components of size ≥ 2 from
`CallGraphProjection::strongly_connected_components()`. Self-loops (size 1
SCCs) MUST be excluded. An empty graph MUST return `vec![]`.

#### Scenario: DAG returns no cycles
- GIVEN `A → B → C` (acyclic)
- WHEN `detect_cycles()` is called
- THEN result is `vec![]`

#### Scenario: Mutual cycle returned as SCC
- GIVEN `A → B → A`
- WHEN `detect_cycles()` is called
- THEN result contains exactly one SCC equal to `{A, B}` (any order within)

#### Scenario: Self-loop excluded
- GIVEN `A → A` only
- WHEN `detect_cycles()` is called
- THEN result is `vec![]` (size-1 SCCs excluded)

#### Scenario: Multiple cycles in one graph
- GIVEN `A → B → A` and `X → Y → X` (disjoint)
- WHEN `detect_cycles()` is called
- THEN result has length 2 and contains both `{A, B}` and `{X, Y}`

### Requirement 6: `containing_component(id)`

`pub fn containing_component(&self, graph: &CallGraph, id: &SymbolId) -> Option<Vec<SymbolId>>`
MUST return the undirected connected component containing `id` from
`CallGraphProjection::connected_components()`. MUST return `None` when `id`
is missing from the graph. For an isolated node, MUST return `Some(vec![id])`.

#### Scenario: Member of a component
- GIVEN `A → B`, `C → D` (no cross-edges)
- WHEN `containing_component(A)` is called
- THEN result is `Some(vec![A, B])` (order not guaranteed)

#### Scenario: Missing id returns None
- GIVEN projection lacks `m`
- WHEN `containing_component(&m)` is called
- THEN result is `None` and no panic occurs

#### Scenario: Isolated node is its own component
- GIVEN graph with `A` only and no edges
- WHEN `containing_component(A)` is called
- THEN result is `Some(vec![A])`

### Requirement 7: DTO additions

`application::dto::impact_dto` MUST be extended with:
- `PathResultDto { path: Vec<String>, total_cost: f64, found: bool }` — Serialize+Deserialize, `Debug`, `Clone`.
- `SccDto { members: Vec<String>, size: usize }` — Serialize+Deserialize, `Debug`, `Clone`.
Both MUST convert from internal `Vec<SymbolId>` to `Vec<String>` via
`symbol_id.as_str().to_string()`. Existing `ImpactDto` and `CycleDto` MUST NOT
change.

#### Scenario: PathResultDto round-trips through JSON
- GIVEN a `PathResultDto` with `path=["A","B"]`, `total_cost=0.1`, `found=true`
- WHEN serialized to JSON and deserialized back
- THEN fields are preserved (path, total_cost, found)

#### Scenario: SccDto size matches members length
- GIVEN an SCC `vec![A, B, A]`
- WHEN `SccDto::from_scc(scc)` is constructed
- THEN `size == 3` AND `members == ["A","B","A"]`

### Requirement 8: Read-only consumption of projection

`ImpactAnalysisService` MUST NOT modify `CallGraphProjection`, `CallGraph`, or
`PetGraphStore`. Every public method MUST be `&self` and accept `graph: &CallGraph`
immutably. No `&mut`, no interior mutability (`Mutex`/`RefCell`/`OnceCell`).

#### Scenario: Repeated calls are non-mutating
- GIVEN a `CallGraph` with 5 symbols and 7 edges
- WHEN 100 calls of all 5 methods are made in sequence
- THEN `graph.symbol_count()` and `graph.edge_count()` are unchanged after each call

### Requirement 9: No new dependencies

MUST NOT add crates to `crates/cognicode-core/Cargo.toml` `[dependencies]` or
`[dev-dependencies]`. Reuse `CallGraphProjection`, `CallGraph`, `SymbolId`,
and existing serde derives.

#### Scenario: Cargo.toml is byte-identical
- GIVEN pre-slice `Cargo.toml`
- WHEN the spec is implemented
- THEN `git diff crates/cognicode-core/Cargo.toml` is empty

### Requirement 10: Test coverage

`#[cfg(test)] mod tests` MUST include at least one test per public method
(requirements 2–6) plus every edge-case scenario enumerated below. All tests
MUST use in-memory `CallGraph::new()` with `add_symbol` + `add_dependency_with_provenance`.
No `#[ignore]` without documented rationale.

#### Scenario: All edge cases covered
- WHEN `cargo test -p cognicode-core` runs
- THEN every edge case (missing symbol, zero depth, disconnected graph, cycle,
no path, `usize::MAX` depth, empty graph, NaN confidence, self-loop) is
exercised by ≥1 passing test

## Acceptance Criteria

| #   | Criterion                                                                          | Verifies         |
| --- | ---------------------------------------------------------------------------------- | ---------------- |
| AC1 | All 5 methods (`impact_radius`, `has_path`, `shortest_path`, `detect_cycles`, `containing_component`) implemented and public | R2–R6            |
| AC2 | `PathResultDto` + `SccDto` added to `impact_dto.rs`; existing DTOs untouched        | R7               |
| AC3 | Module registered in `services/mod.rs`                                             | R1               |
| AC4 | `cargo test -p cognicode-core` passes with zero regressions on pre-slice tests     | R10              |
| AC5 | `cargo clippy --all-targets` clean for `cognicode-core`                            | R8, R9           |
| AC6 | `git diff` against `CallGraphProjection`, `CallGraph`, `PetGraphStore`, `ImpactAnalyzer` is empty | R8         |
| AC7 | `Cargo.toml` for `cognicode-core` is byte-identical to pre-slice                   | R9               |

## Edge Cases (exhaustive — all MUST have ≥1 test)

| ID  | Case                          | Expected behavior                                  |
| --- | ----------------------------- | -------------------------------------------------- |
| E1  | Missing symbol id             | `vec![]` / `false` / `None` — never panic          |
| E2  | Zero `max_depth`              | `impact_radius` returns `vec![]`                   |
| E3  | Disconnected graph            | Component queries return only the queried node's component |
| E4  | Cycle present                 | `detect_cycles` returns SCCs; `has_path`/`shortest_path` terminate |
| E5  | No path between two nodes     | `has_path=false`, `shortest_path=None`             |
| E6  | `max_depth == usize::MAX`     | Returns all reachable predecessors                 |
| E7  | Empty graph                   | All methods return empty/`None`/`false` — no panic |
| E8  | NaN/±∞ edge confidence        | Sanitized to `1.0`; `shortest_path` cost = `0.0`   |
| E9  | Self-loop on single node      | `detect_cycles` excludes (size-1 SCC); `has_path(A,A)=true` |
| E10 | Self-path in shortest_path    | `shortest_path(A,A) == Some(vec![A], 0.0)`         |

## Out of Scope (locked)

- No modification to `ImpactAnalyzer` (stays count-based).
- No modification to `CallGraphProjection` (consumed read-only).
- No replacement of `CycleDetector` (separate refactoring slice).
- No forward impact radius (`forward_reach` — future slice).
- No multi-root impact queries.
- No confidence-weighted impact scoring (multi-factor model).
- No MCP tool exposure / UI endpoint.
- No PostgreSQL schema changes or new repositories.
- No new traits; no new external dependencies.
- No async API.
- No mutation of the projection or the aggregate.
- No serialization of the projection itself.
