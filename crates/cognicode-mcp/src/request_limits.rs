//! Limits on what a client can make the HTTP transport read.
//!
//! The streamable HTTP transport reads the whole body of a request before it
//! decides what to do with it. Without a ceiling, a client can make the
//! server hold as much memory as it cares to send; without a deadline, a
//! client that announces a body and never sends it holds a connection and a
//! task for as long as it keeps the socket open. Both limits are enforced
//! here, before the body reaches the transport.
//!
//! What this does not bound: the time a client takes to send the request
//! *headers*. `axum::serve` runs hyper without a timer, so a connection that
//! never finishes its headers is held until the client closes it.

use std::time::Duration;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use http_body_util::{BodyExt, LengthLimitError, Limited};

/// The largest request body the transport reads: 4 MiB.
///
/// A JSON-RPC request names a tool and its arguments; the largest legitimate
/// one is a `write_file` with the content of a file, and a source file larger
/// than this is not something an agent sends through a tool call.
pub const DEFAULT_MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

/// How long the transport waits for a request body to arrive whole: 30 s.
pub const DEFAULT_BODY_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// The ceiling and the deadline a request body is read under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestLimits {
    /// A body longer than this is refused with `413 Payload Too Large`.
    pub max_body_bytes: usize,
    /// A body that has not arrived whole by then is refused with
    /// `408 Request Timeout`, and the connection is closed.
    pub body_read_timeout: Duration,
}

impl Default for RequestLimits {
    fn default() -> Self {
        Self {
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            body_read_timeout: DEFAULT_BODY_READ_TIMEOUT,
        }
    }
}

/// Middleware that reads the request body under `limits` and hands it on.
///
/// The body is read whole here, so the handler behind receives a body that
/// is already in memory and within the ceiling. Three refusals, each with
/// `Connection: close`, because a connection whose body was not read to the
/// end cannot carry another request:
///
/// - `413` when the declared `Content-Length` exceeds the ceiling, before a
///   byte of the body is read;
/// - `413` when the body grows past the ceiling while it arrives (a chunked
///   body declares no length);
/// - `408` when the body has not arrived whole by the deadline.
pub async fn enforce(
    State(limits): State<RequestLimits>,
    request: Request,
    next: Next,
) -> Response {
    if declared_length(request.headers())
        .is_some_and(|length| length > limits.max_body_bytes as u64)
    {
        return refuse(StatusCode::PAYLOAD_TOO_LARGE, too_large(limits));
    }

    let (parts, body) = request.into_parts();
    let read = tokio::time::timeout(
        limits.body_read_timeout,
        Limited::new(body, limits.max_body_bytes).collect(),
    )
    .await;
    match read {
        Err(_) => refuse(
            StatusCode::REQUEST_TIMEOUT,
            format!(
                "the request body did not arrive within {:?}",
                limits.body_read_timeout
            ),
        ),
        Ok(Err(error)) if error.downcast_ref::<LengthLimitError>().is_some() => {
            refuse(StatusCode::PAYLOAD_TOO_LARGE, too_large(limits))
        }
        Ok(Err(_)) => refuse(
            StatusCode::BAD_REQUEST,
            "the request body could not be read".to_string(),
        ),
        Ok(Ok(collected)) => {
            next.run(Request::from_parts(parts, Body::from(collected.to_bytes())))
                .await
        }
    }
}

/// The length the client declared for the body, when it declared one that
/// can be read.
fn declared_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn too_large(limits: RequestLimits) -> String {
    format!(
        "the request body exceeds the limit of {} bytes",
        limits.max_body_bytes
    )
}

fn refuse(status: StatusCode, message: String) -> Response {
    (status, [(header::CONNECTION, "close")], message).into_response()
}
