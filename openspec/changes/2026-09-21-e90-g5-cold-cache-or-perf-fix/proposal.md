# Cycle e90 — Investigate G5 Latency Budget RED in v1.0.0 scorecard

## Status

**Type**: investigation + diagnostic (no implementation)
**Date**: 2026-09-21
**Author**: jcode-orchestrator
**Status**: INVESTIGATION COMPLETE — IMPLEMENTATION DEFERRED

## Problem

G5 ("Latency Budget by Tool Family") of `sandbox/scripts/release_scorecard.py`
currently reports **RED** with `analytics: p95=367072ms > budget 5000ms`,
blocking the E31-G scorecard streak (Gate 2 of the v1.0.0 pre-cut checklist).

Affected tools (multi-repo scenarios, `sandbox/results/full_run/`):
- `graph_insights`:  tool_call_ms p95 = **367072ms** (worst)
- `graph_communities`: tool_call_ms p95 = **53094ms**
- `graph_pagerank`: tool_call_ms p95 = 836ms (sanity)
- `graph_query`: tool_call_ms p95 = 789ms (sanity)
- `graph_all_paths`: tool_call_ms p95 = 1ms (sanity)

The outliers are concentrated on `multi_realrepo_*` Tier-2/3 scenarios over
zod / commander / click repositories. Single-repo and Tier-1 scenarios pass
within budget.

## Root cause analysis (verified 2026-09-21)

The scorecard **correctly reports** the budget violation. The slowness is
**real** and concentrated in two algorithms of the `GraphInsightsService`
(`crates/cognicode-core/src/application/services/graph_insights.rs`):

1. `CommunityDetector::detect(graph, 100)` — Louvain-like modularity maximisation
   called from `analyze()` (line 120). O(n log n) per pass, but with up to 100
   iterations on the full call graph.
2. `CommunityDetector::surprising_connections(graph, &community_result, 20)` —
   cross-community edge enumeration (line 145). O(n²) in the number of
   cross-community edges, quadratic in dense multi-repo graphs.

These run on every `graph_insights` invocation. In Tier-2/3 multi-repo graphs
(zod, commander) with tens of thousands of symbols and dense cross-module
edges, both calls dominate wall-time.

`graph_pagerank` (836ms p95), `graph_query` (789ms p95), and `graph_all_paths`
(1ms p95) are within budget because they operate on-demand or via the
projection port, not by re-running the full insight pipeline.

This is therefore **a product performance issue**, not a scorecard calibration
issue. Applying a G6-style cold-cache filter would mask a real regression and
is **not the right fix**.

## Diagnostic evidence (commands reproducible)

```bash
# 1. Scorecard run (current state)
just scorecard-streak
# -> streak 0/3 (RESET) because last run was RED (G:9 A:2 R:2)

# 2. Inspect the analytics p95 outliers
python3 -c "
import json, glob
per_tool = {}
for rj in glob.glob('sandbox/results/full_run/*/*/result.json'):
    r = json.load(open(rj))
    t = r.get('timing_ms', {})
    tool = r.get('tool')
    if tool and t.get('total_ms'):
        per_tool.setdefault(tool, []).append({'tc': t.get('tool_call_ms'), 'tt': t.get('total_ms')})
for tool in ['graph_insights','graph_communities','graph_pagerank','graph_query','graph_all_paths']:
    if tool in per_tool:
        for it in sorted(per_tool[tool], key=lambda x: x['tt'], reverse=True)[:3]:
            print(f'{tool}: total={it[\"tt\"]:.0f}ms tool_call={it[\"tc\"]}ms')
"

# 3. Locate the algorithms
grep -n "fn analyze\|CommunityDetector::detect\|surprising_connections" \\
    crates/cognicode-core/src/application/services/graph_insights.rs
```

## Candidate solutions (NOT applied; research required)

### Option A — Algorithmic optimization
- Profile `CommunityDetector::detect` on a representative multi-repo fixture
  to confirm where the 367s is spent (modularity computation vs label
  propagation vs merge).
- Consider bounded iteration count + early termination on
  modularity-improvement delta.
- Replace cross-community edge enumeration with adjacency-list pre-indexing
  (current implementation is O(n²) per cross-edge check).

Pros: real improvement, scales linearly with repo size.
Cons: requires profiling infrastructure + a real Tier-2/3 multi-repo fixture.
Significant implementation work — likely a multi-cycle investigation.

### Option B — Caching layer
- Memoize `GraphInsightsService::analyze(graph_id)` on the projection
  content-hash; invalidate when the graph is rebuilt.
- Add an `InsightCache` port, register in the composition root.

Pros: O(1) repeat-call latency; consistent with the existing ReadSet
infrastructure (per e65).
Cons: introduces new state; requires invalidation contract.

### Option C — Decompose insights into sub-tools
- Split `graph_insights` into `graph_insights_summary`, `graph_insights_communities`,
  `graph_insights_god_nodes`, `graph_insights_cycles`, `graph_insights_surprising`.
- Each sub-tool computes only its slice; clients compose as needed.

Pros: caller-controlled cost; sub-tools can each be budgeted independently.
Cons: breaks API contract; pre-existing callers need migration.

### Option D — Scorecard calibration (NOT recommended)
- Apply a per-tool warm-cache filter (G6-style) to G5 as well.
- Pros: trivial implementation.
- Cons: hides a real product performance regression; violates the "evidence
  over confidence" principle; would require ADR amendment.

## Recommended next cycle (e91)

Open **e91-graph-insights-performance** as a multi-cycle investigation:

WU1 — Profile `graph_insights` on a captured multi-repo fixture
       (`zod_realrepo_graph_insights`) to identify the dominant cost.
WU2 — Implement the lowest-risk algorithmic optimization (likely Option A,
       sub-step 1: bounded iteration count in `CommunityDetector::detect`).
WU3 — Add a regression test (`graph_insights_multi_repo_under_budget`) that
       asserts a known Tier-2 fixture completes within 30s p95.
WU4 — Re-run the scorecard; expect G5 GREEN.
WU5 — Document the budget choice in `openspec/specs/cognicode-analytics/spec.md`.

## Carry-forward

- e90 itself produces no code change; it is the investigation record.
- e91 must be a fresh cycle with its own proposal / spec / tasks / apply.
- The scorecard streak (Gate 2) remains blocked at 0/3 until G5 is GREEN.
- Gate 1 (T7 5-night stability cadence) is **independent** of this issue and
  can still progress.
- The scorecard reads `tool_call_ms` correctly; the G5 measurement contract
  is not at fault.

## Cross-references

- E30-metric-baseline: original G1-G12 scorecard definition
- E31-E2: ACCEPT decision on `retrieve_and_verify` CV 0.105 (parallel pattern)
- ADR-031 §3: scorecard streak rule (3 ALL-GREEN before tag cut)
- ADR-031 §4: denominator renegotiation (parallel for pct_verified)
- E31-G: scorecard streak counter implementation
- `sandbox/scripts/release_scorecard.py`: gate_g5 implementation
- `crates/cognicode-core/src/application/services/graph_insights.rs`:86:
  `GraphInsightsService::analyze`
- `crates/cognicode-graph-algos/src/algorithms/communities.rs`:37:
  `pub fn communities`
