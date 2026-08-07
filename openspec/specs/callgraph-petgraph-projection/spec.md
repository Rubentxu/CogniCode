# Spec: callgraph-petgraph-projection

> Companion to engram `sdd/petgraph-postgres-projection/spec` and proposal `sdd/petgraph-postgres-projection/proposal`.

## Purpose

Read-side algorithmic layer over the canonical `CallGraph` aggregate. `CallGraphProjection` wraps `StableGraph<SymbolId, (DependencyType, f64)>` + `HashMap<SymbolId, Symbol>` side-lookup. Exposes SCC, cycle detection, topological sort, Dijkstra (cost = `1.0 - confidence`), connected components (undirected), `has_path`, and impact-radius (reverse BFS bounded by `max_depth`). Read-only; no mutation of aggregate or `PetGraphStore`.

## Requirements

### Requirement: Struct + constructor

`pub struct CallGraphProjection` MUST hold `StableGraph<SymbolId, (DependencyType, f64)>` and `HashMap<SymbolId, Symbol>`. `pub fn from_call_graph(graph: &CallGraph) -> Self` consumes the aggregate's public iteration API immutably; result's `node_count() == graph.symbol_count()` and `edge_count() == graph.edge_count()`. Re-exported from `infrastructure::graph`.

#### Scenario: Faithful, non-mutating, deterministic
- GIVEN 7 symbols, 12 edges; two projections `p1`, `p2` of the same `g`
- WHEN `from_call_graph(&g)` twice AND re-reading `g.symbol_count()`/`g.edge_count()`
- THEN `p1`/`p2` both have `node_count()==7` AND `edge_count()==12` AND every edge appears once AND every `SymbolId` is keyed AND `g` counts are unchanged AND iteration order is identical

### Requirement: `f64` confidence sanitation

On construction: non-finite values (NaN, +∞, −∞) → `1.0`; clamp to `[0.0, 1.0]`; preserve in-range bit-exact. No panic on any `f64`.

#### Scenario: Sanitization table
- GIVEN confidences `{NaN, +∞, −∞, 1.5, -0.2, 2.0, 0.0, 0.5, 1.0}`
- WHEN built
- THEN stored are `{1.0, 1.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.5, 1.0}`

### Requirement: `topological_sort`

`pub fn topological_sort(&self) -> Result<Vec<SymbolId>, ProjectionError>` — `Ok(order)` (length = `node_count()`) on DAG, `Err(ProjectionError::CycleDetected)` on any cycle, `Ok(vec![])` on empty.

#### Scenario: DAG; cycle; empty
- GIVEN `A → B → C`; `A → B → A`; 0 nodes
- WHEN called three times
- THEN `Ok([A,B,C])`, `Err(CycleDetected)` (no panic), `Ok(vec![])`

### Requirement: `strongly_connected_components`

`pub fn strongly_connected_components(&self) -> Vec<Vec<SymbolId>>` — partition of nodes; each node in exactly one SCC; self-loops are singletons; DAG returns `node_count()` singletons.

#### Scenario: DAG; mutual cycle; self-loop
- GIVEN 5-node DAG; `A → B → A`; `A → A` only
- WHEN called three times
- THEN length 5 all singletons; one SCC equals `{A,B}`; SCC for `A` is size 1

### Requirement: `detect_cycles`

`pub fn detect_cycles(&self) -> bool` — `true` iff graph has a cycle or self-loop. Consistent with SCC.

#### Scenario: Acyclic; cycle; empty
- GIVEN `A → B → C`; `A → B → A`; 0 nodes
- WHEN called three times
- THEN `false`, `true`, `false`

### Requirement: `connected_components` (undirected)

`pub fn connected_components(&self) -> Vec<Vec<SymbolId>>` — partition under undirected view; isolated nodes are singletons.

#### Scenario: Two subgraphs; all isolated
- GIVEN `A → B`, `C → D` (no cross); 4 nodes 0 edges
- WHEN called twice
- THEN length 2: `{A,B}` and `{C,D}`; length 4 all singletons

### Requirement: `has_path`

`pub fn has_path(&self, from: SymbolId, to: SymbolId) -> bool` — `true` iff directed path exists. `false` (no panic) on missing id.

#### Scenario: Direct, transitive, no path, missing
- GIVEN `A → B`; `A → B → C`; `A → B` only; projection lacks `m`
- WHEN `has_path(A,B)`, `has_path(A,C)`, `has_path(B,A)`, `has_path(m, any)`
- THEN `true`, `true`, `false`, `false` (no panic)

### Requirement: `dijkstra`

`pub fn dijkstra(&self, from: SymbolId, to: SymbolId) -> Option<(Vec<SymbolId>, f64)>` — edge cost = `1.0 - confidence`; path starts at `from` ends at `to`, no duplicates; `total_cost` = sum. `None` on missing endpoint or unreachable.

#### Scenario: Shortest wins; unreachable; missing; sanitized feeds cost
- GIVEN `A → B` (conf 0.9), `A → C → B` (0.5, 0.5); `A → B` only; projection lacks `m`; edge conf NaN
- WHEN `dijkstra(A,B)`, `dijkstra(A,C)`, `dijkstra(known,m)`, traverse NaN edge
- THEN `Some(([A,B], 0.1))`, `None`, `None` (no panic), effective cost `0.0`, terminates

### Requirement: `find_impact_radius`

`pub fn find_impact_radius(&self, root: SymbolId, max_depth: usize) -> Vec<SymbolId>` — predecessors of `root` within `max_depth` reverse hops. `vec![]` (no panic) on missing root or empty graph.

#### Scenario: Bounded traversal; missing root; empty
- GIVEN `D → A → C`, `B → C`; projection lacks `m`; 0 nodes
- WHEN `find_impact_radius(C,1)`, `find_impact_radius(C,2)`, `find_impact_radius(m,10)`, `find_impact_radius(any,5)`
- THEN `{A,B}`, `{A,B,D}`, `vec![]`, `vec![]`

### Requirement: `find_forward_reach`

`pub fn find_forward_reach(&self, root: SymbolId, max_depth: usize) -> Vec<SymbolId>`
MUST return the **successors** of `root` reachable within `max_depth` forward
hops, traversing edges via `petgraph::Direction::Outgoing`. MUST exclude
`root` from the result. MUST use a `HashSet<NodeIndex>` visited-set to
guarantee termination on cycles. MUST return `vec![]` (no panic) when
`root` is missing from the projection, `max_depth == 0`, or the projection
is empty. The BFS is the symmetric counterpart of `find_impact_radius`
(mirroring it on `Direction::Outgoing`).

Direction semantics: `find_impact_radius` answers "what depends on X?"
(predecessors). `find_forward_reach` answers "what does X affect?"
(successors). Both MUST live on `CallGraphProjection` as sibling
read-only methods.

#### Scenario: Direct successor within depth 1
- GIVEN graph `A → B`
- WHEN `find_forward_reach(A, 1)`
- THEN result equals `{B}` (any order)

#### Scenario: Transitive successor within depth 2
- GIVEN graph `A → B → C`, `A → D`
- WHEN `find_forward_reach(A, 1)` AND `find_forward_reach(A, 2)`
- THEN result equals `{B, D}` for depth 1 AND `{B, C, D}` for depth 2

#### Scenario: `max_depth == 0` returns empty
- GIVEN any non-empty graph `A → B`
- WHEN `find_forward_reach(A, 0)`
- THEN result is `vec![]`

#### Scenario: Missing root returns empty (no panic)
- GIVEN projection that does not contain `m`
- WHEN `find_forward_reach(m, 10)`
- THEN result is `vec![]` and no panic occurs

#### Scenario: Cycle visited-set prevents infinite loop, root excluded
- GIVEN graph `A → B → C → A` (cycle includes root `A`)
- WHEN `find_forward_reach(A, usize::MAX)`
- THEN result equals `{B, C}` (order not asserted) AND no panic AND
  the BFS terminates in finite time AND `A` MUST NOT appear in the result

#### Scenario: Disconnected successor returns empty
- GIVEN graph `A → B` and a separate isolated node `Z`
- WHEN `find_forward_reach(Z, 5)`
- THEN result is `vec![]` (no panic, no global scan)

#### Scenario: Empty projection returns empty
- GIVEN projection built from `CallGraph::new()` (0 nodes)
- WHEN `find_forward_reach(any_id, 5)`
- THEN result is `vec![]` and no panic occurs

#### Scenario: `usize::MAX` depth sentinel returns all reachable successors
- GIVEN graph `A → B → C → D`
- WHEN `find_forward_reach(A, usize::MAX)`
- THEN result equals `{B, C, D}` and the BFS terminates

### Requirement: `resolve_symbol`

`pub fn resolve_symbol(&self, id: SymbolId) -> Option<&Symbol>` — side-lookup; `None` for unknown id; populated for every node id.

#### Scenario: Known resolves; unknown returns None
- GIVEN `s : Symbol` at `id`; projection lacks `m`
- WHEN `resolve_symbol(id)`, `resolve_symbol(m)`
- THEN `Some(&s)`, `None`

### Requirement: No mutation / no new deps

MUST NOT modify `CallGraph`, `PetGraphStore`, or any existing trait. MUST NOT add dependencies to `cognicode-core`. All public surface in `call_graph_projection` + re-export.

#### Scenario: Default build clean; Cargo.toml unchanged
- GIVEN pre-slice source and `Cargo.toml`
- WHEN `cargo test -p cognicode-core` runs AND slice lands
- THEN every pre-slice test passes AND `git diff` against `PetGraphStore`/`CallGraph` is empty AND `cognicode-core` `[dependencies]`/`[dev-dependencies]` are byte-identical

### Requirement: Re-export

`infrastructure::graph` MUST `pub use call_graph_projection::CallGraphProjection;`.

#### Scenario: Public path resolves
- WHEN `use cognicode_core::infrastructure::graph::CallGraphProjection;`
- THEN compiles AND `from_call_graph(&g)` is reachable

### Requirement: Test coverage

`#[cfg(test)] mod tests` MUST include ≥1 test per algorithm requirement plus every edge-case scenario. No `#[ignore]` without documented rationale.

#### Scenario: All scenarios covered
- WHEN `cargo test -p cognicode-core` runs
- THEN every requirement scenario above is exercised by ≥1 passing test

### Requirement: `extract_subgraph`

`pub fn extract_subgraph(&self, root: SymbolId, direction: SubgraphDirection, max_depth: usize) -> SubgraphView`
MUST return a neighborhood of `root` within `max_depth` hops in the
selected direction(s). `SubgraphView { nodes: Vec<SymbolId>, edges:
Vec<SubgraphEdge> }` is the return DTO. `SubgraphDirection` MUST be
`pub enum { Incoming, Outgoing, Both }`. `SubgraphEdge { from:
SymbolId, to: SymbolId, dependency_type: DependencyType, confidence:
f64 }` is the edge DTO.

Semantics:
- `Outgoing` — BFS via `petgraph::Direction::Outgoing`; root's forward
  reach within `max_depth` hops.
- `Incoming` — BFS via `petgraph::Direction::Incoming`; root's
  predecessors within `max_depth` hops.
- `Both` — union of `Incoming` and `Outgoing` reachable sets; edges
  are the union of edges observed in either pass.
- The `root` MUST be included in `nodes` even if it has no edges in the
  selected direction(s).
- `nodes` MUST be deduplicated (no duplicate `SymbolId`).
- `edges` MUST be deduplicated (no duplicate `(from, to, dependency_type)`).
- `max_depth == 0` MUST return `SubgraphView { nodes: vec![root], edges: vec![] }`.
- `root` not in projection MUST return `SubgraphView { nodes: vec![root], edges: vec![] }` (no panic).
- `usize::MAX` MUST be treated as "unbounded".
- A visited-set MUST prevent infinite loops on cycles.
- MUST be a `&self` method; MUST NOT mutate the projection.
- Confidence values are already sanitized at construction (no further
  sanitization here).

#### Scenario: Outgoing BFS at depth 2 reaches the 3-node chain
- GIVEN graph `A → B → C`, `A → D`
- WHEN `extract_subgraph(A, Outgoing, 2)`
- THEN `nodes` equals `{A, B, C, D}` (any order) AND `edges` equals `{(A,B), (B,C), (A,D)}` with the corresponding `(DependencyType, conf)`

#### Scenario: Incoming BFS at depth 2 finds predecessors
- GIVEN graph `D → A → C`, `B → C`
- WHEN `extract_subgraph(C, Incoming, 2)`
- THEN `nodes` equals `{A, B, C, D}` AND `edges` equals `{(D,A), (A,C), (B,C)}`

#### Scenario: `Both` is the union of incoming + outgoing
- GIVEN graph `D → A → C`, `B → C`
- WHEN `extract_subgraph(A, Both, 2)`
- THEN `nodes` equals `{A, B, C, D}` AND `edges` equals `{(D,A), (A,C), (B,C)}`

#### Scenario: `max_depth == 0` returns just the root
- GIVEN graph `A → B`
- WHEN `extract_subgraph(A, Outgoing, 0)`
- THEN `nodes == vec![A]` AND `edges == vec![]`

#### Scenario: Missing root returns a single-node view (no panic)
- GIVEN projection that does not contain `m`
- WHEN `extract_subgraph(m, Outgoing, 5)`
- THEN `nodes == vec![m]` AND `edges == vec![]` AND no panic

#### Scenario: Cycle reachable, no duplicate edges or nodes
- GIVEN graph `A → B → C → A`
- WHEN `extract_subgraph(A, Both, usize::MAX)`
- THEN `nodes.len() == 3` AND no duplicate symbol ids AND no duplicate edges AND BFS terminates

#### Scenario: Dense graph termination and dedup
- GIVEN graph where `A` has 100 outgoing edges to 100 distinct nodes and each of those nodes points back to `A`
- WHEN `extract_subgraph(A, Both, 2)`
- THEN `nodes.len() == 101` (A + 100 successors) AND each `SymbolId` appears exactly once AND each edge appears exactly once

#### Scenario: Empty projection returns single-node view
- GIVEN projection built from `CallGraph::new()` (0 nodes)
- WHEN `extract_subgraph(any_id, Outgoing, 5)`
- THEN `nodes == vec![any_id]` AND `edges == vec![]` AND no panic

### Requirement: `explain_path`

`pub fn explain_path(&self, from: SymbolId, to: SymbolId) -> Option<ExplanationView>`
MUST run `dijkstra(from, to)` and, on success, walk adjacent pairs
`(path[i], path[i+1])` to look up the edge metadata
`(DependencyType, confidence)` for each hop. The result DTO
`ExplanationView { hops: Vec<ExplanationHop>, total_cost: f64 }` is
returned. `ExplanationHop { from: SymbolId, to: SymbolId,
dependency_type: DependencyType, confidence: f64 }` is the per-hop DTO.

Semantics:
- Cost function is identical to `dijkstra` (edge cost = `1.0 - conf`).
- `hops.len() == path.len() - 1` (one entry per directed edge traversed).
- For a self-path `from == to`, MUST return
  `Some(ExplanationView { hops: vec![ExplanationHop { from, to, dependency_type: Calls, confidence: 1.0 }], total_cost: 0.0 })`.
- Missing endpoint (`from` or `to` not in projection) MUST return `None` (no panic).
- Unreachable pair MUST return `None` (no panic).
- MUST be a `&self` method; MUST NOT mutate the projection.
- Each hop's `confidence` reflects the edge that was actually traversed by `dijkstra`.

#### Scenario: Two-hop path is explained with per-hop metadata
- GIVEN graph `A → B → C` with confidence 1.0 on both edges
- WHEN `explain_path(A, C)`
- THEN result is `Some(ExplanationView { hops: vec![{A,B,Calls,1.0}, {B,C,Calls,1.0}], total_cost: 0.0 })`

#### Scenario: Shortest confidence-weighted path wins
- GIVEN `A → B` (conf 0.9), `A → C → B` (0.5, 0.5)
- WHEN `explain_path(A, B)`
- THEN `hops == [{A,B,Calls,0.9}]` AND `total_cost == 0.1` (direct edge beats the longer 0.5/0.5 chain with cost 1.0)

#### Scenario: Unreachable pair returns `None`
- GIVEN graph `A → B` only
- WHEN `explain_path(A, Z)`
- THEN result is `None` AND no panic

#### Scenario: Missing endpoint returns `None`
- GIVEN projection that lacks `m`
- WHEN `explain_path(known, m)` and `explain_path(m, known)`
- THEN both return `None` AND no panic

#### Scenario: Self-path returns one self-hop
- GIVEN graph containing `A` (any edges or none)
- WHEN `explain_path(A, A)`
- THEN result is `Some(ExplanationView { hops: vec![{A,A,Calls,1.0}], total_cost: 0.0 })`

#### Scenario: NaN confidence edge has cost 0.0 and confidence 1.0
- GIVEN an edge with `f64::NAN` confidence that participates in the winning shortest path
- WHEN `explain_path` traverses it
- THEN `hops[i].confidence == 1.0` (sanitized at construction) AND the contribution to `total_cost` is `0.0`

#### Scenario: Empty projection returns `None`
- GIVEN projection built from `CallGraph::new()` (0 nodes)
- WHEN `explain_path(any, any)`
- THEN result is `None` AND no panic

### Requirement: New DTO types `SubgraphDirection`, `SubgraphView`, `SubgraphEdge`, `ExplanationView`, `ExplanationHop`

All MUST be `pub` and re-exported from `infrastructure::graph`. All
MUST be `Clone, Debug, PartialEq, Eq` where the underlying types
allow. Specifically:
- `SubgraphDirection` MUST derive `Copy, Clone, Debug, PartialEq, Eq, Hash`.
- `SubgraphView` MUST be `Clone, Debug, PartialEq, Eq` (SymbolId is hashable/equatable).
- `SubgraphEdge` MUST be `Clone, Debug`.
- `ExplanationView` and `ExplanationHop` MUST be `Clone, Debug`.

These DTOs MUST be distinct from the application-layer
`SubgraphResultDto` and `ExplainResultDto` (which add `String`
serialization and `summary`/`rationale` strings). The projection
layer stays generic over `SymbolId` and `DependencyType`; the
application layer converts to `String` and adds narration.

#### Scenario: `SubgraphDirection` is constructible from the public enum
- GIVEN `let d = SubgraphDirection::Both;`
- WHEN `d == SubgraphDirection::Both` is evaluated
- THEN the comparison returns `true`

#### Scenario: DTOs are re-exported
- WHEN `use cognicode_core::infrastructure::graph::{SubgraphView, ExplanationView};`
- THEN the import compiles AND both types are reachable

### Requirement: No mutation / no new deps

The new methods MUST NOT modify `CallGraph`, `PetGraphStore`, the
projection's internal `StableGraph`, or any existing trait. They MUST
NOT add dependencies to `cognicode-core`.

#### Scenario: Existing tests still pass
- GIVEN pre-slice `CallGraphProjection` source and tests
- WHEN `cargo test -p cognicode-core` runs AND the new methods land
- THEN every pre-slice test passes AND no existing method signature changes AND `cognicode-core` `[dependencies]`/`[dev-dependencies]` are byte-identical

### Requirement: Test coverage for new methods

`#[cfg(test)] mod tests` MUST include ≥1 test per new method plus
every edge-case scenario enumerated in `extract_subgraph` and
`explain_path`. No `#[ignore]` without documented rationale.

#### Scenario: All new scenarios covered
- WHEN `cargo test -p cognicode-core` runs
- THEN every scenario above (incoming/outgoing/both, max_depth=0, usize::MAX, missing root, cycle, dense, self-path, NaN) is exercised by ≥1 passing test

## Status

Draft. Awaiting `sdd-design`.

## Coverage

- **Happy paths**: construction, topological on DAG, SCC on DAG, Dijkstra, impact radius, side-lookup.
- **Edge cases**: empty, single-node, self-loop, multi-node cycle, NaN/±inf, clamp at 0.0/1.0, unknown `SymbolId`, disconnected components, no path, sanitize-then-traverse.
- **Error states**: `ProjectionError::CycleDetected`; `None` on missing/unreachable.

## Out of Scope (locked)

Explorer UI; MCP tools/envelope; PG schema; replacing `PetGraphStore`; new traits; modifying `CallGraph`; new external deps; mutation API on projection; async API; projection serialization; `ltree`/`pgvector`; new symbol kinds (`Component`/`Container`/`System`).
