// CogniCode Explorer MCP server binary.
//
// Reads JSON-RPC from stdin, writes responses to stdout. Logs and
// traces go to stderr. The handler follows the CogniCodeHandler
// canonical pattern (see cognicode-core/src/interface/mcp/rmcp_adapter.rs).
//
// Dual-mode (ADR-025):
//   Mode A (default): standalone, in-memory. `build_graph` uses the
//     LanguageConfig extractor. No PG required.
//   Mode B (--postgres <URL> or DATABASE_URL): loads the graph from PG
//     on startup. `build_graph` delegates to the ingest pipeline. Graph
//     is shared with the Explorer and persists across restarts.

use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "explorer-mcp",
    version,
    about = "CogniCode Explorer MCP — JSON-RPC over stdio.\n\n\
             Dual-mode: standalone (default) or PG-connected (--postgres)."
)]
struct Args {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,

    /// PostgreSQL connection URL. Enables Mode B (ADR-025): the graph is
    /// loaded from PG and shared with the Explorer. Falls back to the
    /// DATABASE_URL environment variable if not provided.
    #[arg(long)]
    postgres: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Mode B detection: --postgres flag or DATABASE_URL env
    let postgres_url = args.postgres.or_else(|| std::env::var("DATABASE_URL").ok());
    if postgres_url.is_some() {
        tracing::info!("Mode B: PG-connected — graph loaded from PostgreSQL");
    } else {
        tracing::info!("Mode A: standalone — graph built in-memory");
    }

    let runtime = cognicode_runtime::Runtime::bootstrap(args.cwd, postgres_url).await?;
    let handler = runtime.into_mcp_handler();
    let transport = rmcp::transport::io::stdio();
    let server = rmcp::serve_server(handler, transport).await?;
    server.waiting().await?;
    Ok(())
}
