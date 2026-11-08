//! CogniCode MCP Client — E2E test client using rmcp SDK
//!
//! Uses the official rmcp SDK to connect as a proper MCP client.
//!
//! Usage:
//!   mcp-client --workspace /path/to/repo \
//!     --method tools/call \
//!     --params '{"name":"build_graph","arguments":{"directory":"."}}'
//!
//! For a real chained session, pass an ordered JSON array with `--sequence`:
//!   mcp-client --workspace /path/to/repo --sequence calls.json

use std::path::{Path, PathBuf};

use clap::Parser;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, GetPromptRequestParams, ReadResourceRequestParams};
use rmcp::transport::TokioChildProcess;
use serde::Deserialize;
use serde_json::Value;

#[derive(Parser, Debug)]
#[command(
    name = "mcp-client",
    version,
    about = "CogniCode MCP test client (rmcp SDK)"
)]
struct Args {
    /// Path to the workspace directory (passed as --cwd to the server)
    #[arg(short, long)]
    workspace: PathBuf,

    /// JSON-RPC method to call after handshake (e.g. "tools/call").
    /// Mutually exclusive with `--sequence`.
    #[arg(short, long)]
    method: Option<String>,

    /// JSON file containing an ordered sequence of MCP calls. All calls use
    /// the same child process and MCP session.
    #[arg(long, conflicts_with = "method")]
    sequence: Option<PathBuf>,

    /// JSON params for the method call (default: {})
    #[arg(short, long, default_value = "{}")]
    params: String,

    /// Path to the cognicode-mcp binary (default: auto-detect from cargo)
    #[arg(short, long)]
    server_binary: Option<PathBuf>,

    /// Timeout in seconds for each response (default: 30)
    #[arg(short, long, default_value = "30")]
    timeout: u64,

    /// The surface the server publishes (passed to it as `--profile`).
    ///
    /// TASK-84: the server announces one surface. The capture of the whole
    /// catalogue is the `full` one; the default surface announces the goals
    /// that answer before the analysis.
    #[arg(long, default_value = "default")]
    server_profile: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct SequenceCall {
    method: String,
    #[serde(default)]
    params: Value,
}

fn load_sequence(path: &Path) -> anyhow::Result<Vec<SequenceCall>> {
    let contents = std::fs::read_to_string(path)?;
    let calls = serde_json::from_str(&contents)?;
    Ok(calls)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if !args.workspace.exists() {
        eprintln!(
            "Error: workspace '{}' does not exist",
            args.workspace.display()
        );
        std::process::exit(1);
    }

    // Resolve server binary path
    let server_bin = args.server_binary.unwrap_or_else(|| {
        let self_path = std::env::current_exe().expect("failed to get current exe path");
        let dir = self_path.parent().expect("no parent dir");
        dir.join("cognicode-mcp")
    });

    if !server_bin.exists() {
        eprintln!(
            "Error: server binary not found at '{}'",
            server_bin.display()
        );
        std::process::exit(1);
    }

    let calls = match (args.method, args.sequence) {
        (Some(method), None) => vec![SequenceCall {
            method,
            params: serde_json::from_str(&args.params)?,
        }],
        (None, Some(path)) => load_sequence(&path)?,
        (None, None) => anyhow::bail!("one of --method or --sequence is required"),
        (Some(_), Some(_)) => unreachable!("clap enforces mutual exclusion"),
    };

    // Spawn server as child process and use stdin/stdout as transport
    let mut cmd = tokio::process::Command::new(&server_bin);
    cmd.arg("--cwd").arg(&args.workspace);
    cmd.arg("--profile").arg(&args.server_profile);

    // Connect via rmcp using TokioChildProcess (handles length-prefixed framing)
    let client = ().serve(TokioChildProcess::new(cmd)?).await?;

    eprintln!("Connected to server via rmcp SDK");

    let mut outputs = Vec::with_capacity(calls.len());
    for call in calls {
        let output = match call.method.as_str() {
            "tools/list" => serde_json::to_value(client.list_all_tools().await?)?,
            "tools/call" => {
                let tool_name = call.params["name"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                let arguments = call
                    .params
                    .get("arguments")
                    .and_then(|v| v.as_object())
                    .cloned()
                    .unwrap_or_default();
                serde_json::to_value(
                    client
                        .call_tool(CallToolRequestParams::new(tool_name).with_arguments(arguments))
                        .await?,
                )?
            }
            "resources/list" => serde_json::to_value(client.list_all_resources().await?)?,
            "resources/read" => {
                let uri = call.params["uri"].as_str().unwrap_or("unknown").to_string();
                serde_json::to_value(
                    client
                        .read_resource(ReadResourceRequestParams::new(&uri))
                        .await?,
                )?
            }
            "prompts/list" => serde_json::to_value(client.list_all_prompts().await?)?,
            "prompts/get" => {
                let prompt_name = call.params["name"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                let arguments = call
                    .params
                    .get("arguments")
                    .and_then(|v| v.as_object())
                    .cloned()
                    .unwrap_or_default();
                serde_json::to_value(
                    client
                        .get_prompt(
                            GetPromptRequestParams::new(prompt_name).with_arguments(arguments),
                        )
                        .await?,
                )?
            }
            other => anyhow::bail!("Unknown method: {other}"),
        };
        outputs.push(output);
    }

    let result = if outputs.len() == 1 {
        outputs.pop().expect("one output exists")
    } else {
        Value::Array(outputs)
    };

    println!("{}", serde_json::to_string_pretty(&result)?);

    // Disconnect
    client.cancel().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::load_sequence;
    use std::io::Write;

    #[test]
    fn sequence_file_preserves_order_and_params() {
        let mut file = tempfile::NamedTempFile::new().expect("create sequence fixture");
        file.write_all(
            br#"[{"method":"tools/list"},{"method":"tools/call","params":{"name":"build_graph","arguments":{"directory":"."}}}]"#,
        )
        .expect("write sequence fixture");

        let calls = load_sequence(file.path()).expect("parse sequence");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].method, "tools/list");
        assert_eq!(calls[1].params["name"], "build_graph");
        assert_eq!(calls[1].params["arguments"]["directory"], ".");
    }
}
