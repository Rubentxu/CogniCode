# Proposal: Impact Analysis Service

## Intent

`AnalysisService` (2299 LOC, `application/services/analysis_service.rs`) already orchestrates `ImpactAnalyzer` domain service — but only count-based analysis. `CallGraphProjection` (666 LOC, `infrastructure/graph/`) provides petgraph-native algorithms (SCC, Dijkstra, impact radius, path finding) that `AnalysisService` never touches. This gap means no application-level impact queries are confidence-weighted or graph-algorithm-aware. We need a dedicated service that bridges this.

## Scope

### In Scope
- New `ImpactAnalysisService` at `crates/cognicode-core/src/application/services/impact_analysis.rs`
- Coordinates `CallGraphProjection` (infrastructure) with `CallGraph` (domain aggregate)
- Methods: `impact_radius(root, max_depth)`, `has_path(from, to)`, `shortest_path(from, to)`, `detect_cycles()`, `containing_component(id)`
- Extend `application/dto/impact_dto.rs` with projection-aware DTOs (`PathResultDto`, `SccDto`)
- Register module in `application/services/mod.rs`
- Unit tests using in-memory `CallGraph::new()`, zero external dependencies

### Out of Scope
- No modification to existing `ImpactAnalyzer` domain service (stays count-based)
- No changes to `CallGraphProjection` (consumed read-only)
- No MCP/UI endpoints — service only
- No `CycleDetector` replacement (it's a separate refactoring slice)
- No PostgreSQL/DB changes
- No forward impact radius (`forward_reach` — future slice)
- No multi-root or confidence-weighted impact scoring

## Capabilities

### New Capabilities
- `impact-analysis-service`: Application-layer service that coordinates `CallGraphProjection` algorithms to answer graph-aware impact queries (radius, paths, cycles, components)

### Modified Capabilities
None — pure extension, zero existing spec modifications.

## Approach

New `ImpactAnalysisService` in `application/services/impact_analysis.rs`, following existing pattern from `analysis_service.rs` (struct, `new()`, delegates to domain/infrastructure):

```rust
pub struct ImpactAnalysisService;

impl ImpactAnalysisService {
    pub fn analyze(&self, graph: &CallGraph) -> ImpactQuery { ... }
}
pub struct ImpactQuery { projection: CallGraphProjection }
// 5 delegating methods on ImpactQuery
```

DTOs extended in `application/dto/impact_dto.rs`: `PathResultDto` (path + cumulative confidence), `SccDto` (component membership). All tests build `CallGraph` in-memory with `add_symbol` + `add_dependency_with_provenance`.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/application/services/impact_analysis.rs` | **New** | New service (~150 LOC) + tests (~200 LOC) |
| `src/application/services/mod.rs` | Modified | +1 line (`pub mod impact_analysis;`) |
| `src/application/dto/impact_dto.rs` | Modified | ~40 new LOC: `PathResultDto`, `SccDto` |
| `src/infrastructure/graph/call_graph_projection.rs` | **None** | Consumed read-only |
| `src/domain/services/impact_analyzer.rs` | **None** | Unchanged count-based service |
| `src/domain/services/cycle_detector.rs` | **None** | Separate refactoring slice |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `find_impact_radius` is predecessor-only (reverse BFS) — no forward impact | Medium | Name clearly: `impact_radius` = "what depends on X". Document direction semantics. Forward reach is future slice. |
| New service may overlap with `AnalysisService`'s existing impact queries | Low | `ImpactAnalysisService` is projection-aware only; `AnalysisService` delegates to `ImpactAnalyzer` (count-based). No overlap. |
| `dijkstra` cost = `1.0 - confidence`; NaN→0.0 cost (effectively free edges) | Low | `sanitize_confidence` normalizes NaN/inf→1.0. Documented behavior — accept as-is. |
| `max_depth` has no unbounded variant | Medium | Use `usize::MAX` sentinel; document semantics. |

## Rollback Plan

Remove `impact_analysis.rs`, revert 1-line addition in `services/mod.rs`, revert DTO additions. No other code depends on this. Git revert is trivial.

## Dependencies

- `CallGraphProjection` (existing, stable API)
- `CallGraph` aggregate (existing, stable API)
- `SymbolId` (existing)

## Success Criteria

- [ ] `ImpactAnalysisService` passes all unit tests with in-memory `CallGraph`
- [ ] 5 methods: `impact_radius`, `has_path`, `shortest_path`, `detect_cycles`, `containing_component`
- [ ] No modification to existing `ImpactAnalyzer` or `CallGraphProjection`
- [ ] `cargo test -p cognicode-core` passes (no regressions)
- [ ] `cargo clippy` clean

## Entropy Budget

**Method**: Heuristic (CogniCode unavailable)

| Metric | Estimate (bits) | Threshold | Status |
|--------|-----------------|-----------|--------|
| H(Δ_existing) | 0.0 | < 1.0 | ✅ Pure extension |
| H(Δ_new) | 1.58 | > 0 | ✅ |
| New connascence pairs | 2 (IS→Projection: 1.0, IS→CallGraph: 0.5) | < 3 | ✅ |
| OCP compliant? | Yes | yes | ✅ |

**Verdict**: 🟢 Green — no existing code touched. New service consumes existing infrastructure and domain aggregates via stable public APIs. Established application-layer service pattern already exists (`analysis_service.rs`), so this is following convention, not inventing it.
