//! What the HTTP server, started for real, does with requests it must refuse.
//!
//! The rules live in the library (`exposure`, `request_limits`) and are
//! tested there. These checks cover the half that rots silently: a rule the
//! running server does not consult. Each one starts `cognicode-mcp-server`
//! over an empty workspace of its own and speaks HTTP to it over the wire.

use crate::common;

use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;

use serde_json::{Value, json};

use common::{HttpServer, OpenResponse, http_send};

/// The first message of a session.
pub(crate) fn initialize() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "posture", "version": "0"}
        }
    })
}

/// POST one JSON-RPC message to `/mcp` and read the status line and headers.
pub(crate) fn post(
    server: &HttpServer,
    host: &str,
    token: Option<&str>,
    session: Option<&str>,
    message: &Value,
) -> OpenResponse {
    let body = message.to_string();
    let mut headers = format!(
        "POST /mcp HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\n\
         Accept: application/json, text/event-stream\r\nContent-Length: {}\r\n",
        body.len()
    );
    if let Some(token) = token {
        headers.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    if let Some(session) = session {
        headers.push_str(&format!("Mcp-Session-Id: {session}\r\n"));
    }
    let request = format!("{headers}\r\n{body}");
    http_send(server.addr, request.as_bytes()).unwrap_or_else(|error| {
        panic!(
            "POST /mcp with Host `{host}`: {error}\nstderr:\n{}",
            server.stderr()
        )
    })
}

/// The next piece of a chunked body, or `None` once it ends.
fn next_chunk(reader: &mut BufReader<TcpStream>) -> Option<Vec<u8>> {
    let mut size_line = String::new();
    if reader.read_line(&mut size_line).ok()? == 0 {
        return None;
    }
    let size = usize::from_str_radix(size_line.trim().split(';').next()?, 16).ok()?;
    if size == 0 {
        return None;
    }
    let mut chunk = vec![0; size + 2];
    reader.read_exact(&mut chunk).ok()?;
    chunk.truncate(size);
    Some(chunk)
}

/// The JSON-RPC message with `id` in an answer of the MCP transport, which
/// arrives as server-sent events.
fn message_with_id(response: &mut OpenResponse, id: u64) -> Value {
    let chunked = response
        .header("transfer-encoding")
        .is_some_and(|value| value.eq_ignore_ascii_case("chunked"));
    let mut pending: Vec<u8> = Vec::new();
    loop {
        while let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line);
            if let Some(payload) = line.trim_end().strip_prefix("data:")
                && let Ok(message) = serde_json::from_str::<Value>(payload.trim())
                && message["id"] == id
            {
                return message;
            }
        }
        let piece = if chunked {
            next_chunk(&mut response.body)
        } else {
            let mut buffer = [0u8; 4096];
            match response.body.read(&mut buffer) {
                Ok(0) | Err(_) => None,
                Ok(read) => Some(buffer[..read].to_vec()),
            }
        };
        match piece {
            Some(bytes) => pending.extend_from_slice(&bytes),
            None => panic!(
                "the answer ended without the message with id {id}; unread: {}",
                String::from_utf8_lossy(&pending)
            ),
        }
    }
}

#[test]
fn a_body_declared_larger_than_the_ceiling_is_refused_without_being_read() {
    let workspace = tempfile::tempdir().expect("an empty workspace");
    let server = HttpServer::start(workspace.path(), None, &[]);

    // Five MiB announced and none of it sent: a server that reads first and
    // decides later holds this connection until the client gives up.
    let response = http_send(
        server.addr,
        b"POST /mcp HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\n\
          Accept: application/json, text/event-stream\r\nContent-Length: 5242880\r\n\r\n",
    )
    .unwrap_or_else(|error| {
        panic!(
            "the server did not refuse an oversized body: {error}\nstderr:\n{}",
            server.stderr()
        )
    });

    assert_eq!(response.status(), 413, "{}", response.status_line.trim());
}

#[test]
fn a_loopback_server_does_not_answer_to_the_unspecified_address() {
    let workspace = tempfile::tempdir().expect("an empty workspace");
    let server = HttpServer::start(workspace.path(), None, &[]);
    let port = server.addr.port();

    let refused = post(
        &server,
        &format!("0.0.0.0:{port}"),
        None,
        None,
        &initialize(),
    );
    assert_eq!(
        refused.status(),
        403,
        "a server bound to loopback answered a request addressed to 0.0.0.0: {}",
        refused.status_line.trim()
    );

    let answered = post(
        &server,
        &format!("127.0.0.1:{port}"),
        None,
        None,
        &initialize(),
    );
    assert_eq!(
        answered.status(),
        200,
        "the loopback name must keep working: {}",
        answered.status_line.trim()
    );
}

#[test]
fn a_read_only_server_refuses_to_write_and_writes_nothing() {
    let workspace = tempfile::tempdir().expect("an empty workspace");
    let server = HttpServer::start(workspace.path(), None, &["--read-only"]);
    let host = format!("127.0.0.1:{}", server.addr.port());

    let mut initialized = post(&server, &host, None, None, &initialize());
    assert_eq!(
        initialized.status(),
        200,
        "{}",
        initialized.status_line.trim()
    );
    let session = initialized
        .header("mcp-session-id")
        .expect("the transport opens a session")
        .to_string();
    let answer = message_with_id(&mut initialized, 1);
    assert!(
        answer.get("result").is_some(),
        "initialize failed: {answer}"
    );

    let notified = post(
        &server,
        &host,
        None,
        Some(&session),
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    );
    assert_eq!(notified.status(), 202, "{}", notified.status_line.trim());

    let target = "written-by-a-read-only-server.txt";
    let mut called = post(
        &server,
        &host,
        None,
        Some(&session),
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": "write_file", "arguments": {"path": target, "content": "x"}}
        }),
    );
    assert_eq!(called.status(), 200, "{}", called.status_line.trim());
    let answer = message_with_id(&mut called, 2);

    assert_eq!(
        answer["result"]["isError"], true,
        "a read-only server must refuse a write as an error: {answer}"
    );
    let said = answer["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        said.contains("read_only_mode"),
        "the refusal must say why: {answer}"
    );
    assert!(
        !workspace.path().join(target).exists(),
        "the read-only server wrote the file"
    );
}
