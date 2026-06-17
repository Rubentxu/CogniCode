# SDD Kernel Archive Report: bug-prometheus-027

## Change Metadata
- **Change name**: bug-prometheus-027
- **Verdict**: PASS
- **Commit**: 57f455b — fix(mcp): cognicode-mcp-server Prometheus 0.27 API break
- **Files changed**: 4 (workspace Cargo.toml, cognicode-mcp Cargo.toml, server.rs, Cargo.lock)
- **Lines**: 97 insertions, 13 deletions

## Summary
Fixed pre-existing build break in cognicode-mcp-server caused by opentelemetry-prometheus 0.27 API incompatibility with the existing prometheus = "0.13" crate.

## What Was Done
1. Added `prometheus = "0.13"` to workspace dependencies with `workspace = true`
2. Set `prometheus.workspace = true` in cognicode-mcp Cargo.toml
3. Rewrote `crates/cognicode-mcp/src/server.rs`:
   - **Startup wiring**: Registry + PrometheusExporter + SdkMeterProvider + init_global_metrics
   - **metrics_handler rewrite**: TextEncoder + Registry.gather() (replaces deprecated metric family HTTP handler)
   - **Router state split to tuple** for proper ownership

## All 6 SCs Verified PASS
- SC-1: Build succeeds (cargo build -p cognicode-mcp)
- SC-2: /metrics returns Prometheus text format
- SC-3: ToolMetrics instruments (Counter, Histogram, etc.) are wired globally at server startup
- SC-4: Auth posture unchanged (no auth on /metrics, auth preserved on MCP channels)
- SC-5: Stdio transport regression — none
- SC-6: Auth lib regression — none

## Durable Knowledge Updates
| Topic | State | Notes |
|-------|-------|-------|
| cognicode-mcp-server + opentelemetry-prometheus 0.27 | **confirmed** | Builds and /metrics returns real Prometheus metrics |
| ToolMetrics global instrumentation | **confirmed** | Instruments wired at server startup via init_global_metrics |
| Runbook §9.5 (pre-existing build break warning) | **stale** | Should be removed/updated — the build break is fixed |

## Architecture Notes
- The HTTP/SSE binary is now **production-ready** with working Prometheus metrics
- The M3.5 auth work can be tested end-to-end — no blocking issues remain
- PrometheusExporter + SdkMeterProvider + Registry is the correct OTEL+Prometheus wiring pattern for this stack

## Next Steps
- Remove or update Runbook §9.5 stale warning
- No further SDDK work required for this change

---
*Archived by sddk-archive — 2026-06-17*
