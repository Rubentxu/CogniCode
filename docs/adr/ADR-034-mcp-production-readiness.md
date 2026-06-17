# ADR-034: MCP Production Readiness — Observability, Integrity Gates, and Tool Lifecycle

**Status:** Accepted
**Date:** 2026-06-16
**Source:** ADR-033 Phase 0 follow-up + observability audit + production-readiness gap analysis

## Context

ADR-033 Phase 0 closed the tool-surface integrity gap (no duplicates, no ghost
tools, list↔dispatch parity). However, production-ready MCP requires more than
structural integrity. This ADR addresses three remaining gaps identified
through code evidence and live smoke testing.

### Gap 1: Six public STUB tools

The live smoke (`scripts/mcp/mcp_smoke_all.py`) classified 6 tools as STUB —
they appear in `tools/list` with schemas but return placeholder, empty-by-
construction, or note-only output:

| Tool | Smoke output summary | Root cause |
|------|---------------------|------------|
| `smart_search` | `results: [], total: 0` | No backend; returns empty vec |
| `compare_graph` | `"note": "Baseline comparison requires..."` | No PG graph_reports read path |
| `iac_query` | `resource_type: "unknown"` | IaC extraction not wired to query |
| `nl_to_symbol` | `total_candidates: 0` | NLP pipeline not implemented |
| `project_insights` | Incomplete metrics, no hot paths | Uses graph but returns partial data |
| `project_overview` | `architecture_score: null` | Hardcoded placeholder, no real analysis |

A client calling any of these gets a syntactically valid response that is
semantically empty. This is worse than a 404 because it creates a false
expectation of capability.

### Gap 2: Three gated tools with unclear policy

| Tool | Gate | Current behavior |
|------|------|-----------------|
| `graph_diff` | Requires `PostgresRepository` | Returns `internal:` error with message |
| `graph_timeline` | Requires `PostgresRepository` | Returns `internal:` error with message |
| `generate_contract` | Requires persistence layer | Returns `internal:` error with message |

These tools respond correctly when gated (they explain why they cannot run),
but there is no published policy: are they stable, experimental, or
conditionally available?

### Gap 3: Observability coverage at 8%

The codebase has a full OpenTelemetry `ToolMetrics` infrastructure
(`infrastructure/telemetry/mod.rs`) with 9 instruments, an `instrument_tool()`
wrapper, and an OTLP exporter configured in the MCP binary. However:

| Component | Reality | Evidence |
|-----------|---------|----------|
| Tools emitting OTel metrics | **5 of 64** | Only `file_ops_handlers.rs` calls `instrument_tool()` |
| `HandlerContext::record_tool_usage()` | **No-op** | `handlers/mod.rs:525-534`: "PostgreSQL adapter will land" |
| `ToolMetrics::noop()` | **Panics** | `telemetry/mod.rs:84`: `panic!("not intended for direct use")` |
| `/metrics` endpoint | **Does not exist** | Only `/health` is exposed |
| Tool status KPIs | **Missing** | No Counter/label for `OK/STUB/GATED/ERROR` |
| Graph stats as metrics | **Missing** | `symbols_found`, `edges`, `health_score` are JSON-only |

Without observability coverage, we cannot measure production health, set SLOs,
or alert on degradation. The infrastructure exists but is connected to less
than 8% of the surface.

## Decision

### 1. No public STUB tools — implement or gate

**Rule:** Any tool that appears in `tools/list` must either:
- Return real, non-placeholder data for its documented contract, OR
- Be hidden from `tools/list` until its backend is complete, OR
- Return a structured `GATED` response with an accionable reason and `retry_after` hint.

**Action for the 6 current STUBs:**

| Tool | Decision |
|------|----------|
| `smart_search` | **Gate**: hide until backend delegates to `semantic_search` + `ranked_symbols` + `graph_search_idf` |
| `compare_graph` | **Gate**: hide until PG `graph_reports` read path is implemented |
| `iac_query` | **Gate**: hide until IaC extraction nodes are queryable |
| `nl_to_symbol` | **Gate**: hide until NLP keyword-to-symbol pipeline is implemented |
| `project_insights` | **Fix**: wire to real `GraphInsightsService::analyze()` output |
| `project_overview` | **Fix**: wire to real `smart_overview()` instead of hardcoded `85.0` |

### 2. Tool stability metadata

Every tool registration gains an `annotations` block (MCP 2025-06 spec) with:

```json
{
  "annotations": {
    "title": "Human-readable name",
    "category": "graph|search|refactor|quality|file|composite",
    "stability": "stable|experimental|gated",
    "requires_graph": true,
    "requires_persistence": false,
    "destructive": false,
    "estimated_latency_ms": 200
  }
}
```

This metadata:
- Lets clients filter tools by capability needs.
- Lets the CI smoke test classify expected behavior.
- Lets the gateway make routing decisions (e.g., cache, rate-limit expensive tools).

### 3. Centralized instrumented dispatch

**Problem today:** `call_tool_handler()` in `rmcp_adapter.rs` is a 1,700-line
match statement where each arm manually calls `ctx.record_tool_usage()` (which
is a no-op) or nothing at all.

**Decision:** Introduce a single instrumentation boundary at the top of
`call_tool_handler()`:

```rust
// Pseudocode — actual implementation in Sprint M1
let start = Instant::now();
let result = dispatch_tool(ctx, name, arguments).await;
let duration_ms = start.elapsed().as_millis() as f64;
let status = classify_status(&result);  // OK, STUB, GATED, ERROR

if let Some(metrics) = get_global_metrics() {
    metrics.calls.add(1, &[
        KeyValue::new("tool", name),
        KeyValue::new("status", status),
    ]);
    metrics.duration.record(duration_ms, &[
        KeyValue::new("tool", name),
    ]);
    if status == "ERROR" {
        metrics.errors.add(1, &[
            KeyValue::new("tool", name),
        ]);
    }
}
```

This replaces per-handler ad-hoc timing (`let start = Instant::now()` scattered
across 60+ functions) with a single choke point. Every tool call — including
errors, gated responses, and missing tools — is captured automatically.

**Delete `HandlerContext::record_tool_usage()`**: the no-op is dead code once
the central boundary exists.

**Fix `ToolMetrics::noop()`**: return a real no-op (using OTel's no-op
MeterProvider) instead of panicking.

### 4. New KPIs

| Metric | Type | Labels | Purpose |
|--------|------|--------|---------|
| `cognicode.tool.calls` | Counter | `tool`, `status` | Total calls by tool and outcome |
| `cognicode.tool.duration` | Histogram | `tool` | Latency distribution per tool |
| `cognicode.tool.errors` | Counter | `tool`, `error_type` | Error rate per tool |
| `cognicode.graph.symbols` | Gauge (UpDown) | — | Active symbol count after `build_graph` |
| `cognicode.graph.edges` | Gauge (UpDown) | — | Active edge count |
| `cognicode.graph.health_score` | Gauge | — | Graph health score from `GraphInsightsService` |

`status` label values: `ok`, `stub`, `gated`, `error`, `missing`, `skip`.

### 5. `/metrics` endpoint

Add a Prometheus-text-format `/metrics` endpoint to the MCP HTTP/SSE server
alongside `/health`. This enables local scraping without an OTLP collector.

The endpoint reads from a shared metrics registry. When OTLP is also
configured, both export paths coexist (Prometheus for local scraping, OTLP for
remote collection).

### 6. Smoke test as CI gate

`scripts/mcp/mcp_smoke_all.py` becomes a CI gate:

- Build release binary.
- Start MCP server.
- Run `build_graph`.
- Run smoke on all listed tools.
- **Fail CI if:** any tool is `MISSING`, `ERROR`, or `STUB` without explicit
  `allow_stub` in the tool's metadata.

### 7. Operational hardening

| Capability | Detail |
|------------|--------|
| `/health` vs `/ready` | `/health` = process alive; `/ready` = graph loaded and queryable |
| Per-tool timeout | Configurable timeout per tool category (default 30s, graph analytics 60s) |
| Rate limiting | Per-client rate limit on expensive tools (already exists in `security.rs`) |
| Structured logs | Every tool call emits a `tracing::info!` with `tool`, `duration_ms`, `status` |

## Consequences

- **Tool surface shrinks temporarily**: the 4 gated STUBs disappear from
  `tools/list` until implemented. Clients see fewer but honest capabilities.
- **`project_insights` and `project_overview` gain real implementations**: they
  must wire to `GraphInsightsService` and `smart_overview()` respectively.
- **Dispatch refactor**: `call_tool_handler()` gains a wrapping boundary. The
  internal match arms stay the same; the instrumentation moves from scattered
  to centralized.
- **CI time increases**: smoke gate adds ~60s (build_graph + 64 tool calls).
- **Observability cost**: Prometheus scrape adds negligible overhead. OTLP
  export is already configured but inactive (no collector running in dev).

## Roadmap Reference

See [`docs/mcp-production-roadmap.md`](../mcp-production-roadmap.md) for the
three-sprint delivery plan (M1: Instrumentation, M2: Health & Gates, M3:
Operational Hardening).
