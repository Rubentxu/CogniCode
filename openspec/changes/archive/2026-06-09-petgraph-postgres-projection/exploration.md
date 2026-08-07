# Exploration: petgraph-postgres-projection

**Status**: ✅ Complete  
**Date**: 2026-06-09  
**Executor**: sdd-explore  
**Change**: petgraph-postgres-projection  

---

## Current State

### petgraph in the workspace (already present!)

The workspace declares `petgraph = "0.6"` as a non-optional, non-feature-gated dependency in `Cargo.toml`. It is consumed in exactly two places within `cognicode-core`:

| Location | Usage | Description |
|----------|-------|-------------|
| `infrastructure/graph/pet_graph_store.rs` | `DiGraph<NodeData, DependencyType>` | `PetGraphStore` — in-memory graph store implementing `DependencyRepository` with BFS, DFS, `tarjan_scc`, path checks |
| `domain/aggregates/call_graph.rs:839-862` | Ad-hoc `DiGraph<(), ()>` + `tarjan_scc` | `module_dependencies()` — builds a module-level DiGraph and runs SCC to detect module dependency cycles |

**`cognicode-explorer` has zero petgraph usage.** The explorer accesses graph data exclusively through the `SymbolRepository` port and `CallGraph` aggregate — it never touches petgraph directly.

### CallGraph structure (canonical data model)

`CallGraph` (in `cognicode-core/src/domain/aggregates/call_graph.rs`) contains:
- `symbols: HashMap<SymbolId, Symbol>` — all graph nodes
- `edges: HashMap<SymbolId, HashMap<(SymbolId, DependencyType), (Provenance, f64)>>` — directed edges with provenance+confidence metadata
- `reverse_edges: HashMap<SymbolId, HashSet<SymbolId>>` — reverse index for incoming edges
- `name_index: HashMap<String, Vec<SymbolId>>` — case-insensitive name lookup

The aggregate exposes multiple iteration APIs suitable for projection:
- `all_dependencies()` — `(source, target, dep_type)` iterator
- `edges_with_metadata()` — `(source, target, dep_type, provenance, confidence)` iterator
- `dependencies_with_metadata(id)` — per-symbol metadata edges
- `symbol_ids()` / `symbols()` — node iteration

**Verdict: `CallGraph` has all the data needed to construct a `petgraph` projection.**

### PostgreSQL bridge (how data reaches memory)

The `postgres_bridge.rs` in `cognicode-explorer` provides:
```rust
pub async fn open_graph_from_postgres(database_url: &str) -> anyhow::Result<Arc<CallGraph>>
```
This loads a full `CallGraph` from PostgreSQL (via `PostgresRepository::load_call_graph()`) and wraps it in `Arc`. The explorer then wraps it in `CallGraphRepository` (an adapter implementing `SymbolRepository`).

**Flow: PG → load_call_graph() → CallGraph → Arc<CallGraph> → CallGraphRepository → SymbolRepository**

A petgraph projection would sit between `CallGraph` and algorithm consumers — it takes a `&CallGraph` reference and produces a `StableGraph` for in-memory algorithm execution.

### MCP envelope (metadata flow)

The `mcp-postgres-envelope` slice ensured provenance+confidence metadata flows through to MCP JSON payloads via `TypedRelation` DTOs. The projection can leverage this for confidence-weighted algorithms.

---

## Affected Areas

- `crates/cognicode-core/src/infrastructure/graph/` — **Where the projection lands.** Existing files: `pet_graph_store.rs`, `symbol_index.rs`, `lightweight_index.rs`, `strategy.rs`, `mod.rs`
- `crates/cognicode-core/src/domain/aggregates/call_graph.rs` — **Data source.** No changes needed — the projection constructor calls existing public iteration APIs
- `crates/cognicode-explorer/src/postgres_bridge.rs` — **Consumer.** The projection is called by the explorer service (or MCP handler) after loading the graph. No bridge changes needed
- `crates/cognicode-core/src/domain/traits/dependency_repository.rs` — **Existing algorithm trait.** Already has `find_impact_scope`, `detect_cycles`, `has_path`, etc. on `DependencyRepository`. The projection could either integrate with this trait or provide its own algorithm surface
- `crates/cognicode-core/Cargo.toml` — **No changes.** `petgraph` is already a non-optional dep

---

## Approaches

### 1. Extend PetGraphStore with from_call_graph() constructor

Add a `from_call_graph(call_graph: &CallGraph) -> Self` constructor to the existing `PetGraphStore` struct. The store already implements `DependencyRepository` with BFS, DFS, `tarjan_scc`, `has_path` — adding the constructor gives us all these algorithms for free.

```rust
impl PetGraphStore {
    pub fn from_call_graph(call_graph: &CallGraph) -> Self { ... }
}
```

| Pros | Cons |
|------|------|
| ✅ Reuses all existing `DependencyRepository` algorithm methods | ⚠️ `PetGraphStore` is designed as a *mutable store* (`add_dependency(&mut self, ...)`) — projection should be read-only |
| ✅ Zero new traits | ⚠️ `NodeData` wrapper currently stores `Symbol` inline (heavy for 100K+ nodes) |
| ✅ `to_call_graph()` already exists for round-trip | ⚠️ `ensure_symbol()` creates placeholder Symbols which is wrong for a projection from real data |
| Effort: Low | |

### 2. New CallGraphProjection struct (recommended)

Create a new dedicated `CallGraphProjection` struct in `infrastructure/graph/` that wraps `petgraph::StableGraph<SymbolId, (DependencyType, f64)>` with a `from_call_graph()` constructor and algorithm methods.

```rust
pub struct CallGraphProjection {
    graph: petgraph::stable_graph::StableGraph<SymbolId, (DependencyType, f64)>,
    // Optional side-lookup for Symbol resolution
    symbol_lookup: HashMap<SymbolId, Symbol>,
}

impl CallGraphProjection {
    pub fn from_call_graph(call_graph: &CallGraph) -> Self { ... }
    pub fn topological_sort(&self) -> Vec<&SymbolId> { ... }
    pub fn dijkstra(&self, start: &SymbolId, goal: &SymbolId) -> Option<(f64, Vec<SymbolId>)> { ... }
    pub fn strongly_connected_components(&self) -> Vec<Vec<&SymbolId>> { ... }
    pub fn detect_cycles(&self) -> Vec<Vec<&SymbolId>> { ... }
    pub fn connected_components(&self) -> Vec<Vec<&SymbolId>> { ... }
    pub fn to_call_graph(&self) -> CallGraph { ... }
}
```

| Pros | Cons |
|------|------|
| ✅ Clean separation: projection is read-only, immutable | ⚠️ New module (150-200 lines), minimal duplication of algorithm wrappers |
| ✅ `StableGraph` has stable `NodeIndex` — safer for incremental algorithms | ⚠️ Does NOT reuse `DependencyRepository` trait directly |
| ✅ Edge weight `(DependencyType, f64)` supports confidence-weighted algorithms | |
| ✅ Does not mutate — safe to share across threads | |
| ✅ Light node weight (`SymbolId`) — memory efficient | |
| Effort: Low-Medium | |

### 3. Add to_petgraph() method directly on CallGraph

Add `fn to_petgraph() -> petgraph::StableGraph<...>` directly on the `CallGraph` aggregate in the domain layer.

| Pros | Cons |
|------|------|
| ✅ Simplest API: `let g = call_graph.to_petgraph()` | ❌ Violates DDD — domain aggregate depends on infrastructure concern (petgraph) |
| ✅ One-liner from any CallGraph holder | ❌ `CallGraph` is already 1670 lines — adding projection logic bloats the aggregate |
| Effort: Lowest | ❌ Per `core-mcp-boundaries.md`, petgraph belongs in infrastructure layer |

---

## Node/Edge Weight Design

### Node weight: `SymbolId`
- `Symbol` (with `FunctionSignature`, `Location`, etc.) is too heavy for graph algorithms
- `SymbolId` is a newtype over `String`, `Hash + Eq + Clone`
- Algorithms need topology, not symbol details
- Add a side-lookup `HashMap<SymbolId, Symbol>` for resolving names post-algorithm

### Edge weight: `(DependencyType, f64)`
- `DependencyType` enum: `Calls | Imports | Inherits | UsesGeneric | References | Defines | AnnotatedBy | Contains`
- `f64` confidence score in `[0.0, 1.0]`
- Algorithms that don't need confidence can ignore the `f64`
- Confidence-weighted algorithms (e.g., weighted Dijkstra, weighted PageRank) use `f64` as edge cost: `1.0 - confidence` = traversal cost
- Must sanitize: reject `NaN`, `infinite`, clamp to `[0.0, 1.0]`

---

## Algorithms Already Present vs. To Add

### Already available (via `CallGraph` or `PetGraphStore`)
| Algorithm | Source | Method |
|-----------|--------|--------|
| BFS path finding | CallGraph | `find_path`, `find_path_with_max_depth` |
| Transitive dependents | CallGraph | `find_all_dependents` |
| Transitive dependencies | CallGraph | `find_all_dependencies` |
| Roots/Leaves | CallGraph | `roots()`, `leaves()` |
| Connected components | CallGraph | `connected_components()` |
| Module SCC | CallGraph | `module_dependencies()` (tarjan_scc) |
| Impact scope (BFS) | PetGraphStore | `find_impact_scope()` |
| Cycle detection | PetGraphStore | `detect_cycles()` (tarjan_scc) |
| Path check (DFS) | PetGraphStore | `has_path()` |

### Obvious next candidates via petgraph::algo
| Algorithm | petgraph function | Use case |
|-----------|-------------------|----------|
| Topological sort | `petgraph::algo::toposort` | Build order, dependency resolution |
| Dijkstra shortest path | `petgraph::algo::dijkstra` | Minimum call distance between symbols |
| Kosaraju SCC | `petgraph::algo::kosaraju_scc` | Alternative SCC detection |
| Condensation | `petgraph::algo::condensation` | SCC→DAG for macro-level analysis |
| Bellman-Ford | `petgraph::algo::bellman_ford` | Shortest paths with negative weights |
| Is cyclic directed | `petgraph::algo::is_cyclic_directed` | Quick cycle check |
| Connected components (undirected) | `petgraph::algo::connected_components` | Cohesion analysis |

### Needs custom implementation
| Algorithm | Why not in petgraph | Priority |
|-----------|---------------------|----------|
| Betweenness centrality | Not in petgraph; implement with Brandes' algorithm | Medium |
| PageRank | Not in petgraph; 20-line iterative implementation | High |
| Community detection (Louvain) | Not in petgraph; ~100-line implementation | Low |
| Impact radius stats | Custom BFS with depth histogram | Medium |

---

## Relationship to Completed Slices

| Slice | Relationship |
|-------|-------------|
| `explorer-graph-foundation` | Provided `(Provenance, f64)` edge metadata the projection preserves |
| `explorer-graph-postgres-graphstore` | `load_call_graph()` is the data source for `from_call_graph()` |
| `explorer-bridge-postgres` | `open_graph_from_postgres()` produces the `Arc<CallGraph>` the projection consumes |
| `mcp-postgres-envelope` | MCP tools can call projection algorithms and return enriched results |

---

## Recommendation

**Approach 2: New `CallGraphProjection` struct** in `cognicode-core::infrastructure::graph::call_graph_projection`.

- **Node weight**: `SymbolId` with side-lookup `HashMap<SymbolId, Symbol>` for name resolution
- **Edge weight**: `(DependencyType, f64)` — supports both type-aware and confidence-weighted algorithms
- **Graph type**: `StableGraph` — stable NodeIndex for potential incremental algorithms
- **Feature gate**: None — `petgraph` is already non-optional in `cognicode-core`
- **Size estimate**: ~250-350 lines (projection struct + algorithms + tests)
- **Breaking change risk**: Zero — purely additive module, no existing files modified
- **New traits**: None — projection is a concrete struct with inherent methods
- **New deps**: None — all needed crates already in workspace

### Proposed file: `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs`

```rust
//! petgraph projection of a `CallGraph` for graph algorithm execution.
//!
//! This is a read-side projection — constructed from a `&CallGraph`,
//! never mutated. Algorithms run against the in-memory `StableGraph`
//! and return results that can be resolved back to `Symbol` data
//! through the side-lookup.

use petgraph::stable_graph::StableGraph;
use petgraph::visit::{IntoNodeReferences, EdgeRef};
use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
use crate::domain::value_objects::DependencyType;
use std::collections::HashMap;

type EdgeWeight = (DependencyType, f64);

pub struct CallGraphProjection {
    graph: StableGraph<SymbolId, EdgeWeight>,
    /// Maps SymbolId → full Symbol for post-algorithm resolution.
    symbols: HashMap<SymbolId, Symbol>,
}
```

---

## Risks

1. **Memory overhead**: `StableGraph` stores adjacency lists separately from `CallGraph`'s `HashMap`-based edges. For a 50K-node graph with 200K edges, this adds ~10-20MB. Acceptable for the target scale.
2. **`f64` confidence in edge weights**: Must sanitize on construction — `NaN` and `infinite` values must be replaced with `1.0` (the default for directly-extracted edges).
3. **Accidental coupling**: Explorer crate must never depend on petgraph directly. The projection is called in core or through a service method that returns serializable results.
4. **Staleness**: If the `CallGraph` is mutated after projection construction, the projection is stale. Mitigation: projection is constructed on-demand (stateless) or invalidated when the source CallGraph changes.

---

## Ready for Proposal

**Yes.** The exploration confirms:
- petgraph is already in the workspace and used in cognicode-core ✅
- CallGraph has all data needed for projection ✅
- The correct crate is cognicode-core ✅
- The existing bridge and envelope are compatible ✅
- No breaking changes, no new traits, no new deps ✅
- Approach is a single new module, ~250-350 lines ✅
- DDD/hex architecture is preserved: projection is infrastructure, consumes domain model ✅

**Next**: sdd-propose phase with a detailed scope, spec, and task breakdown.
