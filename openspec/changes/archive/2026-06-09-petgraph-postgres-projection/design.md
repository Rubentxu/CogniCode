# Design: CallGraph Projection (petgraph)

## Technical Approach

Read-side projection consuming `&CallGraph` immutably. Builds a `StableGraph<SymbolId, (DependencyType, f64)>` via `edges_with_metadata()` + `symbol_ids()`. Algorithms delegate to `petgraph::algo`. No mutation of `CallGraph`, `PetGraphStore`, or traits.

## Architecture Decisions

| Decision | Choice | Rejected | Rationale |
|----------|--------|----------|-----------|
| Graph type | `StableGraph` | `DiGraph` | Preserves `NodeIndex` stability; supports parallel edges by different `DependencyType`; matches spec requirement |
| Edge weight | `(DependencyType, f64)` | `f64` alone | Dijkstra needs confidence cost; `find_impact_radius` may filter by type; avoids losing information |
| Node weight | `SymbolId` | `Symbol` | Keeps graph lightweight; side-lookup via `HashMap<SymbolId, Symbol>` for resolution |
| Dijkstra cost | `1.0 - sanitize(confidence)` | raw confidence | High confidence = low cost; sanitized confidence prevents NaN→0.0 free-edge exploit |
| `has_path(A,A)` | `true` (trivial path) | `false` | `petgraph::has_path` returns true for same-node; consistent with graph theory; spec did not mandate either, this is the less-surprising default |
| `ProjectionError` | 3 variants | 1 variant only | `CycleDetected` (topo), `NodeNotFound(SymbolId)` (future), `InvalidGraph(String)` (future) |
| Domain isolation | `CallGraphProjection` in `infrastructure::graph` | domain layer | petgraph is infrastructure concern; domain aggregate stays petgraph-free |
| Constructor fidelity | Edge-by-edge from `edges_with_metadata()` | Bulk conversion | Guarantees `edge_count() == graph.edge_count()` per spec Req 1 |

## Data Flow

```
CallGraph (domain aggregate)
    │  edges_with_metadata() + symbol_ids()
    ▼
CallGraphProjection::from_call_graph()
    │  sanitize confidence → build StableGraph + side-lookup
    ▼
StableGraph<SymbolId, (DependencyType, f64)>
    + HashMap<SymbolId, Symbol>
    │
    ├── topological_sort()      → petgraph::algo::toposort
    ├── strongly_connected()    → petgraph::algo::tarjan_scc
    ├── detect_cycles()         → SCC has len > 1
    ├── connected_components()  → petgraph::algo::connected_components + mapping
    ├── has_path()              → petgraph::algo::has_path_connecting
    ├── dijkstra()              → petgraph::algo::astar with cost_fn
    ├── find_impact_radius()    → reverse BFS bounded by max_depth
    └── resolve_symbol()        → HashMap side-lookup
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs` | Create | `CallGraphProjection` struct + `ProjectionError` + algorithm methods |
| `crates/cognicode-core/src/infrastructure/graph/mod.rs` | Modify | Add `mod call_graph_projection;` + `pub use` |
| `crates/cognicode-core/src/domain/aggregates/call_graph.rs` | None | Consumed via existing public API only |

## Interfaces / Contracts

```rust
// call_graph_projection.rs

use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::value_objects::DependencyType;
use petgraph::stable_graph::StableGraph;
use std::collections::HashMap;

/// Errors from projection algorithms
#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error("cycle detected in graph")]
    CycleDetected,
}

/// Confidence sanitization: NaN/±∞ → 1.0, then clamp [0.0, 1.0]
#[inline]
fn sanitize_confidence(val: f64) -> f64 {
    if !val.is_finite() { 1.0 } else { val.clamp(0.0, 1.0) }
}

/// Dijkstra edge cost: 1.0 - sanitized confidence
#[inline]
fn dijkstra_cost(confidence: f64) -> f64 {
    1.0 - sanitize_confidence(confidence)
}

pub struct CallGraphProjection {
    graph: StableGraph<SymbolId, (DependencyType, f64)>,
    symbol_lookup: HashMap<SymbolId, Symbol>,
    id_to_index: HashMap<SymbolId, petgraph::graph::NodeIndex>,
}

impl CallGraphProjection {
    pub fn from_call_graph(cg: &CallGraph) -> Self { /* ... */ }

    pub fn topological_sort(&self) -> Result<Vec<SymbolId>, ProjectionError>;
    pub fn strongly_connected_components(&self) -> Vec<Vec<SymbolId>>;
    pub fn detect_cycles(&self) -> bool;
    pub fn connected_components(&self) -> Vec<Vec<SymbolId>>;
    pub fn has_path(&self, from: &SymbolId, to: &SymbolId) -> bool;
    pub fn dijkstra(&self, from: &SymbolId, to: &SymbolId) -> Option<(Vec<SymbolId>, f64)>;
    pub fn find_impact_radius(&self, root: &SymbolId, max_depth: usize) -> Vec<SymbolId>;
    pub fn resolve_symbol(&self, id: &SymbolId) -> Option<&Symbol>;
}
```

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | Construction fidelity | Node count, edge count, parallel edges preserved |
| Unit | Confidence sanitization | Vector test: NaN, +∞, -∞, 1.5, -0.2, 2.0, 0.0, 1.0 |
| Unit | `topological_sort` | Empty → Ok([]), DAG → order, cyclic → Err |
| Unit | `strongly_connected_components` | Self-loop singleton, DAG N singletons, 2-node cycle |
| Unit | `detect_cycles` | Consistent with SCC: self-loop → true, DAG → false |
| Unit | `connected_components` | Undirected: 2 subgraphs A→B, C→D |
| Unit | `has_path` | Direct, transitive, no-path, missing, A→A true |
| Unit | `dijkstra` | Cost = 1.0-confidence; unreachable None; missing None |
| Unit | `find_impact_radius` | Reverse BFS; missing → []; empty → []; bounded depth |
| Unit | `resolve_symbol` | Found → Some, missing → None |
| Integration | Re-export | `infrastructure::graph::CallGraphProjection` reachable |
| Integration | No regression | `cargo test -p cognicode-core` green |

## Migration / Rollout

No migration required. Pure additive change. Rollback = delete `call_graph_projection.rs`, revert `mod.rs`.

## Open Questions

None. All 10 design questions resolved above.
