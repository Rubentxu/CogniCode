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

## Status

Draft. Awaiting `sdd-design`.

## Coverage

- **Happy paths**: construction, topological on DAG, SCC on DAG, Dijkstra, impact radius, side-lookup.
- **Edge cases**: empty, single-node, self-loop, multi-node cycle, NaN/±inf, clamp at 0.0/1.0, unknown `SymbolId`, disconnected components, no path, sanitize-then-traverse.
- **Error states**: `ProjectionError::CycleDetected`; `None` on missing/unreachable.

## Out of Scope (locked)

Explorer UI; MCP tools/envelope; PG schema; replacing `PetGraphStore`; new traits; modifying `CallGraph`; new external deps; mutation API on projection; async API; projection serialization; `ltree`/`pgvector`; new symbol kinds (`Component`/`Container`/`System`).
