//! The HTTP transport of the MCP server does not serve itself exposed.
//!
//! The server speaks JSON-RPC over stdio by default, and over HTTP when it
//! is started as `cognicode-mcp-server`. The HTTP transport is the one
//! other machines can reach, and the tools it exposes include the ones
//! that write, so an address other machines can reach without a credential
//! is a configuration that must not start.
//!
//! The rules themselves live in the library, where they are cheap to read
//! and cheap to test. This contract covers the other half, which is the
//! one that can rot silently: a rule that exists but that the process
//! does not consult. A server whose exposure rules are written down and
//! never called is a server that listens on every interface, and the
//! rules read like a guarantee.
//!
//! The check runs the real binary with a configuration that must be
//! refused, and with one that must be let through. Both are decided
//! before anything is bound, so neither waits for the graph.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The address the server would listen on if it started. In the refused
/// case it is never reached: the point is that the process ends before
/// binding it.
const EXPOSED_ADDRESS: &str = "0.0.0.0:19847";

/// How long the process is given to decide. A refusal is immediate, so a
/// process that is still running after this window was not refused.
const REFUSAL_WINDOW: Duration = Duration::from_secs(10);

/// What happened to a server that was started with this configuration.
struct Outcome {
    /// Whether the process is still running.
    running: bool,
    /// Whether it ended with a successful status.
    succeeded: bool,
    /// Whatever it said.
    said: String,
}

fn server_binary() -> &'static str {
    env!("CARGO_BIN_EXE_cognicode-mcp-server")
}

/// An empty directory, so the server has nothing to build a graph from
/// and the decision about exposure is the only thing it has to make.
fn empty_workspace(tag: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("cognicode-exposure-{tag}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("the empty workspace is creatable");
    path
}

/// Runs the server and reports how it ended.
fn run_server(workspace: &PathBuf, token: Option<&str>) -> Outcome {
    let mut command = Command::new(server_binary());
    command
        .arg("--listen")
        .arg(EXPOSED_ADDRESS)
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match token {
        Some(value) => {
            command.env("COGNICODE_MCP_AUTH_TOKEN", value);
        }
        None => {
            command.env_remove("COGNICODE_MCP_AUTH_TOKEN");
        }
    }

    let mut child: Child = command.spawn().expect("the server binary starts");
    let deadline = Instant::now() + REFUSAL_WINDOW;
    loop {
        match child.try_wait().expect("the server process is inspectable") {
            Some(status) => {
                let output = child.wait_with_output().expect("the output is readable");
                return Outcome {
                    running: false,
                    succeeded: status.success(),
                    said: format!(
                        "{}{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    ),
                };
            }
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Outcome {
                        running: true,
                        succeeded: false,
                        said: String::new(),
                    };
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

#[test]
fn a_server_exposed_without_a_credential_refuses_to_start() {
    let workspace = empty_workspace("refused");
    let outcome = run_server(&workspace, None);
    let _ = std::fs::remove_dir_all(&workspace);

    assert!(
        !outcome.running,
        "the server kept running while listening on {EXPOSED_ADDRESS} with \
         no credential. A tool that writes was reachable by anybody who \
         could open a socket."
    );
    assert!(
        !outcome.succeeded,
        "the server accepted an exposed address with no credential:\n{}",
        outcome.said
    );
    assert!(
        outcome.said.contains("COGNICODE_MCP_AUTH_TOKEN"),
        "the refusal has to name the variable that fixes it, otherwise the \
         operator is left guessing:\n{}",
        outcome.said
    );
}

#[test]
fn a_server_exposed_with_a_credential_gets_past_the_exposure_check() {
    let workspace = empty_workspace("allowed");
    let outcome = run_server(&workspace, Some("a-secret-value"));
    let _ = std::fs::remove_dir_all(&workspace);

    assert!(
        !outcome.said.contains("refusing to serve"),
        "a server with a credential was refused for its exposure:\n{}",
        outcome.said
    );
}
