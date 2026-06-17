# CogniCode MCP Server — Operations Runbook

**Audience**: operators and SREs deploying the `cognicode-mcp-server`
container in production or staging.

**Scope**: deployment, configuration, observability, troubleshooting
for the CogniCode MCP HTTP/SSE server (ADR-029, ADR-034 §7).

**Related**: see [`mcp-slos.md`](./mcp-slos.md) for service-level
objectives and ADR-034 for the production-readiness rationale.

---

## 1. Overview

`cognicode-mcp-server` is the standalone HTTP/SSE entry point for the
60 CogniCode MCP tools. It is built as a single static binary that can
run in two modes:

| Mode | Description | Persistence |
|------|-------------|-------------|
| **Mode A** (standalone) | Built-in graph cache, in-memory only | None — restart loses state |
| **Mode B** (PG-connected) | Graph + state persisted to PostgreSQL | PostgreSQL 14+ |

In both modes the server exposes the same HTTP surface:

- `POST /mcp` — Streamable HTTP MCP transport (the only endpoint
  that processes tool calls)
- `GET  /health` — process-alive liveness probe (always 200)
- `GET  /ready` — readiness probe (200 once the graph is loaded)
- `GET  /metrics` — Prometheus text-format metrics

The default listen address is `0.0.0.0:9847`.

## 2. Environment variables

All configuration is via env vars. CLI flags (e.g. `--listen`,
`--postgres`, `--cwd`) take precedence over env vars for the values
they cover.

| Variable | Default | Required for | Description |
|----------|---------|--------------|-------------|
| `COGNICODE_MCP_AUTH_TOKEN` | _unset_ | Production (Mode C auth) | When set to a non-empty string, `/mcp` requires `Authorization: Bearer <token>`. `/health`, `/ready`, `/metrics` are exempt. When unset, the server runs in dev mode and accepts all requests on `/mcp` (no auth). |
| `DATABASE_URL` | _unset_ | Mode B | PostgreSQL connection URL. Same format as `psql` (`postgres://user:pass@host:port/db`). Falls back to the `--postgres` CLI flag. |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | _unset_ | Distributed tracing | Standard OpenTelemetry OTLP gRPC endpoint. When unset, the server still records metrics to `/metrics` (Prometheus) and per-call log lines (see §5), but no traces are exported. |
| `OTEL_EXPORTER_OTLP_HEADERS` | _unset_ | Distributed tracing | Optional `key=value,key=value` header list for the OTLP exporter (e.g. auth tokens for managed collectors). |
| `RUST_LOG` | `info` | All | Standard `tracing_subscriber::EnvFilter` syntax. Examples: `info`, `info,cognicode_core=debug`, `warn,opentelemetry=info`. |
| `COGNICODE_MCP_TIMEOUT_OVERRIDE` | _unset_ | Per-category timeout (planned) | Reserved for a future per-category timeout override (M3.2 SLO follow-up). Not consumed in v1. |

> **Secret handling**: `COGNICODE_MCP_AUTH_TOKEN` is a credential.
> Inject it via your secret manager (Kubernetes Secret, Vault, SOPS,
> Quadlet `EnvironmentFile=`). Do **not** bake it into the container
> image. Do **not** log it.

## 3. Deployment

### 3.1 Build the binary

```bash
cargo build --release -p cognicode-mcp
# Binary: target/release/cognicode-mcp (the standalone CLI server)
# Binary: target/release/cognicode-mcp-server (the HTTP/SSE server, requires --features postgres)
```

> **Note**: as of ADR-034, the `cognicode-mcp-server` binary is the
> recommended deployment target. The `cognicode-mcp` binary is the
> stdio-mode CLI used for embedded / child-process scenarios.

### 3.2 Configure Mode A (standalone)

```bash
COGNICODE_MCP_AUTH_TOKEN="$(openssl rand -hex 32)" \
  ./cognicode-mcp-server \
    --listen 0.0.0.0:9847 \
    --cwd /var/lib/cognicode/workspace
```

The graph is held in memory. On restart, the next `build_graph` call
rebuilds it from the workspace.

### 3.3 Configure Mode B (PG-connected)

```bash
export DATABASE_URL="postgres://cognicode:${DB_PASSWORD}@db:5432/cognicode"
export COGNICODE_MCP_AUTH_TOKEN="$(openssl rand -hex 32)"
./cognicode-mcp-server \
  --listen 0.0.0.0:9847 \
  --cwd /var/lib/cognicode/workspace
```

PostgreSQL must be reachable and the `cognicode` database must exist.
Run schema migrations before first start (see
`crates/cognicode-core/migrations/`).

### 3.4 Container / Quadlet

A reference Quadlet unit (`quadlets/cognicode-mcp.container`) is
shipped in the repo. It reads `COGNICODE_MCP_AUTH_TOKEN` and
`DATABASE_URL` from an `EnvironmentFile=` directive — point that at
a `podman secret`-mounted path or systemd `LoadCredential=`.

### 3.5 OpenCode client config

```json
{
  "mcp": {
    "cognicode": {
      "type": "remote",
      "url": "http://cognicode-mcp.internal:9847/mcp",
      "headers": {
        "Authorization": "Bearer ${env:COGNICODE_MCP_AUTH_TOKEN}"
      }
    }
  }
}
```

If `COGNICODE_MCP_AUTH_TOKEN` is unset on the server, the `headers`
block is harmless — the server ignores it and accepts all requests.

## 4. Health checks

| Endpoint | Liveness semantics | When to use |
|----------|--------------------|-------------|
| `GET /health` | Process alive. Always returns `200 OK` with body `OK`. | Kubernetes `livenessProbe` — restart the container only if this fails for 30+ seconds (the process is wedged). |
| `GET /ready`  | Graph loaded and dispatchable. Returns `200 {"status":"ready","graph_loaded":true}` once `build_graph` has succeeded; `503 {"status":"not_ready","graph_loaded":false}` otherwise. | Kubernetes `readinessProbe` — pull the pod out of the Service until the graph is built, so traffic is not sent to a server that would 500 on every tool call. |
| `GET /metrics` | Same as `/health` for the process (always 200) **but** this is the metrics scrape target, not a probe. | Prometheus scrape config (`/metrics` path, every 15s). |

**Orchestrator template (Kubernetes)**:

```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 9847
  periodSeconds: 10
  failureThreshold: 3
readinessProbe:
  httpGet:
    path: /ready
    port: 9847
  periodSeconds: 5
  failureThreshold: 3
```

**Standalone health check (no orchestrator)**:

```bash
curl -fsS http://localhost:9847/health  # → "OK"
curl -fsS http://localhost:9847/ready   # → 503 until first build_graph, then 200
```

## 5. Metrics scraping

The server exposes Prometheus text format on `GET /metrics` (no auth
required, see §7). Recommended `prometheus.yml` scrape config:

```yaml
scrape_configs:
  - job_name: cognicode-mcp
    metrics_path: /metrics
    scrape_interval: 15s
    static_configs:
      - targets: ['cognicode-mcp.internal:9847']
        labels:
          service: cognicode-mcp
          mode: 'b'  # or 'a' for standalone
```

### 5.1 Key metrics

| Metric | Type | Labels | Meaning |
|--------|------|--------|---------|
| `cognicode_tool_calls_total` | Counter | `tool`, `category`, `status` | Every tool-call dispatch (M1.1 boundary). `status` is one of `ok|stub|gated|error|missing|skip`. |
| `cognicode_tool_errors_total` | Counter | `tool`, `category`, `error_type` | Subset of calls that returned an error. `error_type` ∈ {`error`, `timeout`, `rate_limit_exceeded`, `missing`} (M3.2/M3.3). |
| `cognicode_tool_duration_seconds` | Histogram | `tool`, `category` | Wall-clock duration of every tool call. **This is the histogram you build SLO alerts against** — see [`mcp-slos.md`](./mcp-slos.md). |
| `cognicode_graph_loaded` | Gauge | _none_ | 1 once the active graph is in memory, 0 otherwise. Mirrors `/ready`. |
| `cognicode_graph_nodes` | Gauge | _none_ | Number of nodes in the loaded graph. |
| `cognicode_graph_edges` | Gauge | _none_ | Number of edges in the loaded graph. |

SLO alert formulas are documented in [`mcp-slos.md`](./mcp-slos.md).

## 6. Log format

The server uses `tracing` with `tracing_subscriber::fmt()`. Default
output is human-readable. To get structured JSON:

```bash
RUST_LOG=info cargo run --bin cognicode-mcp-server -- ...
# → human format

# For JSON, set the env var. As of ADR-034, the server emits
# `tracing::info!` with named fields; downstream collectors that
# expect JSON should run a sidecar like vector or fluent-bit to
# re-encode.
```

### 6.1 Structured per-call log line (M3.4)

Every tool call emits one log line at the `info` level with the
following named fields:

```text
tool_call tool=build_graph category=graph duration_ms=1247 status=ok
```

```text
tool_call tool=graph_pagerank category=graph duration_ms=60001 status=error error_type=timeout
```

```text
tool_call tool=read_file category=file duration_ms=2 status=ok
```

To filter to only tool calls:

```bash
RUST_LOG=info cargo run --bin cognicode-mcp-server 2>&1 | grep tool_call
```

To forward to Loki / Elasticsearch, run a log shipper that matches
`tool=... duration_ms=... status=...` (a regular expression over the
`tracing` field syntax).

## 7. Security

### 7.1 Auth bypass in dev mode

When `COGNICODE_MCP_AUTH_TOKEN` is **unset or empty**, the server
runs in **dev mode**:

- `POST /mcp` accepts requests without an `Authorization` header.
- `/health`, `/ready`, `/metrics` are always public (no auth).
- A startup log line is emitted:
  `Mode C: auth DISABLED (no COGNICODE_MCP_AUTH_TOKEN set)`.

> **Do not run dev mode in production.** Set the env var on every
> production deployment. Local development on `localhost` is the
> intended use case for dev mode.

### 7.2 Bearer token validation

When `COGNICODE_MCP_AUTH_TOKEN=...` is set:

- The server requires `Authorization: Bearer <token>` on `POST /mcp`.
- `/health`, `/ready`, `/metrics` remain public (orchestrator probes
  and Prometheus scrapers do not need a token).
- Token comparison uses `subtle::ConstantTimeEq` (constant-time, no
  timing leak).
- Case-insensitive scheme (`Bearer` / `bearer` / `BEARER` are all
  accepted per RFC 7235).
- Mismatched or missing header → `401 Unauthorized`.
- A startup log line is emitted:
  `Mode C: auth ENABLED (Bearer token required on /mcp)`.

### 7.3 Token rotation

Token rotation is a restart-only operation in v1 (no hot reload):

```bash
# 1. Generate a new token
NEW_TOKEN="$(openssl rand -hex 32)"

# 2. Update the secret store / EnvironmentFile
# 3. Restart the container / pod
systemctl restart cognicode-mcp.service
# or
kubectl rollout restart deployment/cognicode-mcp
# or
podman restart cognicode-mcp
```

Downtime is bounded by the readiness probe: once `/ready` returns
200 the new token is active. The old token is invalidated the moment
the process exits.

### 7.4 Network exposure

Bind the server to a private interface in production:

```bash
./cognicode-mcp-server --listen 10.0.5.7:9847
```

Do not expose port `9847` to the public internet. Use a reverse proxy
or service mesh for TLS termination if remote access is required.

## 8. Common operations

### 8.1 Start

```bash
# Standalone
./cognicode-mcp-server --listen 0.0.0.0:9847 --cwd /workspace

# PG-connected
DATABASE_URL=postgres://... ./cognicode-mcp-server --listen 0.0.0.0:9847 --cwd /workspace
```

### 8.2 Stop

```bash
# Bare process
kill -TERM $(pgrep cognicode-mcp-server)

# systemd
systemctl stop cognicode-mcp.service

# Kubernetes
kubectl scale deployment/cognicode-mcp --replicas=0
```

The server traps `SIGTERM` and drains in-flight requests for up to
30s before exiting. `SIGKILL` skips the drain.

### 8.3 Restart

```bash
# systemd
systemctl restart cognicode-mcp.service

# Kubernetes (rolls pods one at a time, no downtime)
kubectl rollout restart deployment/cognicode-mcp
```

A restart is the **only** way to rotate `COGNICODE_MCP_AUTH_TOKEN` in
v1 (see §7.3).

### 8.4 Reload graph (no restart)

The graph is rebuilt on the next `build_graph` tool call. There is
no `SIGHUP` reload in v1. To force a rebuild from a client:

```bash
curl -X POST http://localhost:9847/mcp \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $COGNICODE_MCP_AUTH_TOKEN" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"build_graph","arguments":{"directory":"/workspace"}}}'
```

`/ready` will return 503 until the rebuild completes.

## 9. Troubleshooting

### 9.1 `/ready` returns 503

**Symptom**: `curl http://server:9847/ready` returns `503` with body
`{"status":"not_ready","graph_loaded":false}`.

**Cause**: no `build_graph` tool call has been processed yet, OR the
last `build_graph` failed.

**Remediation**:

1. Confirm `/health` is 200 (process is alive):
   ```bash
   curl -fsS http://server:9847/health
   ```
2. Trigger a `build_graph` call (see §8.4).
3. Watch the structured log for `tool_call tool=build_graph status=ok`.
4. Re-check `/ready`. It should flip to 200 within seconds for typical
   workspaces.

### 9.2 Tool call times out

**Symptom**: client sees `error_type=timeout` on a `tools/call`
response; the structured log shows
`tool_call tool=... status=error error_type=timeout duration_ms>=<timeout>`.

**Cause**: the tool handler exceeded the per-category timeout. See
[`mcp-slos.md`](./mcp-slos.md) for the per-category limits.

**Remediation**:

1. Identify the slow tool from the log line.
2. If it is `category=graph`, the 60s budget is hardcoded in v1.
   Consider reducing the workspace size, splitting the call into
   narrower queries, or moving to a faster storage backend.
3. If it is `category=search` (500ms) or `category=file` (200ms), the
   call is taking longer than expected — check disk I/O or index state.
4. If the timeout is recurrent for the same tool across requests, file
   a bug with the workspace + tool name + recent log lines.

### 9.3 Rate-limit error

**Symptom**: client sees `error_type=rate_limit_exceeded`; the
structured log shows the same.

**Cause**: the per-tool or per-strict-category rate limit was
exhausted (100 calls / 60s default; stricter for `graph`,
`navigation`, `aix`).

**Remediation**:

1. Identify the tool from the log line.
2. Reduce call frequency from the client side.
3. If the client is legitimate and the rate is too tight, raise it
   by configuring `InputValidator::with_strict_rate_limit(max, window)`
   at startup (M3.3 internal API — currently requires a code change).
4. For now the only operational lever is to space out calls or use
   fewer parallel agents.

### 9.4 Auth failure (401)

**Symptom**: `POST /mcp` returns `401 Unauthorized`.

**Cause**: `COGNICODE_MCP_AUTH_TOKEN` is set on the server but the
client is sending either no `Authorization` header or a mismatched
token.

**Remediation**:

1. Confirm the env var is set on the server:
   ```bash
   # Check the process environment
   tr '\0' '\n' < /proc/$(pgrep cognicode-mcp-server)/environ | grep COGNICODE_MCP_AUTH_TOKEN
   ```
2. Confirm the client sends the matching token:
   ```bash
   curl -X POST http://server:9847/mcp \
     -H "Authorization: Bearer $COGNICODE_MCP_AUTH_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
   ```
3. Check for whitespace, newline, or copy-paste artifacts in the
   token value.
4. If the client and server were restarted at different times, the
   tokens may be out of sync — rotate and re-deploy both.

### 9.5 `opentelemetry-prometheus::Prometheus` import error (build)

**Symptom**: `cargo build -p cognicode-mcp --bin cognicode-mcp-server`
fails with `unresolved import opentelemetry_prometheus::Prometheus`.

**Cause**: pre-existing build break introduced by the
opentelemetry-prometheus 0.27 upgrade. The `Prometheus::default()`
shortcut was removed in 0.27; the new API requires a `Registry` +
`SdkMeterProvider` + `Meter` setup at startup.

**Remediation**: this is out of scope for M3.5. Track in a follow-up
PR; the `cognicode-mcp` (stdio-mode) binary is unaffected, and the
`cognicode-mcp-server` HTTP/SSE mode is in the same broken state in
main as it was before this PR. Production deployments use the
containerised `cognicode-mcp` image which has the same Prometheus
config baked in.

### 9.6 Connection refused on 9847

**Symptom**: `curl: (7) Failed to connect to server port 9847`.

**Cause**: process is not running, or listening on a different
interface/port.

**Remediation**:

```bash
# Is the process running?
pgrep -fa cognicode-mcp-server

# What is it bound to?
ss -tlnp | grep 9847

# Restart it
systemctl restart cognicode-mcp.service
# or
./cognicode-mcp-server --listen 0.0.0.0:9847 --cwd /workspace
```

## 10. Cross-references

- [`mcp-slos.md`](./mcp-slos.md) — per-category p99 latency targets
  and error-rate objectives. Required reading before configuring
  Prometheus alerts.
- [`docs/adr/ADR-029-mcp-http-sse-server.md`](../adr/ADR-029-mcp-http-sse-server.md)
  — design rationale for the HTTP/SSE transport.
- [`docs/adr/ADR-034-mcp-production-readiness.md`](../adr/ADR-034-mcp-production-readiness.md)
  §7 — operational hardening plan that produced this runbook.
- `crates/cognicode-mcp/src/server.rs` — implementation. The startup
  log lines (`Mode A/B/C: ...`) document the runtime configuration
  chosen from env vars.
