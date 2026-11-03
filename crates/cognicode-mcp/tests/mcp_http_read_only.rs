//! The read-only posture of the HTTP server.
//!
//! The stdio binary takes `--read-only` and hides the tools that change the
//! workspace; the HTTP server has to offer the same posture, and it has to be
//! asked over the wire, because the posture lives in the context the server
//! builds and not in anything a unit could assert on its own.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{free_port, http_get};
use serde_json::{Value, json};

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

fn start(read_only: bool) -> Server {
    let port = free_port();
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().expect("addr");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws");
    let mut command = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_cognicode-mcp-server")));
    command
        .arg("--cwd")
        .arg(workspace)
        .arg("--listen")
        .arg(addr.to_string());
    if read_only {
        command.arg("--read-only");
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cognicode-mcp-server");

    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            let mut err = String::new();
            if let Some(stderr) = child.stderr.as_mut() {
                let _ = stderr.read_to_string(&mut err);
            }
            panic!("cognicode-mcp-server exited early with {status}\nstderr:\n{err}");
        }
        if http_get(addr, "/health", None).is_ok() {
            return Server { child, addr };
        }
        assert!(Instant::now() < deadline, "server never listened on {addr}");
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// One JSON-RPC POST to `/mcp`. Returns the session id the server assigned (if
/// any) and the first JSON message of the answer, which the server frames as
/// an event stream. A notification has no answer, so `None` comes back.
fn post(server: &Server, session: Option<&str>, body: &Value) -> (Option<String>, Option<Value>) {
    let payload = body.to_string();
    let mut stream = TcpStream::connect(server.addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .expect("timeout");
    let session_header = session
        .map(|id| format!("Mcp-Session-Id: {id}\r\n"))
        .unwrap_or_default();
    let request = format!(
        "POST /mcp HTTP/1.1\r\nHost: localhost\r\n\
         Content-Type: application/json\r\n\
         Accept: application/json, text/event-stream\r\n\
         {session_header}Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(request.as_bytes()).expect("send");

    let mut reader = BufReader::new(stream);
    let mut session_id = None;
    let mut line = String::new();
    // Status line and headers.
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some((key, value)) = trimmed.split_once(':')
            && key.eq_ignore_ascii_case("mcp-session-id")
        {
            session_id = Some(value.trim().to_string());
        }
    }
    // Body: the first `data:` line that parses as JSON is the answer.
    let mut answer = None;
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let trimmed = line.trim();
                let candidate = trimmed
                    .strip_prefix("data:")
                    .map(str::trim)
                    .unwrap_or(trimmed);
                if let Ok(value) = serde_json::from_str::<Value>(candidate)
                    && value.get("jsonrpc").is_some()
                {
                    answer = Some(value);
                    break;
                }
            }
        }
    }
    (session_id, answer)
}

fn open_session(server: &Server) -> String {
    let (session, answer) = post(
        server,
        None,
        &json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "read-only-probe", "version": "0"}
            }
        }),
    );
    assert!(answer.is_some(), "initialize must be answered");
    let session = session.expect("the server assigns a session id");
    post(
        server,
        Some(&session),
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    );
    session
}

fn listed_tools(server: &Server) -> Vec<String> {
    let session = open_session(server);
    let mut names = Vec::new();
    let mut cursor: Option<String> = None;
    // `tools/list` is paginated: stop only when the server stops giving a cursor.
    for id in 2..100 {
        let params = match &cursor {
            Some(cursor) => json!({"cursor": cursor}),
            None => json!({}),
        };
        let (_, answer) = post(
            server,
            Some(&session),
            &json!({"jsonrpc": "2.0", "id": id, "method": "tools/list", "params": params}),
        );
        let answer = answer.expect("tools/list must be answered");
        names.extend(
            answer["result"]["tools"]
                .as_array()
                .expect("tools array")
                .iter()
                .map(|tool| tool["name"].as_str().expect("tool name").to_string()),
        );
        cursor = answer["result"]["nextCursor"].as_str().map(str::to_string);
        if cursor.is_none() {
            return names;
        }
    }
    panic!("tools/list never ended its pagination");
}

const MUTATING: [&str; 2] = ["write_file", "edit_file"];

#[test]
fn without_the_flag_the_http_server_offers_the_mutating_tools() {
    let server = start(false);
    let tools = listed_tools(&server);
    for name in MUTATING {
        assert!(tools.iter().any(|t| t == name), "{name} missing: {tools:?}");
    }
}

#[test]
fn with_the_flag_the_http_server_hides_the_mutating_tools() {
    let server = start(true);
    let tools = listed_tools(&server);
    assert!(
        !tools.is_empty(),
        "a read-only server still lists the read tools"
    );
    for name in MUTATING {
        assert!(
            !tools.iter().any(|t| t == name),
            "{name} must not be offered in read-only: {tools:?}"
        );
    }
}

#[test]
fn with_the_flag_the_http_server_refuses_a_mutating_call() {
    let server = start(true);
    let session = open_session(&server);
    let (_, answer) = post(
        &server,
        Some(&session),
        &json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "write_file", "arguments": {"path": "x.txt", "content": "x"}}
        }),
    );
    let answer = answer.expect("the call must be answered");
    let refused =
        answer.get("error").is_some() || answer["result"]["isError"].as_bool().unwrap_or(false);
    assert!(
        refused,
        "a read-only server must refuse write_file: {answer}"
    );
}
