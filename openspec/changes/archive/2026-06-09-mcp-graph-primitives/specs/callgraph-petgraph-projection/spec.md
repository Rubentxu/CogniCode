# Delta for callgraph-petgraph-projection

> Adds two new methods to the existing `CallGraphProjection`:
> `extract_subgraph` and `explain_path`. Existing methods (and their
> tests) MUST be unaffected. Zero changes to existing method signatures.

## ADDED Requirements

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
- THEN `nodes` equals `{A, B, C, D}` (any order) AND `edges` equals
  `{(A,B), (B,C), (A,D)}` with the corresponding `(DependencyType, conf)`

#### Scenario: Incoming BFS at depth 2 finds predecessors

- GIVEN graph `D → A → C`, `B → C`
- WHEN `extract_subgraph(C, Incoming, 2)`
- THEN `nodes` equals `{A, B, C, D}` AND `edges` equals
  `{(D,A), (A,C), (B,C)}`

#### Scenario: `Both` is the union of incoming + outgoing

- GIVEN graph `D → A → C`, `B → C`
- WHEN `extract_subgraph(A, Both, 2)`
- THEN `nodes` equals `{A, B, C, D}` AND `edges` equals
  `{(D,A), (A,C), (B,C)}`

#### Scenario: Default depth via wrapper accepts `usize::MAX` as "unbounded"

- GIVEN graph `A → B → C → D`
- WHEN `extract_subgraph(A, Outgoing, usize::MAX)`
- THEN `nodes` equals `{A, B, C, D}` AND BFS terminates

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
- THEN `nodes.len() == 3` AND no duplicate symbol ids AND no duplicate
  edges AND BFS terminates

#### Scenario: Dense graph termination and dedup

- GIVEN graph where `A` has 100 outgoing edges to 100 distinct nodes
  and each of those nodes points back to `A`
- WHEN `extract_subgraph(A, Both, 2)`
- THEN `nodes.len() == 101` (A + 100 successors) AND each `SymbolId`
  appears exactly once AND each edge appears exactly once

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
dependency_type: DependencyType, confidence: f64 }` is the per-hop
DTO.

Semantics:
- Cost function is identical to `dijkstra` (edge cost = `1.0 - conf`).
- `hops.len() == path.len() - 1` (one entry per directed edge traversed).
- For a self-path `from == to`, MUST return
  `Some(ExplanationView { hops: vec![ExplanationHop { from, to, dependency_type: Calls, confidence: 1.0 }], total_cost: 0.0 })`.
- Missing endpoint (`from` or `to` not in projection) MUST return
  `None` (no panic).
- Unreachable pair MUST return `None` (no panic).
- A `usize::MAX` depth sentinel is not applicable to this method.
- MUST be a `&self` method; MUST NOT mutate the projection.
- Each hop's `confidence` reflects the edge that was actually
  traversed by `dijkstra` (the winning tie-breaker in case of
  multi-edge parallel scenarios — petgraph stores one edge per
  pair, so this is the unique edge).

#### Scenario: Two-hop path is explained with per-hop metadata

- GIVEN graph `A → B → C` with confidence 1.0 on both edges
- WHEN `explain_path(A, C)`
- THEN result is
  `Some(ExplanationView { hops: vec![{A,B,Calls,1.0}, {B,C,Calls,1.0}], total_cost: 0.0 })`

#### Scenario: Shortest confidence-weighted path wins

- GIVEN `A → B` (conf 0.9), `A → C → B` (0.5, 0.5)
- WHEN `explain_path(A, B)`
- THEN `hops == [{A,B,Calls,0.9}]` AND `total_cost == 0.1`
  (direct edge beats the longer 0.5/0.5 chain with cost 1.0)

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
- THEN result is
  `Some(ExplanationView { hops: vec![{A,A,Calls,1.0}], total_cost: 0.0 })`

#### Scenario: NaN confidence edge has cost 0.0 and confidence 1.0

- GIVEN an edge with `f64::NAN` confidence that participates in the
  winning shortest path
- WHEN `explain_path` traverses it
- THEN `hops[i].confidence == 1.0` (sanitized at construction) AND the
  contribution to `total_cost` is `0.0`

#### Scenario: Empty projection returns `None`

- GIVEN projection built from `CallGraph::new()` (0 nodes)
- WHEN `explain_path(any, any)`
- THEN result is `None` AND no panic

### Requirement: New DTO types `SubgraphDirection`, `SubgraphView`, `SubgraphEdge`, `ExplanationView`, `ExplanationHop`

All MUST be `pub` and re-exported from `infrastructure::graph`. All
MUST be `Clone, Debug, PartialEq, Eq` where the underlying types
allow. Specifically:
- `SubgraphDirection` MUST derive `Copy, Clone, Debug, PartialEq, Eq,
  Hash`.
- `SubgraphView` MUST be `Clone, Debug, PartialEq, Eq` (SymbolId is
  hashable/equatable).
- `SubgraphEdge` MUST be `Clone, Debug`; `PartialEq` MAY be derived
  if `DependencyType` supports it.
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

### Requirement: No mutation / no new deps (carried over, unchanged)

The new methods MUST NOT modify `CallGraph`, `PetGraphStore`, the
projection's internal `StableGraph`, or any existing trait. They MUST
NOT add dependencies to `cognicode-core`.

#### Scenario: Existing tests still pass

- GIVEN pre-slice `CallGraphProjection` source and tests
- WHEN `cargo test -p cognicode-core` runs AND the new methods land
- THEN every pre-slice test passes AND no existing method signature
  changes AND `cognicode-core` `[dependencies]`/`[dev-dependencies]`
  are byte-identical

### Requirement: Test coverage for new methods

`#[cfg(test)] mod tests` MUST include ≥1 test per new method plus
every edge-case scenario enumerated in `extract_subgraph` and
`explain_path`. No `#[ignore]` without documented rationale.

#### Scenario: All new scenarios covered

- WHEN `cargo test -p cognicode-core` runs
- THEN every scenario above (incoming/outgoing/both, max_depth=0,
  usize::MAX, missing root, cycle, dense, self-path, NaN) is exercised
  by ≥1 passing test

## REMOVED Requirements

None.

## Out of Scope (locked — unchanged from base spec)

- Edge provenance / `EvidenceBlock` enrichment
- Async API on the projection
- Serialization of the projection itself
- Mutation API on the projection
- Replacing `PetGraphStore`
- New traits
- Modifying `CallGraph`
- New external dependencies
- ltree / pgvector / new symbol kinds
