# M2 Sprint Archive Report — ADR-034: MCP Production Readiness

**Change**: M2 Sprint (ADR-034 MCP Production Readiness — M2.1–M2.11)
**Verdict**: PASS
**Date**: 2026-06-17

---

## Scope

Full implementation and verification of M2 Sprint tasks for ADR-034 MCP Production Readiness.

---

## Per-Task Status

| Task | Description | Status |
|------|-------------|--------|
| M2.1 | 3 OTel Gauges (graph_symbols, graph_edges, graph_health_score) added to ToolMetrics, recorded after build_graph | PASS |
| M2.2 | /metrics Prometheus endpoint added to cognicode-mcp server | PASS |
| M2.3 | 60 tools annotated with cognicode_meta (stability, category, requires_graph, requires_persistence, estimated_latency_ms) | PASS |
| M2.4 | STUB tool `smart_search` gated — removed from build_all_tools() | PASS |
| M2.5 | STUB tool `compare_graph` gated — removed from build_all_tools() | PASS |
| M2.6 | STUB tool `iac_query` gated — removed from build_all_tools() | PASS |
| M2.7 | STUB tool `nl_to_symbol` gated — removed from build_all_tools() | PASS |
| M2.8 | project_insights wired to GraphInsightsService::analyze() | PASS |
| M2.9 | project_overview wired to smart_overview() (no more hardcoded 85.0) | PASS |
| M2.10 | 3 gated tools annotated (graph_diff, graph_timeline, generate_contract) | PASS |
| M2.11 | Smoke test reads meta.cognicode.stability for integrity gate | PASS |
| M2.12 | CI gate — **DEFERRED to M3** (no CI provider decided yet) | DEFERRED |

---

## Final State

| Metric | Value |
|--------|-------|
| Tools listed | 64 → 60 |
| Public STUB tools | 6 → 0 |
| Public GATED tools | 3 (graph_diff, graph_timeline, generate_contract) |
| Tools with cognicode_meta annotations | 60 (100% of surface) |
| New OTel metrics | cognicode.graph.{symbols, edges, health_score} (Gauges) |
| New endpoint | GET /metrics (Prometheus text format) |
| Release binary | 109MB |

---

## ToolMetrics Schema (Stable)

```
cognicode.tool.calls{tool, status}    — Counter
cognicode.tool.duration{tool, status} — Histogram
cognicode.tool.errors{tool, status}   — Counter
cognicode.graph.symbols               — Gauge
cognicode.graph.edges                 — Gauge
cognicode.graph.health_score          — Gauge
```

---

## Tool Categorization (Final)

- **STUB_TOOLS**: empty (all 4 STUBs gated/removed)
- **GATED_TOOLS**: [graph_diff, graph_timeline, generate_contract]

---

## Commits

| Commit | Description |
|--------|-------------|
| bd74a4f | M2.4–2.7 gate 4 STUBs + M2.10 annotate 3 gated |
| cda4b22 | M2.3 annotate 60 tools with cognicode_meta |
| ed3bdb8 | M2.1 graph Gauges + M2.2 /metrics endpoint |
| 4fa2b9c | M2.8 project_insights wired to GraphInsightsService |
| 2be10c7 | M2.9 project_overview wired to smart_overview |
| 9c50d73 | M2.11 smoke reads annotations for integrity gate |

**Files changed**: rmcp_adapter.rs, telemetry/mod.rs, status.rs, consolidated_handlers.rs, mcp_smoke_all.py, server.rs, Cargo.toml (×2)

**Tests**: 7 status:: + 33 mcp_roundtrip — all passing

---

## Open Items

- **M2.12**: CI gate deferred to M3 Sprint (no CI provider decided yet)

---

## Knowledge Updates

1. ToolMetrics schema is now stable with both Counters (calls, duration, errors) and Gauges (graph symbols/edges/health)
2. All CogniCode MCP tools have cognicode_meta annotations — 100% surface coverage
3. STUB_TOOLS is now empty; GATED_TOOLS contains 3 tools
