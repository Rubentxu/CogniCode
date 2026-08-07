# Delta for impact-analysis-service

> Adds three new delegation methods to the existing
> `ImpactAnalysisService`: `subgraph`, `cluster_components`,
> `explain_path`. Existing methods (and their tests) MUST be
> unaffected. Zero changes to existing method signatures.

## ADDED Requirements

### Requirement: `subgraph(root, direction, max_depth)`

`pub fn subgraph(&self, graph: &CallGraph, root: &SymbolId, direction:
&str, max_depth: usize) -> SubgraphResultDto` MUST delegate to
`CallGraphProjection::extract_subgraph` after building the projection
via `CallGraphProjection::from_call_graph(graph)`. MUST map
`direction == "incoming" | "outgoing" | "both"` to the corresponding
`SubgraphDirection`. MUST convert `SymbolId` to `String` via
`as_str().to_string()`. MUST convert the edge list to
`SubgraphEdgeDto` and serialize the `DependencyType` as its `Debug`
string. MUST return `SubgraphResultDto { nodes: vec![root.as_str()...],
edges: [...] }` with the root echoed back even when missing or
isolated. MUST be `&self` and take `graph: &CallGraph` immutably.

#### Scenario: Service layer mirrors projection for outgoing depth 2

- GIVEN graph `A → B → C`, `A → D`
- WHEN `subgraph(&graph, &A, "outgoing", 2)` is called
- THEN `nodes` equals `{A, B, C, D}` (any order) AND `edges` contains
  `(A→B)`, `(B→C)`, `(A→D)` with the correct metadata

#### Scenario: Service layer mirrors projection for `both`

- GIVEN graph `D → A → C`, `B → C`
- WHEN `subgraph(&graph, &A, "both", 2)` is called
- THEN `nodes == {A,B,C,D}` AND `edges` equals
  `{(D→A), (A→C), (B→C)}`

#### Scenario: Unknown direction string is rejected

- GIVEN any graph
- WHEN `subgraph(&graph, &A, "sideways", 3)` is called
- THEN the method MUST panic with a clear message OR return an
  `Err`-style variant; this is the only place where an invalid string
  is acceptable. The MCP layer rejects it earlier (preferred path).

#### Scenario: Missing root is echoed back as the only node

- GIVEN projection that does not contain `m`
- WHEN `subgraph(&graph, &m, "outgoing", 5)` is called
- THEN `nodes == vec![m.as_str().to_string()]` AND `edges == vec![]`

#### Scenario: Empty graph returns just the root

- GIVEN `CallGraph::new()` (0 nodes)
- WHEN `subgraph(&graph, &"X", "outgoing", 5)` is called
- THEN `nodes == vec!["X"]` AND `edges == vec![]` AND no panic

#### Scenario: `max_depth == 0` returns just the root

- GIVEN graph `A → B`
- WHEN `subgraph(&graph, &A, "outgoing", 0)` is called
- THEN `nodes == vec!["A"]` AND `edges == vec![]`

### Requirement: `cluster_components(method)`

`pub fn cluster_components(&self, graph: &CallGraph, method: &str) ->
ClusterResultDto` MUST delegate as follows:
- `method == "scc"` → `CallGraphProjection::strongly_connected_components()`
- `method == "connected"` → `CallGraphProjection::connected_components()`
MUST convert each `Vec<SymbolId>` to `ClusterDto { members: Vec<String>,
size: usize }`. MUST return `vec![]` for an empty graph. MUST be
`&self` and take `graph: &CallGraph` immutably.

#### Scenario: SCC method detects a mutual cycle

- GIVEN graph `A → B → A` plus an isolated `C`
- WHEN `cluster_components(&graph, "scc")` is called
- THEN result has 2 entries: one of size 2 `{A, B}` and one singleton
  `{C}` (order not asserted)

#### Scenario: Connected method treats edges as undirected

- GIVEN graph `A → B`, `B → A` plus `C → D`
- WHEN `cluster_components(&graph, "connected")` is called
- THEN result has 2 entries: `{A, B}` and `{C, D}` (each size 2)

#### Scenario: Empty graph returns empty list

- GIVEN `CallGraph::new()` (0 nodes)
- WHEN `cluster_components(&graph, "scc")` is called
- THEN result is `vec![]`

#### Scenario: Self-loop is a singleton cluster

- GIVEN graph containing `A → A` only
- WHEN `cluster_components(&graph, "scc")` is called
- THEN result is `vec![ClusterDto { members: vec!["A"], size: 1 }]`

### Requirement: `explain_path(from, to)`

`pub fn explain_path(&self, graph: &CallGraph, from: &SymbolId, to: &SymbolId) -> Option<ExplainResultDto>` MUST delegate to
`CallGraphProjection::explain_path` after building the projection via
`CallGraphProjection::from_call_graph(graph)`. MUST convert the result
to `ExplainResultDto { found: bool, hops: Vec<ExplainHopDto>,
total_cost: f64, summary: String }`:
- On `Some(view)` → `found: true`, `hops` built from the view's
  per-hop data, `total_cost` from the view, `summary` built via
  `ExplainResultDto::from_path(...)` (templated one-liner).
- On `None` → `Some(ExplainResultDto { found: false, hops: vec![],
  total_cost: 0.0, summary: "No path from <from> to <to>".to_string() })`
  (NOT `None` — the service returns `Some(_)` with `found: false` so
  the MCP tool can return `is_error == false` with a structured
  payload).

MUST be `&self` and take `graph: &CallGraph` immutably.

#### Scenario: Two-hop path is explained

- GIVEN graph `A → B → C` with conf 1.0 on both edges
- WHEN `explain_path(&graph, &A, &C)` is called
- THEN result is
  `Some(ExplainResultDto { found: true, hops: [{A,B,Calls,1.0,...}, {B,C,Calls,1.0,...}], total_cost: 0.0, summary: "A → B → C (2 hops, total cost 0.00)" })`

#### Scenario: Unreachable pair returns `Some(_)` with `found: false`

- GIVEN graph `A → B` only
- WHEN `explain_path(&graph, &A, &Z)` is called
- THEN result is `Some(ExplainResultDto { found: false, hops: vec![],
  total_cost: 0.0, summary: "No path from A to Z" })` (not `None`)

#### Scenario: Missing endpoint returns `Some(_)` with `found: false`

- GIVEN projection that lacks `m`
- WHEN `explain_path(&graph, &known, &m)` is called
- THEN result is `Some(_)` with `found: false` AND no panic

#### Scenario: Self-path returns one self-hop

- GIVEN graph containing `A` (any edges or none)
- WHEN `explain_path(&graph, &A, &A)` is called
- THEN `found: true` AND `hops.len() == 1` AND
  `hops[0] == {A, A, "Calls", 1.0, "A → A (self)"}` AND
  `total_cost == 0.0`

### Requirement: Read-only consumption of `CallGraph` (carried over, unchanged)

`ImpactAnalysisService` MUST NOT modify `CallGraphProjection`,
`CallGraph`, or `PetGraphStore`. The three new methods MUST be `&self`
and accept `graph: &CallGraph` immutably.

#### Scenario: Repeated calls are non-mutating

- GIVEN a `CallGraph` with 5 symbols and 7 edges
- WHEN 100 calls of all 8 service methods (5 existing + 3 new) are
  made in sequence
- THEN `graph.symbol_count()` and `graph.edge_count()` are unchanged
  after each call

### Requirement: No new dependencies (carried over, unchanged)

MUST NOT add crates to `crates/cognicode-core/Cargo.toml`. Reuse
`CallGraphProjection`, `SubgraphView`, `ExplanationView`,
`DependencyType`, `SymbolId`, and existing serde derives.

#### Scenario: Cargo.toml is byte-identical

- GIVEN pre-slice `Cargo.toml`
- WHEN the spec is implemented
- THEN `git diff crates/cognicode-core/Cargo.toml` is empty

### Requirement: Test coverage for new methods

`#[cfg(test)] mod tests` MUST include at least one test per new public
method (`subgraph`, `cluster_components`, `explain_path`) plus every
edge-case scenario enumerated above. All tests MUST use in-memory
`CallGraph::new()` with `add_symbol` + `add_dependency_with_provenance`.
No `#[ignore]` without documented rationale.

#### Scenario: All edge cases covered

- WHEN `cargo test -p cognicode-core` runs
- THEN every edge case (missing symbol, zero depth, disconnected
  graph, no path, empty graph, NaN confidence, self-path, self-loop)
  is exercised by ≥1 passing test for the new methods

## REMOVED Requirements

None.

## Out of Scope (locked — unchanged from base spec)

- No modification to `ImpactAnalyzer` (stays count-based)
- No modification to existing `CallGraphProjection` methods
- No replacement of `CycleDetector`
- No multi-root impact queries
- No confidence-weighted impact scoring
- No MCP tool exposure (handled in `graph-subgraph` / `graph-cluster`
  / `graph-explain` specs)
- No PostgreSQL schema changes
- No new traits; no new external dependencies
- No async API
- No mutation of the projection or the aggregate
