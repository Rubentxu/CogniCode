# Proposal: Risk Map View — Quality Data Wiring (e12g-risk-map)

## Intent
CogniCode catalogues `ViewKind::RiskMap` but ships no executor. The existing `HotspotsExecutor` only ranks scope symbols by `fan_in` (no quality data, no graph, no composite score). Users cannot visualize where quality issues cluster with high call-graph centrality. This change wires `QualityRepository` into a new `RiskMapExecutor` that emits a graph-shaped, risk-scored view — reusing the `HotspotsLens` formula rather than reinventing it.

## Scope

### In Scope
- `QualityGraphRepository` adapter — projects `QualityIssue` rows into `GraphNode`/`GraphEdge` pairs for the view's graph data source
- `RiskMapExecutor` implementing `ViewExecutor` for `ViewKind::RiskMap` (graph + table composite)
- Shared `compute_risk(fan_in, weighted_issues) -> f32` extracted from `HotspotsLens` (DRY)
- REST endpoint `GET /api/quality/hotspots` (MCP-facing, optional)

### Out of Scope
- RiskMap UI rendering in Explorer (Phase 3)
- Multi-workspace risk aggregation (Phase 2)
- Real-time risk monitoring / PG NOTIFY subscription (Phase 3)
- Modifying `HotspotsExecutor` or `QualityExecutor` (unchanged)

## Capabilities

### New Capabilities
- `risk-map`: Build a graph-shaped, risk-scored view combining call-graph centrality with clustered quality issues (graph + table composite, `RendererKind::Composite`).

### Modified Capabilities
- None. `HotspotsLens` requirements are unchanged; only its formula is extracted to a shared pure function.

## Approach
1. `QualityGraphRepository` wraps `&dyn QualityRepository` and projects each `QualityIssue` into a `GraphNode` (kind `Issue`) plus a `GraphEdge` (file/symbol → issue, kind `Resolves`/`Cites`). Read-only, same pattern as `PostgresQualityRepository`.
2. Extract `compute_risk()` from `hotspots.rs` into a shared module; both `HotspotsLens` and `RiskMapExecutor` call it.
3. `RiskMapExecutor::build(ctx)` composes `GraphQueryPort` (centrality) + `QualityGraphRepository` (issues) → `ContextualView { view_id: "risk_map", blocks: [risk_graph, hotspots_table] }`. Static instance `RISK_MAP_EXECUTOR` registered in `registry.rs::get_executor()`.
4. Empty `QualityRepository` degrades to graph-only view (no panic, matches port contract).

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/adapters/quality_graph_repository.rs` | New | QualityRepository → graph projection adapter |
| `crates/cognicode-explorer/src/domain/views.rs` | Modified | Add `RiskMapExecutor` + `build_risk_map()` + static |
| `crates/cognicode-explorer/src/domain/lenses/hotspots.rs` | Modified | Extract shared `compute_risk()` |
| `crates/cognicode-explorer/src/registry.rs` | Modified | Wire `RISK_MAP_EXECUTOR` into `get_executor()` |
| `crates/cognicode-explorer/src/api.rs` | Modified | Optional `/api/quality/hotspots` handler |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Risk formula drift between lens and executor | Medium | Shared `compute_risk()`; property test both consumers |
| Projection mismatch (issue → node → issue) | Low | Round-trip property test |
| Performance on >10k issues | Medium | Cap at `FINDING_CAP = 20`; paginate REST |

## Rollback Plan
Revert the 5 affected files. No DB migration — read-only over existing `issues` table (`m0011_quality.sql`). No data loss; no schema change.

## Dependencies
- Existing `QualityRepository` port, `GraphQueryPort`, `HotspotsLens` formula
- ADR-002 (moldable exploration parity), ADR-004 (C4 investigation model)

## Success Criteria
- [ ] `RiskMapExecutor` produces `ContextualView` with `view_id = "risk_map"`, `view_kind = RiskMap`
- [ ] Risk scores match `HotspotsLens` on identical input (property test)
- [ ] `GET /api/quality/hotspots` returns top-20 ranked entries
- [ ] Empty `QualityRepository` degrades gracefully (graph-only view, no panic)
- [ ] `cargo test -p cognicode-explorer` green; no regressions in `HotspotsExecutor` tests
