//! CogniCode MCP Client — E2E test client using rmcp SDK
//!
//! Uses the official rmcp SDK to connect as a proper MCP client.
//!
//! Usage:
//!   mcp-client --workspace /path/to/repo \
//!     --method tools/call \
//!     --params '{"name":"build_graph","arguments":{"directory":"."}}'

use std::path::PathBuf;

use clap::Parser;
use rmcp::model::{CallToolRequestParams, GetPromptRequestParams, ReadResourceRequestParams};
use rmcp::transport::TokioChildProcess;
use rmcp::ServiceExt;

#[derive(Parser, Debug)]
#[command(name = "mcp-client", version, about = "CogniCode MCP test client (rmcp SDK)")]
struct Args {
    /// Path to the workspace directory (passed as --cwd to the server)
    #[arg(short, long)]
    workspace: PathBuf,

    /// JSON-RPC method to call after handshake (e.g. "tools/call")
    #[arg(short, long)]
    method: String,

    /// JSON params for the method call (default: {})
    #[arg(short, long, default_value = "{}")]
    params: String,

    /// Path to the cognicode-mcp binary (default: auto-detect from cargo)
    #[arg(short, long)]
    server_binary: Option<PathBuf>,

    /// Timeout in seconds for each response (default: 30)
    #[arg(short, long, default_value = "30")]
    timeout: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if !args.workspace.exists() {
        eprintln!("Error: workspace '{}' does not exist", args.workspace.display());
        std::process::exit(1);
    }

    // Resolve server binary path
    let server_bin = args.server_binary.unwrap_or_else(|| {
        let self_path = std::env::current_exe().expect("failed to get current exe path");
        let dir = self_path.parent().expect("no parent dir");
        dir.join("cognicode-mcp")
    });

    if !server_bin.exists() {
        eprintln!("Error: server binary not found at '{}'", server_bin.display());
        std::process::exit(1);
    }

    // Parse params
    let params: serde_json::Value = match serde_json::from_str(&args.params) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: invalid params JSON: {}", e);
            std::process::exit(1);
        }
    };

    // Spawn server as child process and use stdin/stdout as transport
    let mut cmd = tokio::process::Command::new(&server_bin);
    cmd.arg("--cwd").arg(&args.workspace);

    // Connect via rmcp using TokioChildProcess (handles length-prefixed framing)
    let client = ().serve(TokioChildProcess::new(cmd)?).await?;

    eprintln!("Connected to server via rmcp SDK");

    let result = match args.method.as_str() {
        "tools/list" => {
            let tools = client.list_all_tools().await?;
            serde_json::to_string_pretty(&tools)?
        }
        "tools/call" => {
            let tool_name = params["name"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            let arguments = params
                .get("arguments")
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();
            let tool_result = client
                .call_tool(
                    CallToolRequestParams::new(tool_name)
                        .with_arguments(arguments),
                )
                .await?;
            serde_json::to_string_pretty(&tool_result)?
        }
        "resources/list" => {
            let resources = client.list_all_resources().await?;
            serde_json::to_string_pretty(&resources)?
        }
        "resources/read" => {
            let uri = params["uri"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            let resource = client
                .read_resource(ReadResourceRequestParams::new(&uri))
                .await?;
            serde_json::to_string_pretty(&resource)?
        }
        "prompts/list" => {
            let prompts = client.list_all_prompts().await?;
            serde_json::to_string_pretty(&prompts)?
        }
        "prompts/get" => {
            let prompt_name = params["name"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            let arguments = params
                .get("arguments")
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();
            let prompt = client
                .get_prompt(
                    GetPromptRequestParams::new(prompt_name).with_arguments(arguments),
                )
                .await?;
            serde_json::to_string_pretty(&prompt)?
        }
        other => {
            anyhow::bail!("Unknown method: {}", other);
        }
    };

    // Print result to stdout
    println!("{}", result);

    // Disconnect
    client.cancel().await?;

    Ok(())
}
