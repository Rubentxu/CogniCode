# Cycle e91 — Graph Insights Performance (carry-forward of e90)

## Status

**Type**: feature (performance fix)
**Date**: 2026-09-25 (created), 2026-09-26 (W1 closed)
**Author**: jcode-orchestrator (G0.3 revalidation); W1 fix: jcode-orchestrator (post-C8 cleanup)
**Status**: W1 CLOSED, W2-W5 PENDING
**Predecessor**: `2026-09-21-e90-g5-cold-cache-or-perf-fix/`

## Addendum 2026-09-26 (e91.W1 closed)

The W1 unit as originally scoped in the proposal below has been
**reinterpreted**: instead of profiling, W1 became "honest
metadata" — the previous implementation in
`CommunityDetector::detect_from_projection` hardcoded
`iterations = max_iterations.min(100)` and `converged = true`,
exposing fake values via the MCP `graph_communities` handler
(`graph_handlers.rs:310-311`). The fix ships in commit `6f40a08b`
and is documented in JOURNAL §11. Key changes:

1. `cognicode_graph_algos::communities` now returns
   `(Vec<Vec<usize>>, CommunitiesMeta)` carrying the real iteration
   count and a `converged` flag (true only when no label changed in
   the final iteration).
2. `CommunityDetector::detect_from_projection` propagates the real
   meta into `CommunityResult`.
3. Two pre-existing tests were pinning the bug (asserted
   `converged=true` on graphs that oscillate); they are updated to
   pin the honest behaviour.
4. Two new tests pin the contract (`test_detect_reports_real_iterations_chain`,
   `test_detect_reports_non_convergence_on_oscillating_2cycle`).
5. WASM shim (`cognicode-graph-wasm`) destructures the new tuple,
   discarding the meta (it never exposed it to the browser).

### Important correction to the original problem statement

The original addendum (e90 → 2026-09-25) stated that
`graph_insights`/`graph_communities` "do not exist in v0.98.1".
**This statement is obsolete for the current `main` HEAD**
(see JOURNAL §11). The tools are registered in
`crates/cognicode-explorer/src/mcp/explorer.rs:128-129`,
implemented in
`crates/cognicode-core/src/infrastructure/graph/analytics/community_detector.rs`
+ `application/services/graph_insights.rs`, and exposed via the
handler at
`crates/cognicode-core/src/interface/mcp/handlers/graph_handlers.rs:290,519`.

This means the perf regression described in the proposal (p95=367s
on a multi-repo Tier-2/3 fixture) **does apply to `main` HEAD** —
not to a hypothetical v1.0.0-rc. The e91 work remains valid; the
fix path is the same (algorithmic optimization on the same hot
paths identified below).

## Addendum 2026-09-26 (b) — Second commit ships the fix to the real MCP path

Commit `6f40a08b` fixed the algorithm API (`CommunitiesMeta`) and
propagated it into `CommunityResult`, but only ONE of the two
MCP handlers for `TOOL_GRAPH_COMMUNITIES` was emitting the new
fields. The handler described in the addendum above
(`cognicode-core/src/interface/mcp/handlers/graph_handlers.rs:310-311`)
is registered through `cognicode-core`'s own tool registry; the
bin produced by `cognicode-runtime` (the `explorer-mcp` binary,
which is what end-users actually run) registers a parallel
handler in
`crates/cognicode-explorer/src/mcp/handler/graph_analyze.rs:382`
that emitted only `{"communities": [...]}`, dropping the new
metadata before it reached MCP clients.

Commit `42a1ddcf` corrects the parallel handler to emit
`algorithm`, `max_iterations`, `iterations_used`, `converged`,
`community_count` alongside `communities`, so both MCP paths
now expose the honest execution state. Three new integration
tests in `crates/cognicode-explorer/tests/graph_analyze_integration.rs`
pin the contract:

  * `graph_communities_reports_real_iterations_used`
  * `graph_communities_oscillating_2cycle_reports_non_convergence`
  * `graph_communities_convergent_3cycle_reports_convergence`

All six `graph_communities` tests now pass (28/28 in the
`graph_analyze_integration` suite). See JOURNAL §11 Addendum
2026-09-26 for the discovery narrative and lesson (54).

With both commits in place, W1 (honest metadata) is fully
closed at the user-visible level. W2+ (profiling real
fixtures + algorithmic optimization) remain open per the
original WU2..WU5 plans below.

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

1. `CommunityDetector::detect(graph, 100)` — Label Propagation with
   up to 100 iterations on the full call graph. (Note: the
   original proposal said "Louvain-like modularity maximisation" —
   that is **incorrect** for the current `main` code, which uses
   Label Propagation via `cognicode-graph-algos::communities`.)
2. `CommunityDetector::surprising_connections(graph, &community_result, 20)` —
   cross-community edge enumeration, O(n²) in dense multi-repo graphs.

## Out of scope

- v0.98.x maintenance: this is **feature work**, not a patch. It
  belongs to the next minor/major release of the v1.0.0 line, not to
  `docs/roadmap/MAINTENANCE.md`.
- Re-running the full scorecard: deferred until WU3 (regression test).

## Work units (inherited from e90 proposal §"Recommended next cycle")

| WU | Description | Status |
|---|---|---|
| **WU1** | Profile `graph_insights` on a captured multi-repo fixture (`zod_realrepo_graph_insights`) to identify the dominant cost. | **REINTERPRETED** (commit `6f40a08b`): instead of profiling, W1 became "honest metadata" — fix the fake `iterations`/`converged` exposed to MCP clients. The profiling step is still needed before W2 and is rolled into the W2 entry condition. |
| **WU2** | Implement the lowest-risk algorithmic optimization (likely Option A sub-step 1: bounded iteration count + early termination on modularity delta in `CommunityDetector::detect`). | **PENDING**. Requires WU1 profiling artifact. |
| **WU3** | Add a regression test `graph_insights_multi_repo_under_budget` that asserts a known Tier-2 fixture completes within a budget (target: 30s p95; current: 367s). | PENDING |
| **WU4** | Re-run the scorecard; expect G5 GREEN. | PENDING |
| **WU5** | Document the budget choice in `openspec/specs/cognicode-analytics/spec.md`. | PENDING |

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
  addendum dated 2026-09-25; **addendum partially obsolete**, see addendum 2026-09-26 above).
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
