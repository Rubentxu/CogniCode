// CogniCode Explorer MCP server binary.
//
// Reads JSON-RPC from stdin, writes responses to stdout. Logs and
// traces go to stderr. The handler follows the CogniCodeHandler
// canonical pattern (see cognicode-core/src/interface/mcp/rmcp_adapter.rs).
//
// LadybugDB is the sole persistence backend.
// the MCP server runs in-memory.

use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "explorer-mcp",
    version,
    about = "CogniCode Explorer MCP — JSON-RPC over stdio.\n\n\
             LadybugDB-backed."
)]
struct Args {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing::info!("Starting explorer MCP (in-memory handler)");

    let runtime = cognicode_runtime::bootstrap(args.cwd).await?;
    let handler = runtime.into_mcp_handler();
    let transport = rmcp::transport::io::stdio();
    let server = rmcp::serve_server(handler, transport).await?;
    server.waiting().await?;
    Ok(())
}
