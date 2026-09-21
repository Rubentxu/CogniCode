# Proposal: E28.6 — Advanced Analytics Evidence Gate

> Change: `e28-6-advanced-analytics-evidence-gate` · Depends: `e28-5-structural-analytics-cohort-2` (shipped). Implements ADR-014 §6 admission contract + §8 Neo4j oracle.

## Intent
E28.6 is the **evidence gate**, not a cohort repeat. The deliverable is a **gate-decision ledger** for 8 cohort 3+4 candidates, admitting only algorithms that are *both* low-cost and product-relevant — and *rejection* of commodity/derivable ones is itself a deliverable. It also activates the pre-written Neo4j CI parity oracle (opt-in, never production).

## Scope

### In Scope
- **Gate ledger** of admit/reject/defer per candidate, with rationale (→ candidate ADR)
- **Admit:** Personalized PageRank (separate descriptor, shared pure fn), Conductance, Modularity
- **Reject-as-compose:** k-shortest paths, multi-source reachability (documented helpers, not registry entries)
- **Defer-with-measurement-plan:** Betweenness, Leiden, Node similarity
- **Activate Neo4j CI oracle** — opt-in parity harness gated on `NEO4J_URI`; build stays green without it

### Out of Scope
- Implementing deferred algorithms (Betweenness/Leiden/Similarity)
- Production Neo4j dependency (CI-only)
- Explorer UI entry points for cohort-3 algorithms

## Capabilities
> CONTRACT with sddk-spec. Researched `openspec/specs/` — `graph-analytics-registry` exists; `graph-analytics-execution` exists only as `docs/specs/` (not an openspec capability).

### New Capabilities
- `advanced-analytics-evidence-gate`: admission-decision ledger + opt-in Neo4j CI parity oracle + evidence-gate criteria (measured + product-relevant admission rule)

### Modified Capabilities
- `graph-analytics-registry`: broaden admitted catalog to cohort-3 (Personalized PageRank + Conductance + Modularity); add cohort-3 `RunOutput` variants

## Approach
**Evidence-Gate-First.** The gate ledger is the primary artifact. Winners:
- **Personalized PageRank** → separate `personalized_pagerank@1.0.0` descriptor sharing the `page_rank` pure fn (adds optional personalization vector) — preserves `pagerank@1.0.0` backward compat.
- **Conductance/Modularity** → new metric module; `Stats`-mode companions that *feed* the Leiden gate decision (must land before Leiden can be evaluated).

Rejected candidates become thin composition helpers (`all_simple_paths` + sort = k-shortest; BFS union = multi-source reachability) per ADR-014 §6. Neo4j oracle reuses cohort-1 floating-point tolerance policy (1e-6 seeded / 1e-9 cost).

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `cognicode-graph-algos/src/algorithms/{conductance,modularity}.rs` | New | Pure metric fns over adjacency + labels |
| `cognicode-graph-algos/src/algorithms/page_rank.rs` | Modified | Add optional personalization vector param |
| `cognicode-graph-algos/src/algorithms/{k_shortest,multi_source_reachability}.rs` | New | Composition HELPERS (not admitted) |
| `cognicode-core/src/domain/analytics/{personalized_pagerank,conductance,modularity}_descriptor.rs` | New | 3 descriptors |
| `cognicode-core/src/domain/analytics/descriptor.rs` | Modified | `RunOutput` variants + arms |
| `cognicode-core/src/application/services/graph_analytics.rs` | Modified | `default_analytics_registry()` admits 11 |
| `ci/` (Neo4j parity harness) | New | Gated on `NEO4J_URI`; skip path green |
| `docs/adr/ADR-NNN-e28-6-admission-decisions.md` | New | Gate ledger |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| "Under-delivery" expectation (user expects 8 shipped) | Med | Frame E28.6 as gate; rejections prevent bloat |
| Conductance/modularity need community assignment input | Med | Param schema accepts `community_of: Vec<usize>` OR composes `communities()` internally |
| Neo4j parity flaky on float tolerance | Low | Reuse cohort-1 tolerance policy |

## Rollback Plan
Pure additive — no canonical-graph or migration change. Revert descriptors/admit (registry returns to 8); remove pure fns + oracle harness. No data affected. Helpers are leaf modules.

## Dependencies
- `e28-5-structural-analytics-cohort-2` — registry + cohort-2 descriptors (shipped)
- `NEO4J_URI` env (optional, CI-only)

## Success Criteria
- [ ] Gate ledger documents all 8 decisions with rationale (ADR drafted)
- [ ] Personalized PageRank + Conductance + Modularity admitted & executable via `analytics_run`
- [ ] `pagerank@1.0.0` backward compat preserved (no version bump)
- [ ] Neo4j parity harness: green WITHOUT Neo4j configured; records agreement when set
- [ ] Rejected candidates exist only as composition helpers (not in catalog)
