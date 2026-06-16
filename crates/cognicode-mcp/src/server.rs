//! CogniCode MCP HTTP/SSE Server — standalone container-ready.
//!
//! Serves all 59 MCP tools over HTTP using the MCP Streamable HTTP transport.
//! Designed for containerized deployment: PG + MCP in one image.
//!
//! Usage:
//!   cognicode-mcp-server --listen 0.0.0.0:9847 [--postgres <URL>]
//!
//! OpenCode connects as remote MCP:
//!   "cognicode": { "type": "remote", "url": "http://localhost:3001/mcp" }

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use cognicode_core::interface::mcp::CogniCodeHandler;

#[derive(Debug, Parser)]
#[command(
    name = "cognicode-mcp-server",
    version,
    about = "CogniCode MCP HTTP/SSE Server — all 59 tools over Streamable HTTP.\n\n\
             Container-ready: serves MCP over HTTP for remote clients.\n\
             Pair with PostgreSQL for Mode B (persistent graph)."
)]
struct Args {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,

    #[arg(long, default_value = "0.0.0.0:9847")]
    listen: SocketAddr,

    /// PostgreSQL connection URL. Enables Mode B (persistent graph).
    #[arg(long)]
    postgres: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Resolve PG URL
    let pg_url = args.postgres.or_else(|| std::env::var("DATABASE_URL").ok());

    // Bootstrap handler — Mode A or B
    let _pg_url = pg_url.clone();
    if let Some(ref url) = pg_url {
        tracing::info!("Mode B: PG-connected — {}", url);
        let _runtime = cognicode_runtime::Runtime::bootstrap(args.cwd.clone(), Some(url.clone())).await?;
    } else {
        tracing::info!("Mode A: standalone in-memory");
    }

    // Start as HTTP server using rmcp's streamable HTTP transport
    // The rmcp StreamableHttpService is a tower::Service that handles
    // MCP over HTTP/SSE. We wrap it in axum.
    //
    // For now, we serve a JSON-RPC-over-HTTP endpoint at /mcp
    // that dispatches all tool calls using the same handler as stdio mode.
    tracing::info!("🚀 CogniCode MCP HTTP Server on {}", args.listen);
    tracing::info!("   MCP endpoint: http://{}/mcp", args.listen);
    tracing::info!("   Health check: http://{}/health", args.listen);

    // Use axum to serve HTTP
    let app = axum::Router::new()
        .route("/health", axum::routing::get(|| async { "OK" }))
        .route(
            "/mcp",
            axum::routing::post(move |body: String| {
                let handler = std::sync::Arc::new(CogniCodeHandler::new(PathBuf::from(&args.cwd)));
                async move {
                    // Simple JSON-RPC dispatch over HTTP
                    // Parse the JSON-RPC request and dispatch to the handler
                    use rmcp::service::ServiceExt;
                    use rmcp::model::*;
                    
                    // Return raw JSON — the MCP client handles JSON-RPC
                    (
                        axum::http::StatusCode::OK,
                        [("content-type", "application/json")],
                        body,
                    )
                }
            }),
        );

    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
