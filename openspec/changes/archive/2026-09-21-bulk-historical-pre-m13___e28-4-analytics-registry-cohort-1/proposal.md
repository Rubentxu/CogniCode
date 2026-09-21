# Proposal: E28.4 — Analytics Registry Cohort 1

> Change: `e28-4-analytics-registry-cohort-1` · Branch: `feat/e28-4-analytics-registry-cohort-1` · Depends: `e28-2-runtime-closure` (production executor wiring), E28.2 PR4 `executor-equivalence-conformance` (shipped v0.71.1), and `e28-2-pr5-edge-filter` (shipped v0.71.0; separate from runtime closure)

## Intent
Pure algorithms already exist in `cognicode-graph-algos` (`page_rank`, `condensation`, `cluster_components`) but lack admission, governance, lineage, and common result modes. Per ADR-014 §5–§7, admitted analytics must be versioned, reproducible, bounded, and never mutate canonical facts. Cohort 1 stabilizes PageRank, SCC, WCC, and bounded shortest paths behind a descriptor-driven registry.

## Scope

### In Scope
- `AlgorithmDescriptor` admission: identity, version, maturity, determinism/seed, directed/weighted/heterogeneous traits, projection assumptions, params, output schema, supported modes, complexity/limits, truncation, conformance fixtures
- Registry admission gate (reject incomplete descriptors; cohort-gated)
- Modes: `stream`, `stats`, `annotate`, authorized `persist` (idempotent derived record, no canonical mutation)
- Reproducible run lineage records (queryable)
- Stabilize cohort 1: PageRank, SCC, WCC, bounded shortest paths
- REST / MCP / Explorer surfaces (user-facing per ADR-014 §9)

### Out of Scope
- Cohort 2 (dominators, articulation points, bridges, k-core) → E28.5
- Cohort 3 (betweenness, k-shortest, multi-source reachability, PPR) → E28.6
- Cohort 4 (Leiden, conductance/modularity, similarity) → future
- Production Neo4j (CI oracle only; sidecar needs separate ADR)

## Capabilities
> CONTRACT with sddk-spec. Research `openspec/specs/` first.

### New Capabilities
- `graph-analytics-registry`: descriptor-driven admission + `AlgorithmRegistry` + modes + cohort-1 algorithms + projection contract
- `graph-analytics-run-lineage`: reproducible, queryable run records (workspace, revision, plan hash, versions, params, seed, status, truncation)

### Modified Capabilities
- `plan-limits`: per-descriptor complexity/limits for analytics runs; lift "Persistent limit policies (E28.4+)" exclusion
- `executor-semantics`: algorithm-specific numeric tolerance defaults realized (lift "E28.4+" exclusions); analytics reuse typed-value/truncation/error envelope

## Approach
Admit existing pure functions under descriptors; isolate algorithms from the registry via ports (DIP). A run lowers to a pinned workspace+revision (E28.0) and the descriptor's assumed projection. `persist` writes an idempotent derived-analysis record via a separately authorized command; derived relations still route through `RelationCandidate`. Backend choice stays internal.

## Entropy Budget
- Keep pure algorithms decoupled (low connascence — acceptable `Connascence of Algorithm`).
- Registry depends on ports, not impls (DIP); cap projection-seam coupling.
- **Known source**: `CallGraphProjection` adjacency-orientation defect — fix pre-admission or descriptors declare the corrected projection.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-graph-algos/src/algorithms/` | Modified | descriptors + bounded shortest paths; keep pure |
| `crates/cognicode-core/src/application/services/graph_analytics.rs` | Modified | registry + lineage wiring |
| `crates/cognicode-core/src/domain/plan/` | Modified | descriptor/lineage types |
| `crates/cognicode-core/src/infrastructure/persistence/m00XX_analytics_lineage.sql` | New | Minimal PG migration: analytics lineage + descriptor-limit storage tables (additive) |
| `crates/cognicode-mcp/`, REST, Explorer | New | surfaces for cohort 1 |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Projection orientation defect invalidates results | High | require `e28-2-pr5-edge-filter`; projection contract in descriptor |
| `persist` leak to canonical write | Med | execution-boundary guard; idempotent record only |
| Tolerance divergence PG vs snapshot | Med | golden fixtures + `assert_approx_equal` |

## Rollback Plan
Mostly additive: this change DOES ship a minimal PostgreSQL migration to back the durable analytics lineage records (run records), descriptor limit policies, and derived-analysis idempotency keys required by the `graph-analytics-run-lineage` and `plan-limits` ADDED requirements. The migration is additive (new tables/columns for lineage + descriptor limit storage), companion down-migration drops them. No E28.0–E28.2 contract break and no `graph_plan.rs` change. Single `git revert` of the feature-branch merge plus the companion down-migration; algorithms stay unreached until runtime wires the registry. No canonical graph data is affected — the migration only adds analytics-storage tables.

## Dependencies
- `e28-2-runtime-closure` — production wiring of `GraphExecutor` + per-hop limits + canonical `PlanHash`. The edge-filter fix is NOT part of this closure.
- `executor-equivalence-conformance` — shipped E28.2 PR4 capability supplying the `assert_equivalent`/`assert_approx_equal` differential harness against which cohort-1 conformance fixtures run.
- `e28-2-pr5-edge-filter` — shipped v0.71.0. This separate follow-up distinguishes `calls` from other `DependencyType` values in traversal and is not part of runtime closure.
- E28.0 `SnapshotProvider`/`RevisionId`, E28.1 `PlanLimits`/`executor-semantics`
- ADR-014 §5–§7; `graph-analytics-execution.md`

## Success Criteria
- [ ] Every admitted algorithm is versioned, resource-governed, reproducible, non-mutating
- [ ] Incomplete descriptors are rejected; cohort-2 requests return not-admitted
- [ ] Cohort-1 conformance + composition tests pass (PG vs snapshot golden fixtures)
- [ ] REST/MCP/Explorer expose user-facing cohort-1 algorithms with happy/empty/error/truncation interaction tests
