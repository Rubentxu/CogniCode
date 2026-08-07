# Proposal: E28.5 — Structural Analytics Cohort 2

> Change: `e28-5-structural-analytics-cohort-2` · Depends: `e28-4-analytics-registry-cohort-1` (shipped v0.73.x)

## Intent
E28.4 shipped a descriptor-driven `AlgorithmRegistry` admitting only cohort-1 algorithms (PageRank, SCC, WCC, bounded shortest paths). Critically, `.admit()` is wired only in tests — production builds hold an **empty registry**, so even cohort-1 algorithms are unreachable. E28.5 ships cohort-2 structural analytics (dominators, articulation points, bridges, k-core) **and** closes the production admit gap so all admitted algorithms become reachable.

## Scope

### In Scope
- 4 pure functions in `cognicode-graph-algos` (dominators, articulation points, bridges, k-core)
- 4 descriptor types + 4 new `RunOutput` variants
- `build_undirected_neighbors()` helper on `CallGraphProjection` (DRY for articulation/bridges/k-core)
- `default_analytics_registry()` composition root admitting cohort-1 + cohort-2
- Conformance fixtures per algorithm

### Out of Scope
- Explorer UI entry points (deferred to E28.6+)
- Cohort 3 (betweenness, k-shortest, PPR) and Cohort 4 (Leiden, modularity)

## Capabilities
> CONTRACT with sddk-spec. Researched `openspec/specs/` — `graph-analytics-execution` does NOT exist; RunOutput lives in the registry's descriptor domain (`descriptor.rs`).

### New Capabilities
- None

### Modified Capabilities
- `graph-analytics-registry`: broaden the admitted catalog to cohort-2 algorithms; add cohort-2 `RunOutput` variants; require a production composition root that admits all cohort algorithms

## Approach
Layer-split, 2 PRs. **PR1**: 4 pure functions + RunOutput variants (correctness focus — DFS edge cases, k-core peeling, dominator root). **PR2**: 4 descriptors + registry admit + composition-root fix + conformance fixtures (descriptor-completeness focus). Dominators introduces a 4th descriptor flavor: directed + root-parametrized; root resolution happens in `execute()`, not param `validate()`. Undirected algorithms reuse a shared `build_undirected_neighbors()`.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `cognicode-graph-algos/src/algorithms/{dominators,articulation_points,bridges,k_core}.rs` | New | Pure functions; petgraph wraps dominators, custom DFS for rest |
| `cognicode-core/src/domain/analytics/descriptor.rs` | Modified | RunOutput + 4 variants + row_count/to_json arms |
| `cognicode-core/src/domain/analytics/*_descriptor.rs` | New | 4 descriptors |
| `cognicode-core/src/infrastructure/graph/call_graph_projection.rs` | Modified | `build_undirected_neighbors()` helper |
| `cognicode-core/src/application/services/graph_analytics.rs` | Modified | `default_analytics_registry()` composition root |
| `cognicode-explorer/src/mcp/explorer.rs` | Modified | Wire non-empty registry |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Dominators root not in graph | Med | Resolve+validate in `execute()`, structured error |
| DFS output non-determinism | Med | Sort results by node id before serialization |
| petgraph lacks articulation/bridges/k-core | High | Implement from Tarjan/lowlink/peeling |
| Undirected adjacency correctness | Med | Test self-loops, multi-edges, isolated nodes |

## Rollback Plan
Pure additive — no migration, no canonical-graph change. Revert PR2 (descriptors/admit) then PR1 (functions); registry returns to prior state. Algorithms become unreachable; no data affected.

## Dependencies
- `e28-4-analytics-registry-cohort-1` — `AlgorithmRegistry` + cohort-1 descriptors (shipped)
- `TEST_DATABASE_URL` for conformance fixtures

## Success Criteria
- [ ] 4 algorithms admitted and executable via `analytics_run` (REST + MCP)
- [ ] `GET /api/analytics/catalog` lists all 8 algorithms
- [ ] Production composition root admits algorithms (no empty registry)
- [ ] Conformance fixtures prove deterministic, sorted output
- [ ] Full workspace regression: 0 failures
