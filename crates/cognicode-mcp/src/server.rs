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

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use axum::{
    header,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use clap::Parser;
use cognicode_core::interface::mcp::CogniCodeHandler;
use opentelemetry_prometheus::Prometheus;

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

/// Handler for /metrics endpoint - exposes Prometheus-format metrics
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

    let service = rmcp::transport::streamable_http_server::StreamableHttpService::new(
        move || Ok(CogniCodeHandler::new(cwd.clone())),
        session_manager,
        config,
    );

    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/metrics", get(metrics_handler))
        .nest_service("/mcp", service);

    tracing::info!("CogniCode MCP HTTP/SSE Server on {}", args.listen);
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    axum::serve(listener, app).await?;
    Ok(())
}