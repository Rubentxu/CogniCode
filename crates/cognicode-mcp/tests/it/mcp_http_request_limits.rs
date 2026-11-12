//! The ceiling and the deadline the HTTP transport reads a request body under.
//!
//! The middleware is mounted in front of a handler that reads whatever body
//! reaches it, and served over a real socket in this process. A handler that
//! reads without a limit is exactly what the transport behind the middleware
//! does, so what reaches it is what would reach the transport.
//!
//! The client writes the request by hand, because the requests that matter
//! here are the ones a well-behaved client never sends: a body announced and
//! never sent, a body longer than it is allowed to be.

use crate::common;

use std::io::{ErrorKind, Read};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use axum::{Router, extract::Request, middleware::from_fn_with_state, routing::post};
use cognicode_mcp::request_limits::{RequestLimits, enforce};

use common::{OpenResponse, http_send};

/// How long a refusal may take to arrive. The refusals are decided in
/// milliseconds; a check that waits longer is waiting on a server that held
/// the connection.
const ANSWER_WINDOW: Duration = Duration::from_secs(5);

/// A server running the middleware in front of a handler that reads it all.
struct EchoServer {
    addr: SocketAddr,
    _runtime: tokio::runtime::Runtime,
}

fn serve(limits: RequestLimits) -> EchoServer {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime for the server");
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .expect("bind a loopback port");
    let addr = listener.local_addr().expect("the bound address");
    let app = Router::new()
        .route("/echo", post(echo))
        .layer(from_fn_with_state(limits, enforce));
    runtime.spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    EchoServer {
        addr,
        _runtime: runtime,
    }
}

/// Reads the body with no limit of its own, and says how much it read.
async fn echo(request: Request) -> String {
    match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(bytes) => format!("read {} bytes", bytes.len()),
        Err(error) => format!("read failed: {error}"),
    }
}

/// Sends `request` and fails the check if no answer arrives in time.
fn send(server: &EchoServer, request: &str) -> OpenResponse {
    let started = Instant::now();
    let response = http_send(server.addr, request.as_bytes()).unwrap_or_else(|error| {
        panic!(
            "the server did not answer within {:?}: {error}",
            started.elapsed()
        )
    });
    assert!(
        started.elapsed() < ANSWER_WINDOW,
        "the answer took {:?}: the server held the connection instead of \
         refusing it",
        started.elapsed()
    );
    response
}

/// Reads what is left on the connection, failing if the server keeps it open.
fn rest_of(mut response: OpenResponse) -> String {
    let mut rest = Vec::new();
    match response.body.read_to_end(&mut rest) {
        Ok(_) => {}
        // A server that closes with bytes of the request still unread resets
        // the connection: closed is what is being checked, and it is closed.
        Err(error) if error.kind() == ErrorKind::ConnectionReset => {}
        Err(error) => panic!("the server kept the connection open after answering: {error}"),
    }
    String::from_utf8_lossy(&rest).into_owned()
}

#[test]
fn a_body_that_is_announced_and_never_sent_is_cut_off_at_the_deadline() {
    let server = serve(RequestLimits {
        max_body_bytes: 1024,
        body_read_timeout: Duration::from_millis(200),
    });

    let response = send(
        &server,
        "POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Length: 10\r\n\r\n",
    );

    assert_eq!(response.status(), 408, "{}", response.status_line.trim());
    assert_eq!(
        response.header("connection"),
        Some("close"),
        "a connection whose body never came is not reused"
    );
    rest_of(response);
}

#[test]
fn a_body_declared_longer_than_the_ceiling_is_refused_before_it_is_read() {
    let server = serve(RequestLimits {
        max_body_bytes: 1024,
        body_read_timeout: Duration::from_secs(60),
    });

    // The body is announced and never sent: only a refusal that trusts the
    // declared length answers before the deadline of the read.
    let response = send(
        &server,
        "POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1048576\r\n\r\n",
    );

    assert_eq!(response.status(), 413, "{}", response.status_line.trim());
    assert_eq!(response.header("connection"), Some("close"));
    rest_of(response);
}

#[test]
fn a_chunked_body_that_grows_past_the_ceiling_is_refused_while_it_arrives() {
    let server = serve(RequestLimits {
        max_body_bytes: 1024,
        body_read_timeout: Duration::from_secs(60),
    });

    // No length is declared, and the body never ends: the refusal has to
    // come from counting what arrives.
    let chunk = "a".repeat(2048);
    let response = send(
        &server,
        &format!(
            "POST /echo HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n\
             800\r\n{chunk}\r\n"
        ),
    );

    assert_eq!(response.status(), 413, "{}", response.status_line.trim());
    rest_of(response);
}

#[test]
fn a_body_within_the_ceiling_reaches_the_handler_whole() {
    let server = serve(RequestLimits::default());

    let response = send(
        &server,
        "POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
    );

    assert_eq!(response.status(), 200, "{}", response.status_line.trim());
    assert_eq!(rest_of(response), "read 5 bytes");
}

#[test]
fn a_body_exactly_at_the_ceiling_is_let_through() {
    let server = serve(RequestLimits {
        max_body_bytes: 5,
        body_read_timeout: Duration::from_secs(60),
    });

    let response = send(
        &server,
        "POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
    );

    assert_eq!(response.status(), 200, "{}", response.status_line.trim());
    assert_eq!(rest_of(response), "read 5 bytes");
}
