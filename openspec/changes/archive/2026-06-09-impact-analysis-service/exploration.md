## Exploration: impact-analysis-service

### Current State

The codebase already has two distinct impact-related capabilities:

1. **`ImpactAnalyzer` domain service** (`crates/cognicode-core/src/domain/services/impact_analyzer.rs`, 362 LOC): A lightweight domain service that uses `CallGraph` aggregate methods (`callers()`, `find_all_dependents()`, `find_all_dependencies()`, `is_type_definition()`) to produce `ImpactReport` and `ImpactThreshold` safety checks. It counts direct/transitive dependents and dependencies, classifies into `ImpactLevel` (Minimal→Critical), collects affected files, and checks `is_safe_to_change()`. It has 4 unit tests. It does NOT use petgraph, has no path analysis, no SCC analysis, and no confidence-weighted scoring.

2. **`CallGraphProjection` infrastructure** (`crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs`, 666 LOC): A read-side petgraph projection over `CallGraph`. Provides native graph algorithms against `StableGraph<SymbolId, (DependencyType, f64)>`:
   - `find_impact_radius(root, max_depth)` — reverse BFS over predecessors
   - `has_path(from, to)` — reachability check (petgraph `has_path_connecting`)
   - `dijkstra(from, to)` — confidence-weighted shortest path (cost = `1.0 - confidence`)
   - `topological_sort()`, `strongly_connected_components()`, `detect_cycles()`, `connected_components()`
   - `resolve_symbol(id)`, accessors (`node_count`, `edge_count`, `symbol_count`)

The two capabilities do NOT talk to each other. `ImpactAnalyzer` is unaware of `CallGraphProjection`. The `CallGraph` aggregate itself provides `has_path()`, `find_path()`, `find_path_with_max_depth()`, `find_all_dependents()`, `find_all_dependencies()` — all BFS-based, none confidence-weighted.

The existing `CycleDetector` domain service (`cycle_detector.rs`, 424 LOC) runs Tarjan's SCC on `CallGraph` via ad-hoc `HashMap` state — it duplicates what `CallGraphProjection::strongly_connected_components()` provides natively via petgraph.

### Affected Areas

| File | Impact | Reason |
|------|--------|--------|
| `crates/cognicode-core/src/domain/services/impact_analyzer.rs` | **Potentially refactored** | Current `ImpactAnalyzer` is unaware of `CallGraphProjection`; may be replaced or supplemented |
| `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs` | **Consumed (read-only)** | Service delegates to its algorithm methods |
| `crates/cognicode-core/src/domain/services/mod.rs` | **Modified** | New re-exports if new structs added |
| `crates/cognicode-core/src/domain/aggregates/call_graph.rs` | **None** | Data source — consumed via `from_call_graph`, no changes needed |
| `crates/cognicode-core/src/domain/services/cycle_detector.rs` | **None (adjacent)** | Existing `CycleDetector` duplicates SCC from projection — noted as refactoring opportunity, out of scope for this slice |
| `crates/cognicode-core/src/domain/aggregates/symbol.rs` | **None** | `is_type_definition()` consumed as-is |

### Approaches

1. **Extend existing `ImpactAnalyzer` domain service with `CallGraphProjection` delegate** — Keep `ImpactAnalyzer` in-place, add a `with_projection()` constructor or accept `&CallGraphProjection` alongside `&CallGraph`. New methods (`impact_radius`, `has_path_to`, `shortest_path`) delegate to the projection. Existing `calculate_impact()` keeps working with bare `CallGraph`.
   - Pros: Minimal new files (1 file modified). Backward-compatible — existing `ImpactAnalyzer` callers unchanged. Domain-layer naming continuity.
   - Cons: Domain service depending on infrastructure (`CallGraphProjection`) violates Clean Architecture layering. `ImpactAnalyzer` becomes a "coordinator" that knows about both layers. Existing 362 LOC grows to ~500+.
   - Effort: Low

2. **New `ImpactAnalysisService` application-layer service** — Create `crates/cognicode-core/src/application/services/impact_analysis.rs` (or similar) as a dedicated application service. It coordinates `CallGraph` (domain) and `CallGraphProjection` (infrastructure), producing result DTOs. The existing `ImpactAnalyzer` domain service remains untouched.
   - Pros: Clean architecture. Application layer is the correct place for coordination. Existing `ImpactAnalyzer` stays backward-compatible. New service has clear single responsibility (projection-aware impact queries). Future MCP endpoints call this service via `Arc<ImpactAnalysisService>`.
   - Cons: New file + new module. Application layer doesn't currently exist in `cognicode-core` — would establish a new pattern. Requires module wiring (`mod application; service;` in `lib.rs`).
   - Effort: Low-Medium

3. **Put `ImpactAnalysisService` alongside `CallGraphProjection` in `infrastructure/graph`** — Add impact query methods directly to `CallGraphProjection` or create `impact_analysis.rs` in `infrastructure/graph/`.
   - Pros: Colocated with the projection it uses. No new layer.
   - Cons: Infrastructure is the wrong place for service-like orchestration. `CallGraphProjection` is already 666 LOC — adding more surface to it creates a god-struct. Blurs the infrastructure/domain boundary.
   - Effort: Low

### Recommendation

**Approach 2 — New `ImpactAnalysisService` as an application-layer service.**

Rationale:
- Clean Architecture: application coordinates domain + infrastructure. The impact service is the archetypal "application use case" — it takes a query from the outside, builds a `CallGraphProjection` from domain data, runs algorithms, and returns answer DTOs.
- The existing `ImpactAnalyzer` is a valid domain service for simple counting-based analysis. It should NOT be modified — it serves a different purpose (quick safety checks with zero petgraph dependency).
- A future `CycleDetector` refactoring can also consume `CallGraphProjection` from the application layer, following the same pattern.
- Future MCP/UI slices wire into `ImpactAnalysisService` as a natural boundary — the application service returns DTOs, the interface layer serializes them.

**Service API (smallest useful surface):**

```rust
pub struct ImpactAnalysisService;

impl ImpactAnalysisService {
    /// Build a projection from any CallGraph and run impact queries
    pub fn analyze(&self, graph: &CallGraph) -> ImpactQuery {
        ImpactQuery { projection: CallGraphProjection::from_call_graph(graph) }
    }
}

pub struct ImpactQuery {
    projection: CallGraphProjection,
}

impl ImpactQuery {
    /// "What breaks if I change X?" — predecessors within max_depth
    pub fn impact_radius(&self, root: &SymbolId, max_depth: usize) -> Vec<SymbolId>;
    
    /// "Is A reachable from B?"
    pub fn has_path(&self, from: &SymbolId, to: &SymbolId) -> bool;
    
    /// "What's the confidence-weighted path from A to B?"
    pub fn shortest_path(&self, from: &SymbolId, to: &SymbolId) -> Option<(Vec<SymbolId>, f64)>;
    
    /// "Does the graph have cycles involving this symbol's SCC?"
    pub fn detect_cycles(&self) -> bool;
    
    /// "What SCC contains this symbol?"
    pub fn containing_component(&self, id: &SymbolId) -> Option<Vec<SymbolId>>;
}
```

### Entropy Analysis (Connascence Landscape)

**Method**: Heuristic (qualitative, no CogniCode for this explore phase)

| Component A | Component B | Connascence Type | I(bits) | Severity |
|-------------|-------------|------------------|---------|----------|
| `ImpactAnalysisService` (new) | `CallGraphProjection` | Type (projection struct + all algo return types) | 1.5 | ⚠️ Medium |
| `ImpactAnalysisService` (new) | `CallGraph` (aggregate) | Name (`SymbolId`, `CallGraph::from_call_graph`) | 0.5 | ✅ OK |
| `ImpactAnalysisService` (new) | `ImpactAnalyzer` (existing) | Meaning (both answer "impact" but with different methods) | 0.3 | ✅ OK |

**Critical Pairs (I > 3.0 bits)**: None
**Hidden Connascence (Meaning/Timing)**: None
**SOLID-Entropy Violations**: None — pure extension

**Coupling Score**: ~0.5 bits (single projection dependency)
**Recommendation**: Accept — clean architectural boundary, minimal coupling surface.

### Entropy Budget Prediction

| Metric | Estimate (bits) | Threshold | Status |
|--------|-----------------|-----------|--------|
| H(Δ_existing) | 0.0 | < 1.0 | ✅ Pure extension |
| H(Δ_new) | 1.2 | > 0 | ✅ |
| New connascence pairs introduced | 1 (ImpactService→CallGraphProjection) | < 3 | ✅ |
| OCP compliant? | Yes | yes | ✅ |

**Verdict**: 🟢 Green — no existing code touched. New application service consumes existing infrastructure and domain aggregates via stable public APIs.

### Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `CallGraphProjection::find_impact_radius` returns predecessor-only (reverse BFS) — no forward impact ("what does X affect?") | Medium | Design the service to name this clearly: `impact_radius` = predecessors, `forward_reach` = successors (future). Document the direction semantics unambiguously. |
| Application layer doesn't exist yet in `cognicode-core` — establishes new architectural pattern | Low | Precedent: `interface` layer already exists. Application is the natural next layer. Name it `application/services/` following the existing `domain/services/` convention. |
| `dijkstra` cost = `1.0 - confidence` means NaN/inf produce cost 0.0 (effectively free edges) | Low | `sanitize_confidence` already normalizes: NaN/inf → 1.0 → cost 0.0. This is the documented behavior — high-trust edges are cheap. Accept as-is for this slice. |
| `find_impact_radius` is bounded by `max_depth` — no unbounded variant | Medium | Provide a sentinel value (`usize::MAX`) for unbounded, or add an `impact_radius_unbounded()` wrapper. Document max_depth semantics in service docs. |

### Follow-On Slices Now Unblocked (after this service)

1. **MCP impact analysis tool** — Expose `impact_radius`, `has_path`, `shortest_path` as MCP tools via `mcp-postgres-envelope` pattern
2. **Cycle detection refactoring** — Replace `CycleDetector`'s ad-hoc Tarjan with `CallGraphProjection::strongly_connected_components()` 
3. **Forward impact radius** — `reachable_from(root, max_depth)` using forward BFS/DFS
4. **Multi-root impact analysis** — "what breaks if I change these 3 symbols together?"
5. **Confidence-weighted impact scoring** — Aggregate confidence along all paths to produce a single impact score
6. **Explorer UI impact view** — Graph visualization of impact radius with confidence-colored edges

### Answers to Explore Questions

**Q1: Where should an impact analysis service live?**
→ **Application layer** (`crates/cognicode-core/src/application/services/`). Domain service `ImpactAnalyzer` is for simple count-based analysis. Application service coordinates `CallGraphProjection` (infrastructure) with `CallGraph` (domain).

**Q2: What impact questions are already supported by `CallGraphProjection`?**
→ `find_impact_radius` (reverse predecessors), `has_path` (reachability), `dijkstra` (confidence-weighted path), SCC, cycles, topological sort, connected components. Missing: forward reachability, unbounded impact radius, multi-root analysis, impact scoring/aggregation.

**Q3: What does existing code call "impact", "dependency", "affected"?**
→ `ImpactAnalyzer` uses `direct_dependents`, `transitive_dependents`, `impact_level`, `affected_files`. `CallGraphProjection` uses `find_impact_radius`. `DependencyType` and `DependencyRepository` are established value objects/traits. `find_all_dependents`/`find_all_dependencies` exist on `CallGraph`.

**Q4: Smallest useful service API?**
→ `ImpactAnalysisService` with `analyze(graph) → ImpactQuery`. Methods: `impact_radius(id, depth)`, `has_path(from, to)`, `shortest_path(from, to)`, `detect_cycles()`, `containing_component(id)`. ~5 methods, ~150 LOC implementation + tests.

**Q5: Should confidence/provenance influence impact scoring in this slice?**
→ **No.** `find_impact_radius` is unweighted BFS — it treats all edges equally. Confidence already influences `dijkstra` (cost = `1.0 - sanitize(confidence)`), so the `shortest_path` method naturally surfaces confidence-weighted results. Scoring/averaging is a later slice.

**Q6: How should results be represented?**
→ **`Vec<SymbolId>` for reachability/composition queries, `Option<(Vec<SymbolId>, f64)>` for path queries, `bool` for existence checks.** No `Symbol` lookup in this slice — callers resolve symbols via `Projection::resolve_symbol()` or `CallGraph::get_symbol()` if they need full records. This keeps the service DTO-free and testable with `CallGraph::new()`.

**Q7: What tests would prove the service without PostgreSQL/live DB?**
→ Build `CallGraph` in-memory with `CallGraph::new()` + `add_symbol` + `add_dependency_with_provenance`. Construct `CallGraphProjection::from_call_graph(&g)`. Test: impact radius on chain/star/DAG, has_path on disconnected, shortest_path on parallel edges, detect_cycles on cycle/non-cycle, SCC partitioning. 0 external dependencies needed — `CallGraph` already used in unit tests.

**Q8: How does this relate to future MCP/UI slices?**
→ The service is the computational boundary. MCP tools → call `ImpactAnalysisService::analyze()` → serialize DTO to JSON. UI → call service → render graph. No MCP or UI code in this slice — just the service and its tests.

### Ready for Proposal

**Yes.** The exploration confirms:
- A clean architectural home exists (application layer)
- `CallGraphProjection` provides all needed primitives
- No existing code needs modification (pure extension)
- Tests can be written without PostgreSQL
- The slice unblocks MCP and UI slices that follow

The orchestrator should launch `sdd-propose` for `impact-analysis-service` with the recommendation: new `ImpactAnalysisService` application-layer service consuming `CallGraphProjection`.
