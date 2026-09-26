# Verify Report — Cycle e90 (investigation only)

## Type

INVESTIGATION ONLY. No code change produced.

## Method

Read-only inspection of:
1. `sandbox/results/scorecard_run.json` (freshly generated via
   `just scorecard-streak` on 2026-09-21T07:22:23Z).
2. `sandbox/results/full_run/*/*/result.json` (multi-repo Tier-2/3
   timing measurements).
3. `crates/cognicode-core/src/application/services/graph_insights.rs`
   (responsible algorithm).
4. `sandbox/scripts/release_scorecard.py:gate_g5` (measurement contract).

## Findings

### F1 — G5 RED verdict (verbatim)

```json
{
  "id": "G5",
  "name": "Latency Budget by Tool Family",
  "status": "RED",
  "measured": "analytics: p95=367072ms > budget 5000ms",
  "budget": null,
  "evidence_path": "sandbox/results/ci_smoke,sandbox/results/quality,sandbox/results/full_run"
}
```

### F2 — Outlier table (tool_call_ms, top 3 per tool)

| tool | total_ms | tool_call_ms |
|---|---|---|
| graph_insights | 741783 | 367072 |
| graph_insights | (1 sample) | 1280 (sanity single-repo) |
| graph_communities | 423979 | 53094 |
| graph_communities | (1 sample) | 1430 (sanity) |
| graph_pagerank | 361029 | 836 |
| graph_pagerank | (1 sample) | 1449 |
| graph_query | 369765 | 1 (single-repo) |
| graph_query | (1 sample) | 789 (single-repo) |
| graph_all_paths | (1 sample) | 1 |

### F3 — Scorecard streak state

```json
{
  "current_streak": 0,
  "goal": 3,
  "last_run_at": "2026-09-21T07:22:23",
  "status": "RESET"
}
```

### F4 — Algorithm trace

`graph_insights` invocations flow through
`crates/cognicode-core/src/interface/mcp/handlers/graph_handlers.rs:
handle_graph_insights` which delegates to
`crates/cognicode-core/src/application/services/graph_insights.rs:
GraphInsightsService::analyze(graph)` (line 92).

`analyze()` runs in order:
- L105 `GraphAnalyticsService::god_nodes(graph, 0.95)` — bounded p95
- L109 `projection.strongly_connected_components()` — O(V+E)
- L116 `GraphAnalyticsService::feedback_arc_set(graph)` — NP-hard, but
  called once with `take(10)`
- L120 `CommunityDetector::detect(graph, 100)` — Louvain up to 100 iterations
- L145 `CommunityDetector::surprising_connections(graph, &community_result, 20)`
  — O(n²) cross-community edges

The two `CommunityDetector` calls dominate wall-time on multi-repo
graphs and are the responsible algorithms.

## Verdict

**VERIFIED — product performance issue, not scorecard calibration.**
A G6-style cold-cache filter would hide the regression; it is
explicitly NOT the right fix.

## Status

```text
investigation  PASS
scorecard      unchanged (RED)
recommendation e91 (next cycle)
```
