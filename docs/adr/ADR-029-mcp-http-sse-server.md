# ADR-029: MCP HTTP/SSE Server — Standalone Container Deployment

**Status:** Proposed  
**Date:** 2026-06-16  
**Source:** Container distribution requirement + rmcp 1.7 Streamable HTTP

## Context

CogniCode MCP currently uses **stdio transport only** — OpenCode spawns the
binary as a child process and communicates via stdin/stdout. This works for
local development but has limitations:

1. **Container deployment** requires `podman exec` hacks to reach the stdio
   binary inside a container — not professional, not standard.
2. **Remote access** is impossible — the MCP must run on the same machine
   as the client.
3. **Multi-client** — only one client can talk to the MCP at a time (single
   stdin/stdout pair).
4. **Container distribution** — the standalone image needs to expose MCP
   over HTTP for any client to connect.

rmcp v1.7.0 (already in our workspace) provides
`transport-streamable-http-server` — a native MCP-over-HTTP/SSE transport
that integrates with `tower::Service` and `axum`. This is the standard MCP
Streamable HTTP transport (spec 2025-06-18).

## Decision

Add a new binary `cognicode-mcp-server` that serves MCP over HTTP/SSE using
rmcp's `StreamableHttpService`, designed for standalone container deployment.

### Architecture

```
┌──────────────────────────────────────────────────┐
│         cognicode-mcp-server (container)          │
│                                                   │
│  ┌─────────────┐    ┌──────────────────────────┐ │
│  │ PostgreSQL  │    │ axum server (:9847)      │ │
│  │   :5432     │    │  ├── POST /mcp (SSE)    │ │
│  │             │◄───┤  ├── GET  /mcp (stream)  │ │
│  │ graph_nodes │    │  ├── DEL  /mcp (cancel)  │ │
│  │ graph_edges │    │  └── GET  /health        │ │
│  │ scan_manifest│   │                           │ │
│  │ graph_reports│   │  StreamableHttpService   │ │
│  └─────────────┘    │  + LocalSessionManager   │ │
│                     │  + CogniCodeHandler       │ │
│                     └──────────────────────────┘ │
└──────────────────────────────────────────────────┘
         ▲
         │ HTTP/SSE (MCP Streamable HTTP transport)
         │
    ┌────┴────┐
    │ OpenCode │  →  "cognicode": { "type": "remote", "url": "http://localhost:9847/mcp" }
    │ Claude   │  →  same URL
    │ Cursor   │  →  same URL
    └─────────┘
```

### Key properties

1. **Stateful sessions** — `LocalSessionManager` tracks each client connection
   with a unique `SessionId`. Multiple clients can connect simultaneously.
2. **SSE streaming** — tool responses stream via Server-Sent Events. Long-running
   tools (build_graph, scan) can send progress updates.
3. **Session resumption** — clients can disconnect and resume using the
   `Last-Event-ID` header. The session manager caches events.
4. **Keep-alive** — SSE connections pinged every 30s to prevent proxy timeouts.
5. **Mode A/B** — same dual-mode as the stdio binary. Mode B connects to PG
   for persistent graphs.

### Integration with rmcp 1.7

```rust
// From rmcp's StreamableHttpService:
let session_manager = Arc::new(LocalSessionManager::default());
let config = StreamableHttpServerConfig {
    stateful_mode: true,
    json_response: false, // SSE streaming
    sse_keep_alive: Some(Duration::from_secs(30)),
    sse_retry: Some(Duration::from_secs(3)),
    ..
Default::default()
};

let service = StreamableHttpService::new(
    || Ok(CogniCodeHandler::new(cwd.clone())),
    session_manager,
    config,
);

// Mount on axum:
let app = axum::Router::new()
    .route_service("/mcp", service)  // tower::Service → axum
    .route("/health", get(|| async { "OK" }));
```

### OpenCode configuration

```json
{
  "mcp": {
    "cognicode": {
      "enabled": true,
      "type": "remote",
      "url": "http://localhost:9847/mcp"
    }
  }
}
```

### Container deployment

```bash
# Build
podman build -t cognicode-mcp-server .

# Run (single project, Mode B)
podman run -p 9847:9847 -v /my/project:/workspace:ro cognicode-mcp-server

# Run (with external PG)
podman run -p 9847:9847 \
  -e DATABASE_URL=postgres://user:pass@db:5432/cognicode \
  -v /my/project:/workspace:ro cognicode-mcp-server

# Health check
curl http://localhost:9847/health
```

### Relationship with stdio binary

| | cognicode-mcp (stdio) | cognicode-mcp-server (HTTP) |
|---|---|---|
| **Transport** | stdin/stdout | HTTP/SSE |
| **Clients** | 1 at a time | Multiple (sessions) |
| **Container** | `podman exec` (hacky) | HTTP endpoint (standard) |
| **Use case** | Local dev, single agent | Container, remote, multi-client |
| **Binaries** | Both share CogniCodeHandler | Same |

Both binaries coexist. The stdio binary remains for OpenCode local mode.
The HTTP server is for containerized/remote deployment.

## Consequences

- New binary `cognicode-mcp-server` added to `cognicode-mcp` crate.
- Requires `axum`, `tower`, `http`, `http-body-util`, `tokio-util` deps.
- The `StreamableHttpService` needs a proper tower→axum bridge. The rmcp
  crate provides `StreamableHttpService` as a `tower::Service<Request>`, which
  axum can consume via `Router::route_service()`.
- Session management is in-memory (`LocalSessionManager`). For multi-instance
  deployment, a `SessionStore` backed by Redis or PG is possible.
- The container image needs to bundle PG + the HTTP server.

## Alternatives Considered

- **Keep stdio only + podman exec:** rejected — hacky, not standard, fragile.
- **Separate HTTP proxy in front of stdio:** rejected — extra component,
  adds latency, doesn't solve multi-session.
- **Use explorer-api as MCP proxy:** rejected — explorer-api uses a different
  protocol (REST), not MCP. It would need to translate JSON-RPC → REST,
  losing MCP semantics.
- **Wait for rmcp to provide a ready-made axum integration:** the
  `StreamableHttpService` IS the integration. It's a `tower::Service`.
  Just mount it on axum with `route_service()`.

## SDD flow

This feature will be implemented via SDD kernel:
1. `sdd-kernel-explore` — investigate rmcp 1.7 StreamableHttpService API
2. `sdd-kernel-propose` — design the axum bridge and session management
3. `sdd-kernel-apply` — implement the binary + Dockerfile + quadlet
