//! CogniCode MCP HTTP/SSE Server — standalone container-ready.
//!
//! Serves all 59 MCP tools over HTTP using the MCP Streamable HTTP transport.
//! Designed for containerized deployment: PG + MCP in one image.
//!
//! Usage:
//!   cognicode-mcp-server --listen 127.0.0.1:9847 [--postgres `<URL>`]
//!
//! OpenCode connects as remote MCP:
//!   "cognicode": { "type": "remote", "url": "http://localhost:9847/mcp" }
//!
//! The server listens on loopback unless it is told otherwise. Serving an
//! address other machines can reach requires a bearer token, and starting
//! without one is refused rather than served in the clear: the tools it
//! exposes include the ones that write.
//!
//! The bearer token is read from `COGNICODE_MCP_AUTH_TOKEN` once, at
//! startup. `/health`, `/ready` and `/metrics` are exempt by design so
//! orchestrator probes and Prometheus scrapers do not need to hold a
//! token; everything else, including `/watch`, requires it when the
//! server is exposed.

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
use cognicode_mcp::exposure::{DEFAULT_LISTEN, configured_token, ensure_safe_exposure};

#[derive(Debug, Parser)]
#[command(name = "cognicode-mcp-server", version)]
struct Args {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,
    #[arg(long, default_value = DEFAULT_LISTEN)]
    listen: SocketAddr,
    /// TASK-84: the surface this server publishes in `tools/list`.
    ///
    /// `default` publishes the goals that answer before the analysis,
    /// `full` publishes every goal and every tool, and `legacy` publishes
    /// the tools with the notice that names the goal that replaces them.
    /// This is not the install profile (`core`, `reviewer`, ...): that one
    /// decides what the server may do, this one decides what it announces.
    #[arg(long, default_value = "default")]
    profile: String,
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
/// Returns JSON with watcher state, workspace path, and the files the
/// watcher looks at. The extensions come from the core, so this endpoint
/// names what the watcher really watches instead of a list of its own.
/// Always 200 OK (watcher is optional — not a readiness requirement).
async fn watch_handler(
    State((ctx, _registry)): State<(Arc<HandlerContext>, Arc<Registry>)>,
) -> impl IntoResponse {
    let workspace = ctx.working_dir().display().to_string();
    Json(serde_json::json!({
        "status": "active",
        "workspace": workspace,
        "debounce_ms": 500,
        "watched_extensions": cognicode_core::application::ingest::observed_extensions(),
        "ignored_directories": cognicode_core::application::ingest::noise_directories(),
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

/// Bearer-token auth middleware. Applied to every route except the
/// three that orchestrators and scrapers reach without a credential:
/// `/health`, `/ready` and `/metrics`.
///
/// Behaviour:
/// - The server was started without a token → the address it listens on
///   is loopback, so pass through. Nothing else can reach it.
/// - The server was started with a token → require a matching
///   `Authorization: Bearer <token>` header; otherwise 401.
/// - Comparison uses `subtle::ConstantTimeEq` to avoid leaking length
///   or content timing.
///
/// The token is the one read at startup, not one read per request: the
/// rule that depends on it decides whether the server may be exposed at
/// all, and a credential that appears halfway through the life of a
/// server is a credential that was absent while it was exposed.
async fn auth_middleware(
    State(_ctx): State<Arc<HandlerContext>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // No token configured → the address is loopback, pass through.
    let Some(expected_token) = configured_token() else {
        return Ok(next.run(request).await);
    };

    // Read the Authorization header (if any) as a string slice.
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    check_bearer_token(auth_header, &expected_token)?;
    Ok(next.run(request).await)
}

/// The daemon's index, read from the snapshot it wrote.
///
/// The snapshot is what a client will be served, so the index the daemon
/// vouches for is the one the snapshot describes. A daemon with no
/// snapshot keeps no index, and it says so: it vouches for nothing.
fn refresh_daemon_index(
    state: &cognicode_core::interface::daemon::DaemonState,
    cache_root: Option<&std::path::Path>,
    workspace: &std::path::Path,
) {
    let Some(root) = cache_root else {
        state.mark_changed();
        return;
    };
    match cognicode_core::interface::daemon::index_from_snapshot(root, workspace) {
        Some(record) => state.publish(record),
        None => state.mark_changed(),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("Standalone in-memory handler");

    // The exposure rules decide whether this server may be reachable from
    // outside this machine. They are checked before anything is bound, so
    // an insecure configuration ends the process instead of serving.
    let token = configured_token();
    ensure_safe_exposure(&args.listen, token.as_deref())
        .map_err(|message| anyhow::anyhow!(message))?;

    // Surface the posture at startup: the mode line in the container log
    // is the same kind of "mode" line we already emit for PG.
    match token {
        Some(_) => {
            tracing::info!("Mode C: auth ENABLED (Bearer token required)");
        }
        None => {
            tracing::info!("Mode C: auth DISABLED (loopback address only)");
        }
    }

    let cwd = args.cwd.clone();

    // Una superficie que no se conoce no se asume: el servidor publicaría
    // algo que nadie pidió, y el que lo pidió no sabría qué recibe.
    let profile = match cognicode_core::interface::surface::SurfaceProfile::parse(&args.profile) {
        Some(profile) => profile,
        None => {
            eprintln!(
                "Error: unknown surface profile '{}'. The profiles are: {}.",
                args.profile,
                cognicode_core::interface::surface::SurfaceProfile::names().join(", ")
            );
            std::process::exit(2);
        }
    };
    tracing::info!("surface profile: {}", profile.as_str());
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
    //
    // The cache is the user's: this server keeps what it derives in the
    // same place the stdio server does, so the two agree on what the
    // product reuses and the analysed tree stays untouched.
    let cache_root = cognicode_core::application::graph_cache::default_cache_root();
    let mut ctx_builder = HandlerContext::builder()
        .with_working_dir(cwd.clone())
        .with_cache_root(cache_root.clone());
    // This server is the daemon: it is the one that watches the tree, and
    // its word is about the index it keeps. Asking it is not a second
    // opinion —it is the only watcher there is— and the answer says who
    // served the graph.
    if let Some(root) = cache_root.as_ref() {
        ctx_builder = ctx_builder.with_freshness_oracle(Arc::new(
            cognicode_core::interface::daemon::DaemonOracle::for_workspace(root, &cwd),
        ));
    }
    let shared_ctx: Arc<HandlerContext> = Arc::new(ctx_builder.build());

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
    let _watcher_handle = Arc::new(std::sync::Mutex::new(watcher_handle));

    // ── The daemon's local socket ─────────────────────────────────────────
    // A client on this machine asks the daemon whether the index it keeps
    // is still fresh, and the daemon answers from what its watcher has
    // seen. The socket lives in the user's cache, next to the snapshot
    // the two of them already share, and it is private to the user.
    let daemon_state = Arc::new(cognicode_core::interface::daemon::DaemonState::new());
    let cache_root = cognicode_core::application::graph_cache::default_cache_root();
    if let Some(root) = cache_root.as_ref() {
        let socket = cognicode_core::application::graph_cache::daemon_socket_path(root, &cwd);
        match cognicode_core::interface::daemon::bind(&socket) {
            Ok(listener) => {
                let state_for_socket = daemon_state.clone();
                std::thread::spawn(move || {
                    cognicode_core::interface::daemon::serve(listener, state_for_socket);
                });
                tracing::info!("daemon socket at {}", socket.display());
            }
            Err(e) => {
                // The daemon still serves over HTTP: the socket is a way
                // in, not the way in.
                tracing::error!("daemon socket at {} failed: {e}", socket.display());
            }
        }
    }

    // The daemon's index: it is calculated at startup, so the first
    // client finds a warm index and not a cold one. A daemon with no
    // index says nothing is fresh, and the client goes on without it.
    {
        let input = cognicode_core::interface::mcp::handlers::BuildGraphInput { directory: None };
        match cognicode_core::interface::mcp::handlers::handle_build_graph(&shared_ctx, input).await
        {
            Ok(output) => {
                tracing::info!(
                    "daemon index: {} symbols, {} relationships",
                    output.symbols_found,
                    output.relationships_found
                );
                refresh_daemon_index(&daemon_state, cache_root.as_deref(), &cwd);
            }
            Err(e) => {
                tracing::error!("daemon index: the first build failed: {e}");
            }
        }
    }

    // Spawn a task that consumes watcher events and triggers graph rebuilds.
    // Uses debounce_changes to coalesce rapid file changes into a single rebuild.
    {
        let ctx_for_watcher = shared_ctx.clone();
        let state_for_watcher = daemon_state.clone();
        let state_for_report = daemon_state.clone();
        let root_for_watcher = cache_root.clone();
        let workspace_for_watcher = cwd.clone();
        let debounced = cognicode_core::application::ingest::watcher::debounce_changes_reporting(
            watcher_rx,
            500,
            move || {
                // The tree moved: the index is not fresh from this moment,
                // not from the moment the rebuild finishes.
                state_for_report.mark_changed();
            },
        );
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
                        refresh_daemon_index(
                            &state_for_watcher,
                            root_for_watcher.as_deref(),
                            &workspace_for_watcher,
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
            move || {
                Ok(CogniCodeHandler::from_ctx_with_profile(
                    shared_ctx.clone(),
                    profile,
                ))
            }
        },
        session_manager,
        config,
    );

    // The auth middleware covers every route that is not one of the three
    // orchestrator and scraper endpoints. `/watch` lives here with the
    // rest: it reports the workspace path and the watcher state, and a
    // diagnostic that names the directory the server works on is not
    // something to hand to whoever can reach the port.
    let protected_routes = Router::new()
        .nest_service("/mcp", service)
        .route("/watch", get(watch_handler))
        .layer(middleware::from_fn_with_state(
            shared_ctx.clone(),
            auth_middleware,
        ));

    let public_routes = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/metrics", get(metrics_handler));

    let app = Router::new()
        .merge(protected_routes)
        .merge(public_routes)
        .with_state((shared_ctx, prometheus_registry));

    tracing::info!("CogniCode MCP HTTP/SSE Server on {}", args.listen);
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
