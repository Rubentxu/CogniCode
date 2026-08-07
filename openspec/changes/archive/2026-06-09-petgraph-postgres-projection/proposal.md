# Proposal: petgraph-postgres-projection

## Intent

After the PG bridge and MCP envelope slices, `CallGraph` data flows from PostgreSQL into memory. We need a petgraph-based read-side projection to unlock native graph algorithms (SCC, Dijkstra, topological sort) without mutating the canonical `CallGraph` aggregate or bloating the existing mutable `PetGraphStore`. This is the algorithmic layer that makes graph analysis fast and idiomatic.

## Scope

### In Scope
- New `CallGraphProjection` struct in `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs`
- Constructor `from_call_graph(&CallGraph) -> Self` consuming public iteration APIs
- Graph type: `StableGraph<SymbolId, (DependencyType, f64)>` with side-lookup `HashMap<SymbolId, Symbol>`
- Algorithm wrappers: `topological_sort`, `dijkstra`, `strongly_connected_components`, `connected_components`, `has_path`, `detect_cycles`, `find_impact_radius`
- `f64` confidence sanitation (reject NaN/inf, clamp to [0,1])
- Unit tests covering construction, algorithms, and edge cases (empty graph, cycle-only, NaN)
- Re-export from `infrastructure::graph::mod`

### Out of Scope
- No explorer UI or MCP tool changes
- No PostgreSQL schema changes
- No replacement of `PetGraphStore` (remains the mutable `DependencyRepository` impl)
- No new traits — concrete struct with inherent methods
- No `CallGraph` modifications
- No new dependencies (`petgraph` already in workspace)

## Capabilities

### New Capabilities
- `callgraph-petgraph-projection`: Read-side petgraph projection over `CallGraph` providing SCC, topological sort, Dijkstra shortest path, connected components, cycle detection, and impact radius analysis. Consumes `&CallGraph` immutably.

### Modified Capabilities
None. All existing capabilities (`postgres-callgraph-persistence`, `explorer-postgres-bridge`, `mcp-edge-metadata`) are consumed, not modified.

## Approach

**Recommended: Approach 2 — New `CallGraphProjection` struct** (from exploration).

Create a dedicated read-side struct wrapping `petgraph::stable_graph::StableGraph`. Constructor iterates `CallGraph::edges_with_metadata()` and `CallGraph::symbols()` to build nodes and edges. Algorithms delegate to `petgraph::algo` functions. `SymbolId` as node weight (light), `(DependencyType, f64)` as edge weight (supports confidence-weighted traversal). Side-lookup resolves `SymbolId` → `Symbol` post-algorithm.

Why NOT extend `PetGraphStore`: it is mutable infrastructure that creates placeholder `Symbol`s — wrong for a read-side projection from authoritative data.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs` | New | Projection struct + algorithm methods + tests |
| `crates/cognicode-core/src/infrastructure/graph/mod.rs` | Modified | Add `mod call_graph_projection;` + re-export |
| `crates/cognicode-core/src/domain/aggregates/call_graph.rs` | None | Data source — consumed via existing public API |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Stale projection after `CallGraph` mutation | Low | Projections are constructed on-demand; document immutability contract in docstring |
| Memory overhead for large graphs (>100K nodes) | Low | `StableGraph` overhead ~10-20MB for 50K nodes; acceptable for target scale |
| `f64` NaN/inf in confidence poisoning algorithms | Low | Sanitize on construction: `if !val.is_finite() { 1.0 } else { val.clamp(0.0, 1.0) }` |
| Accidental explorer coupling to petgraph | Low | Projection lives in core; explorer calls through core service methods returning serializable results |

## Rollback Plan

Remove `call_graph_projection.rs`, revert `mod.rs` re-export. No other files touched. Build should succeed immediately. No migration, no schema rollback.

## Dependencies

- `petgraph = "0.6"` (already in workspace, non-optional)
- `CallGraph` aggregate (existing, stable public API)
- `DependencyType`, `Provenance` value objects (existing)

## Success Criteria

- [ ] `CallGraphProjection::from_call_graph()` constructs correct `StableGraph` from any `CallGraph`
- [ ] All algorithm methods return correct results verified against known graphs
- [ ] `f64` confidence sanitation handles NaN, +inf, -inf, values outside [0,1]
- [ ] Empty graph, single-node, and cycle-only graphs handled without panics
- [ ] `cargo test -p cognicode-core` passes
- [ ] No clippy warnings
- [ ] Module re-exported from `infrastructure::graph`

## Entropy Budget

**Method**: Heuristic (additive change, no CogniCode impact needed)

| Metric | Estimate | Threshold | Status |
|--------|----------|-----------|--------|
| H(Δ_existing) | 0.0 bits | < 1.0 | ✅ |
| H(Δ_new) | 1.5 bits | > 0 | ✅ |
| New connascence pairs | 1 (Name: CallGraph → CallGraphProjection) | < 3 | ✅ |
| OCP compliant? | Yes | yes | ✅ |

**Verdict**: 🟢 Green — pure extension, zero existing code changes. Projection is an infrastructure consumer of the domain aggregate via stable public APIs.
