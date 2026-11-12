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
//! refused, and with one that must be let through. The refusal is decided
//! before anything is bound, so it does not wait for the graph.

use crate::common;
use crate::mcp_http_server_posture::{initialize, post};

use std::net::Ipv4Addr;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::HttpServer;

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

/// Runs the server and reports how it ended.
fn run_server(workspace: &Path, token: Option<&str>) -> Outcome {
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
    // An empty directory, so the server has nothing to build a graph from
    // and the decision about exposure is the only thing it has to make.
    let workspace = tempfile::tempdir().expect("an empty workspace");
    let outcome = run_server(workspace.path(), None);

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

/// The other half: with a credential, the wide address is served, and it is
/// served under the name it was bound to, `0.0.0.0` included.
#[test]
fn a_server_exposed_with_a_credential_is_served_under_the_address_it_was_given() {
    let token = "a-secret-value";
    let workspace = tempfile::tempdir().expect("an empty workspace");
    let server = HttpServer::start_on(
        Ipv4Addr::UNSPECIFIED.into(),
        workspace.path(),
        Some(token),
        &[],
    );
    let port = server.addr.port();

    let answered = post(
        &server,
        &format!("0.0.0.0:{port}"),
        Some(token),
        None,
        &initialize(),
    );
    assert_eq!(
        answered.status(),
        200,
        "an operator who bound 0.0.0.0 reaches the server through it: {}\nstderr:\n{}",
        answered.status_line.trim(),
        server.stderr()
    );
}
