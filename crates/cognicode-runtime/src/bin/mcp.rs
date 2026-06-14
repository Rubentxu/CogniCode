// CogniCode Explorer MCP server binary.
//
// Reads JSON-RPC from stdin, writes responses to stdout. Logs and
// traces go to stderr. The handler follows the CogniCodeHandler
// canonical pattern (see cognicode-core/src/interface/mcp/rmcp_adapter.rs).

use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "explorer-mcp",
    version,
    about = "CogniCode Explorer MCP — JSON-RPC over stdio.\n\n\
             The MCP server operates in explore-only mode without a graph database."
)]
struct Args {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let runtime = cognicode_runtime::Runtime::bootstrap(args.cwd, None).await?;
    let handler = runtime.into_mcp_handler();
    let transport = rmcp::transport::io::stdio();
    let server = rmcp::serve_server(handler, transport).await?;
    server.waiting().await?;
    Ok(())
}
