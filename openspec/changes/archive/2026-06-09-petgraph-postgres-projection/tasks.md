# Tasks: CallGraph Projection (petgraph)

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~280 (240 new in `call_graph_projection.rs` + ~3 in `mod.rs`) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | single PR |
| Delivery strategy | single-pr |
| Chain strategy | size-exception |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low

### Suggested Work Units

Not needed — single-PR scope (~280 lines) is well under the 400-line review budget. One cohesive feature, one file, no migration. Work is purely additive.

## Phase 1: Foundation — Types, Helpers, Module Wiring

- [x] 1.1 In `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs` (new file): add module-level `//!` docstring + imports (`CallGraph`, `SymbolId`, `Symbol`, `DependencyType`, `petgraph::stable_graph::StableGraph`, `petgraph::graph::NodeIndex`, `HashMap`).
- [x] 1.2 In the same file: define `pub enum ProjectionError { #[error("cycle detected in graph")] CycleDetected }` with `#[derive(Debug, thiserror::Error)]`; add `#[non_exhaustive]` if matching repo style — confirm via `grep` on adjacent error enums.
- [x] 1.3 In the same file: add private `fn sanitize_confidence(val: f64) -> f64` — `if !val.is_finite() { 1.0 } else { val.clamp(0.0, 1.0) }` (spec Req 2).
- [x] 1.4 In the same file: add private `fn dijkstra_cost(confidence: f64) -> f64` returning `1.0 - sanitize_confidence(confidence)` (design decision D4).
- [x] 1.5 In the same file: declare `pub struct CallGraphProjection { graph: StableGraph<SymbolId, (DependencyType, f64)>, symbol_lookup: HashMap<SymbolId, Symbol>, id_to_index: HashMap<SymbolId, NodeIndex> }`.
- [x] 1.6 In `crates/cognicode-core/src/infrastructure/graph/mod.rs`: append `mod call_graph_projection;` and `pub use call_graph_projection::{CallGraphProjection, ProjectionError};`; preserve alphabetical-ish ordering (matches spec Req 12).

## Phase 2: Core Implementation — Constructor + 8 Algorithm Methods

- [x] 2.1 `impl CallGraphProjection { pub fn from_call_graph(cg: &CallGraph) -> Self }` — iterate `cg.symbol_ids()` to populate `symbol_lookup` + add nodes + build `id_to_index`; iterate `cg.edges_with_metadata()` edge-by-edge to add edges with `(dep_type, sanitize_confidence(raw))` (spec Req 1, design fidelity guarantee).
- [x] 2.2 `pub fn topological_sort(&self) -> Result<Vec<SymbolId>, ProjectionError>` — call `petgraph::algo::toposort(&self.graph, None)`; map `Err(_)` → `Err(ProjectionError::CycleDetected)`; map `Ok(order)` → `Ok(order.into_iter().map(|ni| *self.graph[ni]).collect())`; empty graph → `Ok(vec![])` (spec Req 3).
- [x] 2.3 `pub fn strongly_connected_components(&self) -> Vec<Vec<SymbolId>>` — call `petgraph::algo::tarjan_scc(&self.graph)`; map each `Vec<NodeIndex>` to `Vec<SymbolId>` via the `StableGraph` indexer; self-loops stay singletons (spec Req 4).
- [x] 2.4 `pub fn detect_cycles(&self) -> bool` — return `self.strongly_connected_components().iter().any(|scc| scc.len() > 1 || self.has_self_loop_in(scc))` OR re-derive from `topological_sort().is_err()` — pick the cheaper single-pass check; empty graph → `false` (spec Req 5).
- [x] 2.5 `pub fn connected_components(&self) -> Vec<Vec<SymbolId>>` — call `petgraph::algo::connected_components(&self.graph)`; map `Vec<NodeIndex>` partition to `Vec<Vec<SymbolId>>`; isolated nodes are singletons (spec Req 6).
- [x] 2.6 `pub fn has_path(&self, from: &SymbolId, to: &SymbolId) -> bool` — look up `from_node` and `to_node` in `id_to_index`; return `false` (no panic) if either is missing; else `petgraph::algo::has_path_connecting(&self.graph, from_node, to_node, None)` (spec Req 7, design D5: `A→A` returns `true`).
- [x] 2.7 `pub fn dijkstra(&self, from: &SymbolId, to: &SymbolId) -> Option<(Vec<SymbolId>, f64)>` — use `petgraph::algo::astar` with cost closure `|e: petgraph::graph::EdgeReference| e.weight().1`; return `None` on missing ids or unreachable; on success return `(path_nodes, total_cost)` mapped back to `SymbolId` (spec Req 8).
- [x] 2.8 `pub fn find_impact_radius(&self, root: &SymbolId, max_depth: usize) -> Vec<SymbolId>` — reverse BFS over predecessors (use `petgraph::graph::Graph::neighbors_directed(n, Incoming)`); track depth; return `vec![]` on missing root or `max_depth == 0`; deduplicate visited set (spec Req 9).
- [x] 2.9 `pub fn resolve_symbol(&self, id: &SymbolId) -> Option<&Symbol>` — `self.symbol_lookup.get(id)` (spec Req 10).

## Phase 3: Testing — 11 Unit Tests + 2 Integration Checks

- [x] 3.1 `#[test] fn from_call_graph_preserves_node_and_edge_counts()` — build `CallGraph` with 3 nodes + 2 parallel edges (different `DependencyType`) between A→B; assert `projection` internal `graph.node_count()==3` and `edge_count()==2`; assert side-lookup has 3 entries (spec Req 1, parallel-edge preservation).
- [x] 3.2 `#[test] fn sanitize_confidence_handles_invalid_floats()` — table-driven over `[f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1.5, -0.2, 2.0, 0.0, 1.0]`; assert all map to `1.0, 1.0, 1.0, 1.0, 0.0, 1.0, 0.0, 1.0` respectively (spec Req 2, edge-case vector).
- [x] 3.3 `#[test] fn topological_sort_dag_returns_order()` — 3-node DAG A→B, A→C, B→C; assert `Ok(order)` and `order.len()==3`; assert topological invariant (A before B, A before C, B before C).
- [x] 3.4 `#[test] fn topological_sort_empty_graph_returns_ok_empty()` — empty `CallGraph`; assert `Ok(vec![])` (spec Req 3, empty-graph edge case).
- [x] 3.5 `#[test] fn topological_sort_cycle_returns_err()` — 2-node cycle A→B→A; assert `Err(ProjectionError::CycleDetected)` (spec Req 3).
- [x] 3.6 `#[test] fn strongly_connected_components_self_loop_is_singleton()` — single node A with self-loop; assert one SCC of `[A]`; assert `detect_cycles()==true` (spec Req 4 + 5, self-loop edge case).
- [x] 3.7 `#[test] fn strongly_connected_components_dag_returns_n_singletons()` — 3-node DAG; assert 3 SCCs, each of length 1; assert `detect_cycles()==false` (spec Req 4 + 5).
- [x] 3.8 `#[test] fn connected_components_two_subgraphs()` — build `A→B` and `C→D`; assert 2 SCCs of length 2 in undirected view (spec Req 6, disconnected-components edge case).
- [x] 3.9 `#[test] fn has_path_direct_transitive_no_path_and_missing()` — table: A→B direct=true, A→B→C transitive=true, B→A no-path=false, missing-from=false, missing-to=false, A→A=true (spec Req 7, design D5).
- [x] 3.10 `#[test] fn dijkstra_cost_is_one_minus_confidence_and_unreachable_is_none()` — graph A→B (conf=0.8) cost=0.2; A→C (conf=0.5) cost=0.5; assert `dijkstra(A,B)==Some((vec![A,B],0.2))`; assert `dijkstra(A,C)==Some((vec![A,C],0.5))`; unreachable target → `None`; missing id → `None` (spec Req 8).
- [x] 3.11 `#[test] fn find_impact_radius_reverse_bfs_bounded_by_max_depth()` — chain B→A, C→B, D→C (predecessors of A are {B,C,D}); `max_depth=1` → `[B]`; `max_depth=3` → `[B,C,D]`; missing root → `vec![]`; empty graph → `vec![]` (spec Req 9, edge cases).
- [x] 3.12 `#[test] fn resolve_symbol_found_and_missing()` — build with symbol X; assert `resolve_symbol(&X)==Some(_)`; assert unknown id → `None` (spec Req 10).
- [x] 3.13 Integration check (doc or `#[ignore]`-justified test): `use crate::infrastructure::graph::{CallGraphProjection, ProjectionError};` compiles → confirms re-export (spec Req 12, integration test).
- [x] 3.14 Run `cargo test -p cognicode-core`; assert full test suite green and zero clippy warnings (`cargo clippy -p cognicode-core --all-targets -- -D warnings`); assert `git diff` against `domain/aggregates/call_graph.rs` and `infrastructure/graph/pet_graph_store.rs` is empty; assert `cognicode-core/Cargo.toml` byte-identical to pre-slice (spec Req 11, acceptance gates).

## Phase 4: Verification & Cleanup

- [x] 4.1 Verify `git diff --stat` shows changes in exactly 2 files: `call_graph_projection.rs` (new) and `graph/mod.rs` (modified); no other crates touched.
- [x] 4.2 Run `cargo build -p cognicode-core` and `cargo test -p cognicode-core`; both must pass.
- [x] 4.3 Confirm `pub use` re-export visible: add a one-liner smoke test in 3.13 OR confirm by import in 3.1; no orphan imports.

## Dependency Map

```
Phase 1.1–1.5 (file scaffold + types) ──► Phase 1.6 (mod.rs re-export) ──► Phase 2.1 (constructor)
                                                                          │
                                                  ┌───────────────────────┼───────────────────────┐
                                                  ▼                       ▼                       ▼
                                       Phase 2.2 topo_sort       Phase 2.3 SCC            Phase 2.4 detect_cycles
                                       Phase 2.5 conn_comp       Phase 2.6 has_path      Phase 2.7 dijkstra
                                       Phase 2.8 impact_radius   Phase 2.9 resolve_symbol
                                                  │
                                                  ▼
                                            Phase 3 (all tests reference a constructed projection)
                                                  │
                                                  ▼
                                            Phase 4 (verification)
```

## Mapping to Spec Requirements

| Spec Req | Task(s) |
|----------|---------|
| R1 Struct + constructor | 1.5, 2.1, 3.1 |
| R2 f64 sanitation | 1.3, 3.2 |
| R3 topological_sort | 2.2, 3.3, 3.4, 3.5 |
| R4 SCC | 2.3, 3.6, 3.7 |
| R5 detect_cycles | 2.4, 3.6, 3.7 |
| R6 connected_components | 2.5, 3.8 |
| R7 has_path | 2.6, 3.9 |
| R8 dijkstra | 2.7, 3.10 |
| R9 impact_radius | 2.8, 3.11 |
| R10 resolve_symbol | 2.9, 3.12 |
| R11 No mutation / no new deps | 3.14, 4.1 |
| R12 Re-export | 1.6, 3.13 |
| R13 Test coverage | 3.1–3.12 |
