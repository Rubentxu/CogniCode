//! CogniCode MCP HTTP/SSE Server — standalone container-ready.
//!
//! Serves all 59 MCP tools over HTTP using the MCP Streamable HTTP transport.
//! Designed for containerized deployment: PG + MCP in one image.
//!
//! Usage:
//!   cognicode-mcp-server --listen 0.0.0.0:9847 [--postgres <URL>]
//!
//! OpenCode connects as remote MCP:
//!   "cognicode": { "type": "remote", "url": "http://localhost:9847/mcp" }
//!
//! M3.5: Bearer-token auth middleware. Set `COGNICODE_MCP_AUTH_TOKEN` to
//! require `Authorization: Bearer <token>` on `/mcp`. When unset, the
//! auth layer passes all requests through (localhost dev mode).
//! `/health`, `/ready`, `/metrics` are exempt by design so orchestrator
//! probes and Prometheus scrapers do not need to hold a token.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use clap::Parser;
use cognicode_core::interface::mcp::CogniCodeHandler;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use opentelemetry_prometheus::Prometheus;

use cognicode_mcp::auth::check_bearer_token;

#[derive(Debug, Parser)]
#[command(name = "cognicode-mcp-server", version)]
struct Args {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,
    #[arg(long, default_value = "0.0.0.0:9847")]
    listen: SocketAddr,
    #[arg(long)]
    postgres: Option<String>,
}

/// Handler for `/health` — process alive, always 200 OK.
async fn health_handler() -> &'static str {
    "OK"
}

/// Handler for `/metrics` endpoint — exposes Prometheus-format metrics.
async fn metrics_handler() -> impl IntoResponse {
    let exporter = Prometheus::default();
    let body = match exporter.export() {
        Ok(body) => body,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    };
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        body,
    )
        .into_response()
}

/// M3.1: Readiness probe — returns 200 with `{"status":"ready","graph_loaded":true}`
/// once `build_graph` has succeeded at least once, otherwise 503 with
/// `{"status":"not_ready","graph_loaded":false}`. Distinct from `/health`,
/// which always returns 200 (process alive).
async fn ready_handler(State(ctx): State<Arc<HandlerContext>>) -> impl IntoResponse {
    if ctx.is_graph_loaded() {
        (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ready", "graph_loaded": true})),
        )
            .into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"status": "not_ready", "graph_loaded": false})),
        )
            .into_response()
    }
}

/// M3.5: Bearer-token auth middleware. Applied to `/mcp` only;
/// `/health`, `/ready`, and `/metrics` remain public so orchestrator
/// probes and Prometheus scrapers do not need a token.
///
/// Behaviour:
/// - `COGNICODE_MCP_AUTH_TOKEN` unset or empty → pass through (dev mode).
/// - `COGNICODE_MCP_AUTH_TOKEN` set to non-empty value → require a
///   matching `Authorization: Bearer <token>` header; otherwise 401.
/// - Comparison uses `subtle::ConstantTimeEq` to avoid leaking length
///   or content timing.
///
/// We read the env var on every request (instead of caching at startup)
/// so an operator can rotate the token by restarting the container —
/// no hot-reload semantics needed for v1.
async fn auth_middleware(
    State(_ctx): State<Arc<HandlerContext>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // No token configured → dev mode, pass through.
    let expected_token = match std::env::var("COGNICODE_MCP_AUTH_TOKEN") {
        Ok(t) if !t.is_empty() => t,
        _ => return Ok(next.run(request).await),
    };

    // Read the Authorization header (if any) as a string slice.
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    check_bearer_token(auth_header, &expected_token)?;
    Ok(next.run(request).await)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let pg_url = args.postgres.or_else(|| std::env::var("DATABASE_URL").ok());
    if pg_url.is_some() {
        tracing::info!("Mode B: PG-connected");
    } else {
        tracing::info!("Mode A: standalone");
    }

    // M3.5: surface whether the auth gate is active at startup. This
    // makes the security posture obvious in container logs and is the
    // same kind of "mode" line we already emit for PG.
    match std::env::var("COGNICODE_MCP_AUTH_TOKEN") {
        Ok(t) if !t.is_empty() => {
            tracing::info!("Mode C: auth ENABLED (Bearer token required on /mcp)");
        }
        _ => {
            tracing::info!("Mode C: auth DISABLED (no COGNICODE_MCP_AUTH_TOKEN set)");
        }
    }

    let cwd = args.cwd.clone();
    let session_manager = std::sync::Arc::new(
        rmcp::transport::streamable_http_server::session::local::LocalSessionManager::default()
    );
    let mut config = rmcp::transport::streamable_http_server::StreamableHttpServerConfig::default();
    config.stateful_mode = true;
    config.json_response = false;
    config.sse_keep_alive = Some(Duration::from_secs(30));
    config.sse_retry = Some(Duration::from_secs(3));
    config.allowed_hosts = vec![
        "localhost".into(),
        "127.0.0.1".into(),
        "::1".into(),
        "0.0.0.0".into(),
    ];

    // M3.1: Build the HandlerContext once and share it between the
    // MCP dispatch (per session) and the /ready HTTP handler. The
    // `graph_loaded` flag is set by `call_tool_handler` after a
    // successful `build_graph` and read by `ready_handler` via the
    // shared Arc<HandlerContext>.
    let shared_ctx: Arc<HandlerContext> = Arc::new(
        HandlerContext::builder()
            .with_working_dir(cwd.clone())
            .build(),
    );

    let service = rmcp::transport::streamable_http_server::StreamableHttpService::new(
        {
            let shared_ctx = shared_ctx.clone();
            move || Ok(CogniCodeHandler::from_ctx(shared_ctx.clone()))
        },
        session_manager,
        config,
    );

    // M3.5: split the router so the auth middleware applies to /mcp
    // only. /health, /ready, /metrics are public — orchestrators and
    // Prometheus scrapers must reach them without a token. We mount
    // the middleware via `from_fn_with_state` so it shares the same
    // `Arc<HandlerContext>` as the rest of the request pipeline.
    let api_routes = Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(
            shared_ctx.clone(),
            auth_middleware,
        ));

    let public_routes = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/metrics", get(metrics_handler));

    let app = Router::new()
        .merge(api_routes)
        .merge(public_routes)
        .with_state(shared_ctx);

    tracing::info!("CogniCode MCP HTTP/SSE Server on {}", args.listen);
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    axum::serve(listener, app).await?;
    Ok(())
}