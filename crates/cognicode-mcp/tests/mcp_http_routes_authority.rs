//! Which HTTP routes ask for the bearer token, and which stay open.
//!
//! The server is started for real with a credential configured, and every
//! route is asked over the wire with no token, a wrong token and the right
//! one. The routes that report or run something about the workspace (`/mcp`,
//! `/watch`) must refuse the first two; the three that an orchestrator or a
//! scraper reaches without a secret (`/health`, `/ready`, `/metrics`) must
//! keep answering.

mod common;

use std::io::Read;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{HttpResponse, free_port, http_get};

const TOKEN: &str = "route-authority-secret";

/// A server that is killed when the test ends, pass or fail.
struct Server {
    child: Child,
    addr: SocketAddr,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_with_token() -> Server {
    let port = free_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().expect("addr");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws");
    let child = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_cognicode-mcp-server")))
        .arg("--cwd")
        .arg(workspace)
        .arg("--listen")
        .arg(addr.to_string())
        .env("COGNICODE_MCP_AUTH_TOKEN", TOKEN)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cognicode-mcp-server");
    Server { child, addr }
}

/// Wait for the first answer of any status: `/health` needs no credential, so
/// it is the probe, and nothing is retried once a status line arrives.
fn wait_until_listening(server: &mut Server) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if let Ok(Some(status)) = server.child.try_wait() {
            let mut err = String::new();
            if let Some(stderr) = server.child.stderr.as_mut() {
                let _ = stderr.read_to_string(&mut err);
            }
            panic!("cognicode-mcp-server exited early with {status}\nstderr:\n{err}");
        }
        if http_get(server.addr, "/health", None).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    panic!(
        "cognicode-mcp-server never started listening on {}",
        server.addr
    );
}

fn status_code(response: &HttpResponse) -> u16 {
    response
        .status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("unreadable status line `{}`", response.status_line))
}

fn get(server: &Server, path: &str, token: Option<&str>) -> u16 {
    let response = http_get(server.addr, path, token)
        .unwrap_or_else(|e| panic!("GET {path} (token: {token:?}): {e}"));
    status_code(&response)
}

#[test]
fn the_workspace_routes_refuse_a_missing_or_wrong_token() {
    let mut server = start_with_token();
    wait_until_listening(&mut server);

    for path in ["/mcp", "/watch"] {
        assert_eq!(get(&server, path, None), 401, "{path} without a token");
        assert_eq!(
            get(&server, path, Some("not-the-secret")),
            401,
            "{path} with a wrong token"
        );
    }
}

#[test]
fn the_right_token_gets_past_the_check_on_the_watch_route() {
    let mut server = start_with_token();
    wait_until_listening(&mut server);

    assert_eq!(get(&server, "/watch", Some(TOKEN)), 200);
}

#[test]
fn the_orchestrator_and_scraper_routes_stay_open_with_a_token_configured() {
    let mut server = start_with_token();
    wait_until_listening(&mut server);

    for path in ["/health", "/ready", "/metrics"] {
        let code = get(&server, path, None);
        assert_ne!(code, 401, "{path} must not ask for the token");
    }
    assert_eq!(get(&server, "/metrics", None), 200);
}
