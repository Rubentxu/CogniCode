use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::Mutex;
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: i64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

struct StdioPair {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

pub struct JsonRpcTransport {
    next_id: AtomicI64,
    io: Mutex<StdioPair>,
}

impl JsonRpcTransport {
    pub fn new(stdin: ChildStdin, stdout: ChildStdout) -> Self {
        Self {
            next_id: AtomicI64::new(1),
            io: Mutex::new(StdioPair {
                stdin,
                stdout: BufReader::new(stdout),
            }),
        }
    }

    pub async fn send_request(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<JsonRpcResponse, JsonRpcTransportError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let body = serde_json::to_string(&request)
            .map_err(|e| JsonRpcTransportError::Serialization(e.to_string()))?;

        let mut io = self.io.lock().await;
        Self::write_message(&mut io.stdin, &body).await?;
        Self::read_response(&mut io.stdout, id).await
    }

    pub async fn send_notification(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), JsonRpcTransportError> {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };

        let body = serde_json::to_string(&notification)
            .map_err(|e| JsonRpcTransportError::Serialization(e.to_string()))?;

        let mut io = self.io.lock().await;
        Self::write_message(&mut io.stdin, &body).await
    }

    async fn write_message(
        stdin: &mut ChildStdin,
        body: &str,
    ) -> Result<(), JsonRpcTransportError> {
        let header = format!("Content-Length: {}\r\n\r\n", body.len());

        stdin
            .write_all(header.as_bytes())
            .await
            .map_err(|e| JsonRpcTransportError::Io(e.to_string()))?;
        stdin
            .write_all(body.as_bytes())
            .await
            .map_err(|e| JsonRpcTransportError::Io(e.to_string()))?;
        stdin
            .flush()
            .await
            .map_err(|e| JsonRpcTransportError::Io(e.to_string()))?;

        debug!("LSP >> {}", body);
        Ok(())
    }

    async fn read_response(
        reader: &mut BufReader<ChildStdout>,
        expected_id: i64,
    ) -> Result<JsonRpcResponse, JsonRpcTransportError> {
        loop {
            let mut content_length: Option<usize> = None;
            let mut headers_done = false;

            while !headers_done {
                let mut line = String::new();
                reader
                    .read_line(&mut line)
                    .await
                    .map_err(|e| JsonRpcTransportError::Io(e.to_string()))?;

                if line == "\r\n" || line == "\n" {
                    headers_done = true;
                } else if let Some(len_str) = line.strip_prefix("Content-Length: ") {
                    content_length = Some(
                        len_str
                            .trim()
                            .trim_end_matches('\r')
                            .parse()
                            .map_err(|e: std::num::ParseIntError| {
                                JsonRpcTransportError::Protocol(format!(
                                    "Invalid Content-Length: {}",
                                    e
                                ))
                            })?,
                    );
                }
            }

            let len = content_length.ok_or_else(|| {
                JsonRpcTransportError::Protocol("Missing Content-Length header".to_string())
            })?;

            let mut buf = vec![0u8; len];
            reader
                .read_exact(&mut buf)
                .await
                .map_err(|e| JsonRpcTransportError::Io(e.to_string()))?;

            let body = String::from_utf8(buf)
                .map_err(|e| JsonRpcTransportError::Serialization(e.to_string()))?;

            debug!("LSP << {}", body);

            let response: JsonRpcResponse = serde_json::from_str(&body)
                .map_err(|e| JsonRpcTransportError::Serialization(e.to_string()))?;

            if response.id == Some(expected_id) {
                if let Some(ref error) = response.error {
                    return Err(JsonRpcTransportError::ServerError(
                        error.code,
                        error.message.clone(),
                    ));
                }
                return Ok(response);
            }

            if response.id.is_some() {
                warn!(
                    "Received response for unexpected id {:?}, expected {}",
                    response.id, expected_id
                );
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum JsonRpcTransportError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Server error (code {0}): {1}")]
    ServerError(i64, String),
    #[error("Unexpected EOF")]
    UnexpectedEof,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_request_serialization() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "processId": null,
                "rootUri": "/tmp/test"
            })),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"initialize\""));
    }

    #[test]
    fn test_json_rpc_response_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, Some(1));
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_json_rpc_error_response() {
        let json = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert!(response.result.is_none());
        let error = response.error.unwrap();
        assert_eq!(error.code, -32600);
        assert_eq!(error.message, "Invalid Request");
    }

    #[test]
    fn test_content_length_header_format() {
        let body = r#"{"jsonrpc":"2.0","id":1}"#;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        assert!(header.starts_with("Content-Length: "));
        assert!(header.contains("\r\n\r\n"));
    }

    #[test]
    fn test_json_rpc_notification_serialization() {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "initialized".to_string(),
            params: None,
        };
        let json = serde_json::to_string(&notif).unwrap();
        assert!(!json.contains("\"id\""));
        assert!(json.contains("\"method\":\"initialized\""));
    }

    // Task 4.4: JsonRpc framing tests

    #[test]
    fn test_json_rpc_request_with_params() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 42,
            method: "textDocument/hover".to_string(),
            params: Some(serde_json::json!({
                "textDocument": { "uri": "file:///test.rs" },
                "position": { "line": 10, "character": 5 }
            })),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"id\":42"));
        assert!(json.contains("\"method\":\"textDocument/hover\""));
        assert!(json.contains("\"uri\""));
    }

    #[test]
    fn test_json_rpc_error_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request","data":null}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert!(response.error.is_some());
        let err = response.error.unwrap();
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid Request");
    }

    #[test]
    fn test_json_rpc_request_no_params_omitted() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 7,
            method: "shutdown".to_string(),
            params: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        // params field should be omitted when None due to skip_serializing_if
        assert!(!json.contains("\"params\""));
        assert!(json.contains("\"method\":\"shutdown\""));
    }

    #[test]
    fn test_json_rpc_response_no_result_or_error() {
        // A response with neither result nor error (valid per LSP spec for null result)
        let json = r#"{"jsonrpc":"2.0","id":5,"result":null}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, Some(5));
        assert!(response.error.is_none());
    }
}
