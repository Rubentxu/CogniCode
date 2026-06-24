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
    Json, Router,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use clap::Parser;
use cognicode_core::interface::mcp::CogniCodeHandler;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use opentelemetry::global;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use prometheus::{Encoder, Registry};

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
///
/// The shared `Arc<Registry>` is the same instance that the
/// `PrometheusExporter` registered itself against at startup, so
/// `Registry::gather()` walks the exporter's collector and yields the
/// `ToolMetrics` instruments as Prometheus `MetricFamily`s. We then
/// encode them with `TextEncoder` (the 0.27 replacement for the
/// removed `Prometheus::default().export()` shortcut).
///
/// Content type is locked to `text/plain; version=0.0.4` per the
/// Prometheus exposition format spec — orchestrator scrape configs
/// match on this literal.
async fn metrics_handler(
    State((_ctx, registry)): State<(Arc<HandlerContext>, Arc<Registry>)>,
) -> impl IntoResponse {
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&registry.gather(), &mut buffer) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("metrics encode failed: {e}"),
        )
            .into_response();
    }
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        buffer,
    )
        .into_response()
}

/// Handler for `/watch` — file watcher status.
///
/// Returns JSON with watcher state, workspace path, and uptime.
/// Always 200 OK (watcher is optional — not a readiness requirement).
async fn watch_handler(
    State((ctx, _registry)): State<(Arc<HandlerContext>, Arc<Registry>)>,
) -> impl IntoResponse {
    let workspace = ctx.working_dir.display().to_string();
    Json(serde_json::json!({
        "status": "active",
        "workspace": workspace,
        "debounce_ms": 500,
        "watched_extensions": ["rs","py","ts","tsx","js","jsx","go","java","c","h","cpp","cs","tf","yml","yaml","rb","php","swift","md","json","toml"],
    }))
}

/// M3.1: Readiness probe — returns 200 with `{"status":"ready","graph_loaded":true}`
/// once `build_graph` has succeeded at least once, otherwise 503 with
/// `{"status":"not_ready","graph_loaded":false}`. Distinct from `/health`,
/// which always returns 200 (process alive).
///
/// The state is a tuple `(Arc<HandlerContext>, Arc<Registry>)`; the
/// readiness probe only needs the context. The registry is the second
/// slot because the global `Router::with_state` is shared with
/// `metrics_handler` and `/health`.
async fn ready_handler(
    State((ctx, _registry)): State<(Arc<HandlerContext>, Arc<Registry>)>,
) -> impl IntoResponse {
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
        rmcp::transport::streamable_http_server::session::local::LocalSessionManager::default(),
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

    // Prometheus wiring (fix for bug-prometheus-027).
    //
    // The 0.27 OTel→Prom exporter removed the `Prometheus::default()
    // .export()` shortcut, so we have to build a `prometheus::Registry`,
    // register a `PrometheusExporter` against it, hand the exporter to
    // an `SdkMeterProvider` as a `MetricReader`, install the provider
    // globally, and then call `init_global_metrics()` so the
    // `ToolMetrics` instruments in `cognicode-core` (counters /
    // histograms / gauges for tool calls, duration, errors, graph
    // health, etc.) attach to our provider.
    //
    // The same `Arc<Registry>` is then shared with the `/metrics`
    // handler via the router's tuple state — `metrics_handler` walks
    // the registry with `TextEncoder::encode(&registry.gather(), …)`
    // to render the exposition.
    //
    // Note on API: the 0.27 entry point is the free function
    // `opentelemetry_prometheus::exporter()` (not
    // `PrometheusExporter::builder()`); it returns an `ExporterBuilder`
    // whose `.with_registry(reg).build()` returns a `MetricResult`.
    // We use `?` on `anyhow::Result<()>` because the build error
    // type (`MetricError`) implements `std::error::Error`.
    let prometheus_registry = Arc::new(Registry::new());

    let exporter = opentelemetry_prometheus::exporter()
        .with_registry((*prometheus_registry).clone())
        .build()?;

    let meter_provider = SdkMeterProvider::builder().with_reader(exporter).build();
    global::set_meter_provider(meter_provider);

    // `init_global_metrics()` returns
    // `Result<(), Box<dyn std::error::Error>>` (no Send+Sync on the
    // default `dyn Error`), so the `?` to `anyhow::Result<()>` needs
    // an explicit conversion. Surface the failure fast — if telemetry
    // never registers, SC-3 (instrumented metrics visible on
    // `/metrics`) cannot pass, and silently warning would mask a
    // broken observability surface from operators.
    cognicode_core::infrastructure::telemetry::init_global_metrics()
        .map_err(|e| anyhow::anyhow!("telemetry init failed: {e}"))?;

    // ── File watcher (Sprint 4 / ADR-022) ─────────────────────────────────
    // Start the file watcher as a background task. It watches the workspace
    // for file changes and triggers graph rebuilds automatically.
    let (watcher_handle, watcher_rx) =
        cognicode_core::application::ingest::watcher::start_watcher(cwd.clone());
    let watcher_handle = Arc::new(std::sync::Mutex::new(watcher_handle));

    // Spawn a task that consumes watcher events and triggers graph rebuilds.
    // Uses debounce_changes to coalesce rapid file changes into a single rebuild.
    {
        let ctx_for_watcher = shared_ctx.clone();
        let debounced =
            cognicode_core::application::ingest::watcher::debounce_changes(watcher_rx, 500);
        tokio::spawn(async move {
            let mut rx = debounced.await;
            while let Some(changed_files) = rx.recv().await {
                tracing::info!(
                    "file watcher: {} files changed, rebuilding graph",
                    changed_files.len()
                );
                let input =
                    cognicode_core::interface::mcp::handlers::BuildGraphInput { directory: None };
                match cognicode_core::interface::mcp::handlers::handle_build_graph(
                    &ctx_for_watcher,
                    input,
                )
                .await
                {
                    Ok(output) => {
                        tracing::info!(
                            "file watcher: graph rebuilt — {} symbols, {} edges",
                            output.symbols_found,
                            output.relationships_found
                        );
                    }
                    Err(e) => {
                        tracing::error!("file watcher: graph rebuild failed: {e}");
                    }
                }
            }
            tracing::info!("file watcher: event channel closed");
        });
    }

    tracing::info!("file watcher active for {}", cwd.display());

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
    // `Arc<HandlerContext>` as the rest of the request pipeline. The
    // middleware's local state (`Arc<HandlerContext>`) is independent
    // of the global router state (the tuple below), which is why
    // `auth_middleware` keeps its `State<Arc<HandlerContext>>`
    // signature unchanged.
    let api_routes =
        Router::new()
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
        .route("/watch", get(watch_handler))
        .with_state((shared_ctx, prometheus_registry));

    tracing::info!("CogniCode MCP HTTP/SSE Server on {}", args.listen);
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
