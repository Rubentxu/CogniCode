# Cycle e91 — Graph Insights Performance (carry-forward of e90)

## Status

**Type**: feature (performance fix)
**Date**: 2026-09-25
**Author**: jcode-orchestrator (G0.3 revalidation)
**Status**: PROPOSAL — NOT STARTED
**Predecessor**: `2026-09-21-e90-g5-cold-cache-or-perf-fix/`

## Problem

The v1.0.0 scorecard G5 ("Latency Budget by Tool Family") is RED:

```
analytics: p95=367072ms > budget 5000ms
```

Worst offenders:
- `graph_insights`: p95 = 367072ms (6+ minutes!)
- `graph_communities`: p95 = 53094ms

This blocks Gate 2 of the v1.0.0 pre-cut checklist (scorecard streak 0/3).

The investigation in e90 identified two hot paths in
`crates/cognicode-core/src/application/services/graph_insights.rs`:

1. `CommunityDetector::detect(graph, 100)` — Louvain-like modularity
   maximisation with up to 100 iterations on the full call graph.
2. `CommunityDetector::surprising_connections(graph, &community_result, 20)` —
   cross-community edge enumeration, O(n²) in dense multi-repo graphs.

The release v0.98.1 (production-ready contractual, C7 firmado) does
**not contain** the `graph_insights` or `graph_communities` tools that
e90 measured — those tools belong to a separate **v1.0.0-rc** binary.
See e90 addendum `2026-09-25` for verification of the tool inventory
mismatch between v0.98.1 and the e90 scorecard run.

## Out of scope

- v0.98.x maintenance: this is **feature work**, not a patch. It
  belongs to the next minor/major release of the v1.0.0 line, not to
  `docs/roadmap/MAINTENANCE.md`.
- Re-running the full scorecard: deferred until WU3 (regression test).

## Work units (inherited from e90 proposal §"Recommended next cycle")

| WU | Description | Acceptance |
|---|---|---|
| **WU1** | Profile `graph_insights` on a captured multi-repo fixture (`zod_realrepo_graph_insights`) to identify the dominant cost. | Profiling artifact saved under `openspec/changes/2026-09-25-e91-graph-insights-performance/profiles/`; verdict on whether cost is `modularity compute` or `cross-community enumerate`. |
| **WU2** | Implement the lowest-risk algorithmic optimization (likely Option A sub-step 1: bounded iteration count + early termination on modularity delta in `CommunityDetector::detect`). | Code change with unit tests; semantic equivalence preserved (same community assignments on `fixture-petclinic-single-repo` fixture). |
| **WU3** | Add a regression test `graph_insights_multi_repo_under_budget` that asserts a known Tier-2 fixture completes within a budget (target: 30s p95; current: 367s). | Test fails before WU2, passes after. |
| **WU4** | Re-run the scorecard; expect G5 GREEN. | Scorecard run captured under `sandbox/results/scorecard_run.json` with timestamp after WU2 merge; G5 status = GREEN. |
| **WU5** | Document the budget choice in `openspec/specs/cognicode-analytics/spec.md`. | Spec updated with budget rationale; linked from e91 verify-report. |

## Candidate solutions (carried from e90)

### Option A — Algorithmic optimization (preferred)

- Bounded iteration count + early termination on modularity-improvement
  delta in `CommunityDetector::detect`.
- Replace cross-community edge enumeration with adjacency-list pre-indexing.

Pros: real improvement, scales linearly with repo size.
Cons: requires profiling infrastructure + a real Tier-2/3 multi-repo fixture.

### Option B — Caching layer

- Memoize `GraphInsightsService::analyze(graph_id)` on the projection
  content-hash; invalidate when the graph is rebuilt.
- Add an `InsightCache` port, register in the composition root.

Pros: O(1) repeat-call latency.
Cons: introduces new state; requires invalidation contract.

### Option C — Decompose insights into sub-tools

- Split `graph_insights` into `graph_insights_summary`,
  `graph_insights_communities`, `graph_insights_god_nodes`,
  `graph_insights_cycles`, `graph_insights_surprising`.
- Each sub-tool computes only its slice.

Pros: caller-controlled cost.
Cons: breaks API contract; pre-existing callers need migration.

### Option D — Scorecard calibration (REJECTED)

Apply a per-tool warm-cache filter to G5. Same arguments as e90 §Option D:
hides a real product performance regression; violates "evidence over
confidence" principle.

## Decision gate

After WU1 (profiling), choose between A/B/C based on where the cost is:

- If modularity computation dominates → A.
- If surprising_connections dominates → A sub-step 2.
- If repeat-call latency dominates → B.
- If single-call latency dominates AND decomposition is acceptable to
  clients → C.

## Carry-forward and dependencies

- Predecessor: `2026-09-21-e90-g5-cold-cache-or-perf-fix/` (closed with
  addendum dated 2026-09-25).
- Consumer of e91: v1.0.0 pre-cut checklist Gate 2.
- This cycle does NOT touch v0.98.x.

## How e91 closes

When ALL of the following hold:

1. WU1..WU5 all PASS with evidence.
2. `cargo test -p cognicode-core --lib` remains green (no regression).
3. The scorecard has G5 GREEN at least once (WU4 evidence).
4. A new release candidate is built (v1.0.0-rc or similar).
5. ADR-XXX published documenting the budget choice (per WU5).

Then e91 transitions to ACCEPTED, the change is archived, and the
scorecard streak can resume.
