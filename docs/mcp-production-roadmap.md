# Roadmap: MCP Production Readiness — Observability, Integrity Gates, and Tool Lifecycle

> **ADR**: [ADR-034](../adr/ADR-034-mcp-production-readiness.md)
> **Baseline**: ADR-033 Phase 0 — 64 tools, 0 duplicates, 0 missing, smoke passing
> **Goal**: Every tool in `tools/list` is honest, observable, and operationally ready

---

## Current State (Post-ADR-033)

| Metric | Value |
|--------|-------|
| Listed tools | 64 |
| Unique tools | 64 |
| Duplicates | 0 |
| Missing (ghost) | 0 |
| OK (real response) | 53 |
| STUB (placeholder) | 6 |
| GATED_OK (correctly gated) | 3 |
| SKIP (destructive/needs artifact) | 4 |
| Tools emitting OTel metrics | 5 of 64 (8%) |
| `record_tool_usage()` callsites | 3 (all no-op) |
| `/metrics` endpoint | none |

---

## Sprint M1 — Universal Instrumentation

**Goal:** Every tool call flows through a single instrumentation boundary.
We can see calls, duration, errors, and status for all 64 tools.

### Tasks

| ID | Task | Files | Est |
|----|------|-------|-----|
| M1.1 | Centralize dispatch: wrap `call_tool_handler` with instrumentation boundary | `rmcp_adapter.rs` | 4h |
| M1.2 | Implement `classify_status()` — classify result as ok/stub/gated/error/missing | `rmcp_adapter.rs` | 2h |
| M1.3 | Delete `HandlerContext::record_tool_usage()` no-op + all 3 callsites | `handlers/mod.rs`, `rmcp_adapter.rs` | 1h |
| M1.4 | Fix `ToolMetrics::noop()` to return real no-op via OTel no-op MeterProvider | `telemetry/mod.rs` | 1h |
| M1.5 | Remove scattered `Instant::now()` timing from handlers that duplicate the central boundary | `aix_handlers.rs`, `handlers/mod.rs` | 3h |
| M1.6 | Add `cognicode.tool.calls` Counter with `tool` + `status` labels | `telemetry/mod.rs` | 1h |
| M1.7 | Verify: run smoke with OTLP collector → confirm 64 tools emit calls + duration | manual | 2h |

**Deliverable:** `curl` to any tool produces a metric increment. Dashboard can
show calls/s, latency p50/p99, and error rate per tool.

**Definition of done:** Smoke test with `OTEL_EXPORTER_OTLP_ENDPOINT` set
shows 64 tools with `calls > 0` in the collector.

---

## Sprint M2 — Health KPIs and Integrity Gates

**Goal:** We can measure production health quantitatively. No public STUB tools.
CI blocks regressions.

### Tasks

| ID | Task | Files | Est |
|----|------|-------|-----|
| M2.1 | Add `cognicode.graph.symbols`, `cognicode.graph.edges`, `cognicode.graph.health_score` as Gauges updated after `build_graph` | `telemetry/mod.rs`, `handlers/mod.rs` | 2h |
| M2.2 | Add `/metrics` endpoint (Prometheus text format) to MCP HTTP/SSE server | `cognicode-mcp/src/server.rs` | 3h |
| M2.3 | Add tool `annotations` metadata to `Tool::new()` registrations (category, stability, requires_graph, destructive) | `rmcp_adapter.rs` | 4h |
| M2.4 | Gate `smart_search`: remove from `build_all_tools()` until backend implemented | `rmcp_adapter.rs` | 30m |
| M2.5 | Gate `compare_graph`: remove from `build_all_tools()` until PG read path | `rmcp_adapter.rs` | 30m |
| M2.6 | Gate `iac_query`: remove from `build_all_tools()` until IaC query wired | `rmcp_adapter.rs` | 30m |
| M2.7 | Gate `nl_to_symbol`: remove from `build_all_tools()` until NLP pipeline | `rmcp_adapter.rs` | 30m |
| M2.8 | Fix `project_insights`: wire to `GraphInsightsService::analyze()` for real communities, god nodes, hot paths | `consolidated_handlers.rs` | 3h |
| M2.9 | Fix `project_overview`: wire to `smart_overview()` instead of hardcoded `85.0` | `consolidated_handlers.rs` | 2h |
| M2.10 | Document gated tools policy: `graph_diff`, `graph_timeline`, `generate_contract` are `stability: gated` with `requires_persistence: true` | `rmcp_adapter.rs` annotations | 1h |
| M2.11 | Update smoke test: read `annotations.stability` from `tools/list`; fail on `STUB` from `stable` tools | `scripts/mcp/mcp_smoke_all.py` | 2h |
| M2.12 | Add smoke as CI gate in GitHub Actions / CI config | `.github/workflows/` or equivalent | 2h |

**Deliverable:** `tools/list` returns only honest tools. `/metrics` scrapes
Prometheus-compatible metrics. CI fails if a stable tool regresses to STUB.

**Definition of done:**
- Smoke reports `listed_total=60` (4 gated removed, 2 fixed), `STUB=0`.
- `curl http://127.0.0.1:9847/metrics` returns Prometheus text with tool metrics.
- CI workflow includes smoke gate.

---

## Sprint M3 — Operational Hardening

**Goal:** The MCP server is safe to expose beyond localhost with timeouts,
rate limits, structured logging, and readiness checks.

### Tasks

| ID | Task | Files | Est |
|----|------|-------|-----|
| M3.1 | Separate `/health` (process alive) from `/ready` (graph loaded + queryable) | `cognicode-mcp/src/server.rs` | 2h |
| M3.2 | Per-tool timeout: configurable per category (default 30s, graph analytics 60s, LSP 45s) | `rmcp_adapter.rs` | 3h |
| M3.3 | Rate limit enforcement on expensive tools (graph analytics, LSP operations) — already exists in `security.rs`, wire to tool dispatch | `rmcp_adapter.rs`, `security.rs` | 2h |
| M3.4 | Structured log per tool call: `tracing::info!(tool, duration_ms, status, "tool_call")` at central boundary | `rmcp_adapter.rs` | 1h |
| M3.5 | Auth/token middleware for non-localhost deployments (Bearer token or API key) | `cognicode-mcp/src/server.rs` | 4h |
| M3.6 | Document deployment runbook: env vars, health checks, metrics scraping, log format | `docs/operations/mcp-server-runbook.md` | 2h |
| M3.7 | SLO definitions per tool category (search < 500ms p99, graph analytics < 5s p99, LSP < 30s p99) | `docs/operations/mcp-slos.md` | 1h |

**Deliverable:** Server can be deployed behind a reverse proxy with auth,
scraped by Prometheus, and monitored with SLO-based alerting.

**Definition of done:**
- `/ready` returns 503 until graph is loaded.
- Timeouts enforced per tool category.
- Bearer token required when `COGNICODE_MCP_AUTH_TOKEN` is set.
- Runbook covers deployment, scaling, and troubleshooting.

---

## After M3: Feature Completion (Parallel)

Once the server is production-ready, the gated tools can be implemented in
parallel without blocking operability:

| Tool | Dependency | Effort |
|------|------------|--------|
| `smart_search` | Delegate to `semantic_search` + `ranked_symbols` + `graph_search_idf` | 2d |
| `compare_graph` | PG `graph_reports` read path (ADR-022 graph_reports table) | 3d |
| `iac_query` | IaC extraction query layer (ADR-024 nodes queryable) | 2d |
| `nl_to_symbol` | NLP keyword extraction → symbol match pipeline | 3d |

---

## ADRs Referenced

- [ADR-029](../adr/ADR-029-mcp-http-sse-server.md) — HTTP/SSE transport
- [ADR-030](../adr/ADR-030-fix-mcp-graph-store-fallback.md) — GraphStore fallback fix
- [ADR-031](../adr/ADR-031-linear-pagerank.md) — Linear PageRank
- [ADR-032](../adr/ADR-032-graphstore-graphcache-convergence.md) — GraphStore convergence
- [ADR-033](../adr/ADR-033-mcp-tool-surface-integrity.md) — Tool surface integrity Phase 0
- [ADR-034](../adr/ADR-034-mcp-production-readiness.md) — This roadmap's ADR
