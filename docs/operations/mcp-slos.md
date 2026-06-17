# CogniCode MCP Server — SLO Definitions

**Audience**: SREs, on-call engineers, and operators configuring
Prometheus alerts and Grafana dashboards for the
`cognicode-mcp-server` process.

**Scope**: service-level objectives for the 60-tool MCP HTTP/SSE
surface, derived from the per-tool `cognicode_meta` annotations
(M2.3) and the operational hardening work in ADR-034 §7.

**Related**: see [`mcp-server-runbook.md`](./mcp-server-runbook.md)
for deployment, env vars, and troubleshooting.

---

## 1. SLI inventory

The MCP server exposes the following SLI histograms / counters in
`/metrics` (Prometheus text format). All values are in **seconds** or
unit-less counts.

| Metric | Type | What it measures |
|--------|------|-------------------|
| `cognicode_tool_duration_seconds` | Histogram | Wall-clock duration of every `tools/call` dispatch, labelled by `tool` and `category` (M1.1). **Source of truth for latency SLOs.** |
| `cognicode_tool_calls_total` | Counter | Total number of tool calls, labelled by `tool`, `category`, and `status` (`ok\|stub\|gated\|error\|missing\|skip`). |
| `cognicode_tool_errors_total` | Counter | Subset of calls that returned an error, labelled by `tool`, `category`, and `error_type` (`error\|timeout\|rate_limit_exceeded\|missing`). |
| `cognicode_graph_loaded` | Gauge | 1 once the active graph is loaded; 0 otherwise. |

The source-of-truth histogram for p99 latency alerts is
`cognicode_tool_duration_seconds_bucket`. All p99 formulas below
use:

```promql
histogram_quantile(0.99, sum by (le, category) (rate(cognicode_tool_duration_seconds_bucket[5m])))
```

## 2. Latency SLOs — per category

Targets are derived from a combination of the per-tool
`cognicode_meta.estimated_latency_ms` ranges (M2.3) and the
per-category timeouts enforced by the dispatch boundary (M3.2). The
p99 target is the operator-observable budget; the timeout is the
hard cap beyond which the call is force-cancelled and returned as
`error_type=timeout`.

| Category | p99 latency target | Dispatch timeout (M3.2) | Source budget |
|----------|--------------------|--------------------------|----------------|
| `search`     | ≤ 500 ms | 500 ms | M3.2 timeout table; ADR-034 §7 |
| `file`       | ≤ 200 ms | 30 s   | estimated_latency_ms range (M2.3) |
| `navigation` | ≤ 1 s    | 45 s   | estimated_latency_ms range (M2.3) |
| `quality`    | ≤ 1 s    | 30 s   | estimated_latency_ms range (M2.3) |
| `refactor`   | ≤ 1 s    | 30 s   | estimated_latency_ms range (M2.3) |
| `graph` (query)  | ≤ 1 s    | 60 s   | estimated_latency_ms range (M2.3); tighter per-tool for `graph_query`, `graph_neighborhood` |
| `graph` (analytics) | ≤ 5 s | 60 s   | ADR-034 §7; covers `build_graph`, `graph_pagerank`, `graph_communities` |
| `composite`  | ≤ 2 s    | 30 s   | estimated_latency_ms range (M2.3) |
| `aix`        | ≤ 10 s   | 30 s   | estimated_latency_ms range (M2.3) |

> **Why two rows for `graph`?** The `graph` category mixes cheap
> structural queries (`graph_query`, `graph_neighborhood` — sub-second
> in normal operation) with expensive analytics (`build_graph`,
> `graph_pagerank`, `graph_communities` — multi-second on large
> graphs). Operators writing alerts should pick the column that
> matches the call mix: if the workload is mostly interactive
> queries, alert at 1s; if it is mostly analytics batch jobs, alert
> at 5s. Both share the same 60s hard timeout.

### 2.1 Per-category p99 alert formulas

Each formula uses a 5-minute rate window. Adjust `[5m]` to `[10m]`
or `[15m]` if the alert would be too noisy on low-volume categories
(`refactor`, `quality`).

```promql
# search — alert at 500ms
histogram_quantile(0.99, sum by (le) (rate(cognicode_tool_duration_seconds_bucket{category="search"}[5m]))) > 0.5

# file — alert at 200ms
histogram_quantile(0.99, sum by (le) (rate(cognicode_tool_duration_seconds_bucket{category="file"}[5m]))) > 0.2

# navigation — alert at 1s
histogram_quantile(0.99, sum by (le) (rate(cognicode_tool_duration_seconds_bucket{category="navigation"}[5m]))) > 1.0

# quality — alert at 1s
histogram_quantile(0.99, sum by (le) (rate(cognicode_tool_duration_seconds_bucket{category="quality"}[5m]))) > 1.0

# refactor — alert at 1s
histogram_quantile(0.99, sum by (le) (rate(cognicode_tool_duration_seconds_bucket{category="refactor"}[5m]))) > 1.0

# graph (interactive queries) — alert at 1s
histogram_quantile(0.99, sum by (le) (rate(cognicode_tool_duration_seconds_bucket{category="graph"}[5m]))) > 1.0

# graph (analytics) — alert at 5s
histogram_quantile(0.99, sum by (le) (rate(cognicode_tool_duration_seconds_bucket{category="graph"}[5m]))) > 5.0

# composite — alert at 2s
histogram_quantile(0.99, sum by (le) (rate(cognicode_tool_duration_seconds_bucket{category="composite"}[5m]))) > 2.0

# aix — alert at 10s
histogram_quantile(0.99, sum by (le) (rate(cognicode_tool_duration_seconds_bucket{category="aix"}[5m]))) > 10.0
```

## 3. Error-rate SLOs

The error rate is the ratio of `cognicode_tool_errors_total` to
`cognicode_tool_calls_total` over a 5-minute window. We distinguish
between **stable** tools (production-grade, low error budget) and
**experimental** tools (gated or stub implementations, higher
error budget).

| Tool class | Error-rate target | Definition |
|------------|-------------------|------------|
| **stable**       | < 1% of calls in any 5-minute window | `cognicode_meta.stability = "stable"` — the tool is production-grade and contract-locked. |
| **experimental** | < 5% of calls in any 5-minute window | `cognicode_meta.stability ∈ {"experimental", "gated"}` — the tool may be incomplete, rate-limited, or only available behind a feature flag. |
| **overall**      | < 1% of calls in any 5-minute window | Computed over all stable + experimental calls. Use this as the headline SLO. |

### 3.1 Error-rate alert formulas

```promql
# Stable tool error rate
sum(rate(cognicode_tool_errors_total{tool=~".+"}[5m]))      # adjust the label filter
/
sum(rate(cognicode_tool_calls_total{tool=~".+"}[5m]))       # to match the actual stability label once it is wired in M2.11
> 0.01

# Overall (all-tool) error rate
sum(rate(cognicode_tool_errors_total[5m]))
/
sum(rate(cognicode_tool_calls_total[5m]))
> 0.01
```

> **Note on label wiring**: as of M2.11, the stability classification
> is recorded in the `tool_call` structured log line but not yet
> mirrored as a Prometheus label. Once the label is wired (a small
> M3.7 follow-up), the alerts above should be tightened to filter
> by `stability="stable"`. Until then, alert on the overall ratio
> and treat elevated experimental errors as informational, not
> paging.

## 4. Availability SLO

**Target**: **99.5% of `/ready` checks return `200 OK` in any
calendar month.**

This corresponds to roughly 3.6 hours of "not ready" time per month.
In practice this is dominated by cold-start `build_graph` latency
after a restart (a few seconds to a few minutes depending on
workspace size and PG vs. standalone mode).

### 4.1 Availability alert formula

```promql
# Blackbox-style: probe /ready every 30s, alert if 3 consecutive failures
# (assumes blackbox_exporter is configured against /ready on :9847)

probe_success{instance="cognicode-mcp:9847", probe="ready"} == 0
```

Or, using the in-process gauge:

```promql
# /ready is failing — i.e. graph not loaded — for more than 5 minutes
avg_over_time(cognicode_graph_loaded[5m]) < 0.5
```

## 5. Per-tool override mechanism (planned)

The M3.2 timeout table is hard-coded in
`crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs`. The SLO
table above is the operator-observable target, not the dispatch
timeout. If a particular tool's p99 consistently exceeds the SLO
target, the operator can:

1. **First**: alert the team and investigate root cause (storage,
   workspace size, etc.). The p99 target is not a hard cap.
2. **Then**: file a request to widen the per-tool timeout in the
   dispatch boundary. The override mechanism
   (`COGNICODE_MCP_TIMEOUT_OVERRIDE` env var, reserved but
   unimplemented) is a planned follow-up tracked in
   `docs/sdd-kernel/M3-Sprint-spec.md` §Open Questions.

## 6. SLO review cadence

This SLO table is reviewed:

- **At every M-sprint archive** — confirm the per-category
  `estimated_latency_ms` ranges still match reality, update the
  p99 targets if the workspace profile has shifted.
- **At every release that adds or refactors tools** — update the
  SLO table to cover any new category.
- **Quarterly** — independent of the release cadence, review the
  error budget and availability target with the platform team.

## 7. Cross-references

- [`mcp-server-runbook.md`](./mcp-server-runbook.md) — env vars,
  metrics scraping, troubleshooting.
- `docs/sdd-kernel/M3-Sprint-spec.md` §M3.7 — source spec
  (capability `slo-definitions`).
- `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` — the
  `timeout_for_category` table that backs the dispatch timeouts in
  §2.
- ADR-034 §7 — operational hardening rationale.
