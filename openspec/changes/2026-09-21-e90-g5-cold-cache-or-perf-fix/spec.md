# Spec — Cycle e90 (investigation only)

## Requirements

### REQ-G5-INVESTIGATION-01 — Investigate G5 scorecard RED state

The cycle SHALL document the root cause of the G5 Latency Budget RED
verdict that blocks the E31-G scorecard streak (Gate 2 of the v1.0.0
pre-cut checklist).

**Verified**: G5 reports `analytics: p95=367072ms > budget 5000ms`.
Root cause is `graph_insights` and `graph_communities` being slow in
multi-repo Tier-2/3 scenarios (zod, commander). Single-repo and Tier-1
scenarios pass.

### REQ-G5-INVESTIGATION-02 — Categorise the failure mode

The cycle SHALL classify whether the issue is a scorecard calibration
problem or a product performance problem.

**Verified**: The failure is a **product performance problem**.
`tool_call_ms` is the scorecard measurement and the outlier is the
tool's wall-time inside the container, not container startup. Applying
a G6-style cold-cache filter would mask a real regression.

### REQ-G5-INVESTIGATION-03 — Identify the responsible algorithms

The cycle SHALL identify the specific algorithms in the product that
dominate the latency.

**Verified**:
1. `CommunityDetector::detect(graph, 100)` in
   `crates/cognicode-core/src/application/services/graph_insights.rs:120`
   — Louvain-like modularity maximisation with up to 100 iterations.
2. `CommunityDetector::surprising_connections(graph, &community_result, 20)`
   in the same file at line 145 — O(n²) cross-community edge enumeration.

### REQ-G5-INVESTIGATION-04 — Propose candidate solutions

The cycle SHALL enumerate 3-4 candidate solutions with their trade-offs,
including at least one that does NOT touch the scorecard calibration.

**Verified**: Option A (algorithmic), Option B (caching), Option C
(decomposition), Option D (NOT recommended: scorecard calibration).

### REQ-G5-INVESTIGATION-05 — Recommend a follow-up cycle

The cycle SHALL recommend the next cycle (e91) with a work-unit
breakdown that targets the lowest-risk algorithmic fix.

**Verified**: e91-graph-insights-performance with WU1..WU5 covering
profiling, optimization, regression test, scorecard re-run, spec sync.

## Scenarios

### S-G5-01 — Outlier traceable to algorithm

**Given** a multi-repo `graph_insights` invocation that exceeds 60s
**When** the scorecard reports G5 RED
**Then** the outlier SHALL be traceable to one of the algorithms in
`GraphInsightsService::analyze` (REQ-G5-INVESTIGATION-03).

### S-G5-02 — Scorecard measurement is honest

**Given** any scorecard run
**When** the scorecard reports a budget violation
**Then** the measurement SHALL be `tool_call_ms` from the result.json
(no wall-clock artifact, no cold-cache contamination of the budget gate).

### S-G5-03 — Carry-forward documented

**Given** the investigation completes
**When** the cycle is archived
**Then** the next cycle (e91) SHALL be named in the cross-references
and its work units SHALL be defined.

## Acceptance status

All requirements and scenarios are **VERIFIED** by diagnostic commands
captured in `proposal.md`. The cycle produces no code change; it is
the investigation record.
