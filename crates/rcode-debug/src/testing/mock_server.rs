//! Mock DAP Server for testing
//!
//! A simple TCP server that implements the Debug Adapter Protocol subset
//! needed for testing DapClient.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// A mock DAP server that handles basic debug adapter protocol messages
pub struct MockDapServer {
    /// Port the server is listening on
    port: u16,
    /// Handle to the server thread
    handle: Option<thread::JoinHandle<()>>,
    /// Flag to signal shutdown
    shutdown: Arc<Mutex<bool>>,
}

impl MockDapServer {
    /// Start a mock DAP server on a random available port
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let shutdown = Arc::new(Mutex::new(false));
        let shutdown_for_handle = shutdown.clone();

        let handle = thread::spawn(move || {
            Self::run_server(listener, shutdown_for_handle);
        });

        // Give the server time to start
        thread::sleep(Duration::from_millis(50));

        Self {
            port,
            handle: Some(handle),
            shutdown,
        }
    }

    /// Get the port the server is listening on
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Stop the server
    pub fn stop(&mut self) {
        *self.shutdown.lock().unwrap() = true;
        // Connect to wake up the accept() call
        let _ = TcpStream::connect(format!("127.0.0.1:{}", self.port));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    fn run_server(listener: TcpListener, shutdown: Arc<Mutex<bool>>) {
        listener.set_nonblocking(false).ok();
        for stream in listener.incoming() {
            if *shutdown.lock().unwrap() {
                break;
            }
            match stream {
                Ok(mut stream) => {
                    if let Err(e) = Self::handle_connection(&mut stream) {
                        eprintln!("Mock DAP server error: {}", e);
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => {
                    eprintln!("Connection error: {}", e);
                }
            }
        }
    }

    fn handle_connection(stream: &mut TcpStream) -> std::io::Result<()> {
        let mut buffer = [0u8; 8192];
        let mut leftover = Vec::new();

        loop {
            let bytes_read = stream.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            leftover.extend_from_slice(&buffer[..bytes_read]);

            // Process all messages in the buffer
            while let Some(msg_end) = Self::find_message_end(&leftover) {
                let message_bytes = leftover.drain(..msg_end).collect::<Vec<_>>();
                let response = Self::process_message(&message_bytes);
                if !response.is_empty() {
                    stream.write_all(&response)?;
                    stream.flush()?;
                }
            }
        }

        Ok(())
    }

    /// Find the end of a DAP message (Content-Length header + body)
    fn find_message_end(buffer: &[u8]) -> Option<usize> {
        // Look for the empty line separator "\r\n\r\n"
        let separator = b"\r\n\r\n";
        let separator_pos = buffer.windows(4).position(|w| w == separator)?;

        // Parse Content-Length header
        let header = &buffer[..separator_pos];
        let header_str = String::from_utf8_lossy(header);

        if !header_str.starts_with("Content-Length:") {
            return None;
        }

        let content_length: usize = header_str
            .strip_prefix("Content-Length:")
            .and_then(|s| s.trim().parse().ok())?;

        let body_start = separator_pos + 4; // Skip \r\n\r\n
        let total_length = body_start + content_length;

        if buffer.len() >= total_length {
            Some(total_length)
        } else {
            None
        }
    }

    /// Process a DAP message and return the response
    fn process_message(input: &[u8]) -> Vec<u8> {
        // Parse the JSON part (skip headers)
        let body_start = input
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|p| p + 4)
            .unwrap_or(0);

        let json_str = String::from_utf8_lossy(&input[body_start..]);
        let request: serde_json::Value = match serde_json::from_str(&json_str) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Failed to parse request: {}", e);
                return Vec::new();
            }
        };

        let command = request.get("command").and_then(|v| v.as_str()).unwrap_or("");
        let seq = request.get("seq").and_then(|v| v.as_i64()).unwrap_or(1);

        let response = match command {
            "initialize" => Self::handle_initialize(seq),
            "launch" => Self::handle_launch(seq),
            "setBreakpoints" => Self::handle_set_breakpoints(seq, &request),
            "configurationDone" => Self::handle_configuration_done(seq),
            "continue" => Self::handle_continue(seq),
            "stackTrace" => Self::handle_stack_trace(seq),
            "variables" => Self::handle_variables(seq, &request),
            "evaluate" => Self::handle_evaluate(seq, &request),
            "disconnect" => Self::handle_disconnect(seq),
            _ => Self::make_response(seq, false, &format!("Unknown command: {}", command)),
        };

        let json_str = serde_json::to_string(&response).unwrap();
        format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str)
            .into_bytes()
    }

    fn handle_initialize(seq: i64) -> serde_json::Value {
        serde_json::json!({
            "type": "response",
            "request_seq": seq,
            "success": true,
            "command": "initialize",
            "body": {
                "capabilities": {
                    "supportsConfigurationDoneRequest": true,
                    "supportsFunctionBreakpoints": true,
                    "supportsConditionalBreakpoints": true,
                    "supportsEvaluateForHovers": true,
                    "supportsStepBack": false,
                    "supportsSetVariable": true,
                    "supportsRestartFrame": false,
                    "supportsGotoTargetsRequest": false,
                    "supportsStepInTargetsRequest": false,
                    "supportsCompletionsRequest": true,
                    "completionTriggerCharacters": [".", "["],
                    "supportsModulesRequest": false,
                    "additionalModuleColumns": [],
                    "supportedChecksumAlgorithms": [],
                    "supportsExecStatus": false
                }
            }
        })
    }

    fn handle_launch(seq: i64) -> serde_json::Value {
        serde_json::json!({
            "type": "response",
            "request_seq": seq,
            "success": true,
            "command": "launch"
        })
    }

    fn handle_set_breakpoints(seq: i64, request: &serde_json::Value) -> serde_json::Value {
        let breakpoints = request
            .get("arguments")
            .and_then(|a| a.get("breakpoints"))
            .and_then(|bp| bp.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|bp| bp.get("line").and_then(|l| l.as_u64()))
                    .map(|line| {
                        serde_json::json!({
                            "id": line as i64,
                            "verified": true,
                            "line": line
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        serde_json::json!({
            "type": "response",
            "request_seq": seq,
            "success": true,
            "command": "setBreakpoints",
            "body": {
                "breakpoints": breakpoints
            }
        })
    }

    fn handle_configuration_done(seq: i64) -> serde_json::Value {
        serde_json::json!({
            "type": "response",
            "request_seq": seq,
            "success": true,
            "command": "configurationDone"
        })
    }

    fn handle_continue(seq: i64) -> serde_json::Value {
        serde_json::json!({
            "type": "response",
            "request_seq": seq,
            "success": true,
            "command": "continue",
            "body": {
                "allThreadsStopped": true,
                "threads": [{
                    "id": 1,
                    "name": "main",
                    "reason": "breakpoint"
                }]
            }
        })
    }

    fn handle_stack_trace(seq: i64) -> serde_json::Value {
        serde_json::json!({
            "type": "response",
            "request_seq": seq,
            "success": true,
            "command": "stackTrace",
            "body": {
                "stackFrames": [
                    {
                        "id": 1,
                        "name": "main",
                        "source": {
                            "path": "/test/main.rs"
                        },
                        "line": 10,
                        "column": 1
                    },
                    {
                        "id": 2,
                        "name": "foo",
                        "source": {
                            "path": "/test/main.rs"
                        },
                        "line": 5,
                        "column": 1
                    }
                ],
                "totalFrames": 2
            }
        })
    }

    fn handle_variables(seq: i64, request: &serde_json::Value) -> serde_json::Value {
        let _ref = request
            .get("arguments")
            .and_then(|a| a.get("variablesReference"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        serde_json::json!({
            "type": "response",
            "request_seq": seq,
            "success": true,
            "command": "variables",
            "body": {
                "variables": [
                    {
                        "name": "x",
                        "type": "i32",
                        "value": "42",
                        "variablesReference": 0
                    },
                    {
                        "name": "s",
                        "type": "&str",
                        "value": "\"hello\"",
                        "variablesReference": 0
                    },
                    {
                        "name": "items",
                        "type": "Vec<i32>",
                        "value": "[1, 2, 3]",
                        "variablesReference": 100,
                        "namedVariables": 3
                    }
                ]
            }
        })
    }

    fn handle_evaluate(seq: i64, request: &serde_json::Value) -> serde_json::Value {
        let expr = request
            .get("arguments")
            .and_then(|a| a.get("expression"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let (result, type_) = match expr {
            "x" => ("42", "i32"),
            "s" => ("\"hello\"", "&str"),
            "items.len()" => ("3", "usize"),
            _ => (expr, "unknown"),
        };

        serde_json::json!({
            "type": "response",
            "request_seq": seq,
            "success": true,
            "command": "evaluate",
            "body": {
                "result": result,
                "type": type_,
                "variablesReference": 0
            }
        })
    }

    fn handle_disconnect(seq: i64) -> serde_json::Value {
        serde_json::json!({
            "type": "response",
            "request_seq": seq,
            "success": true,
            "command": "disconnect"
        })
    }

    fn make_response(seq: i64, success: bool, message: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "response",
            "request_seq": seq,
            "success": success,
            "command": "",
            "message": message
        })
    }
}

impl Drop for MockDapServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_server_start_stop() {
        let mut server = MockDapServer::start();
        let port = server.port();
        assert!(port > 0);
        server.stop();
    }

    #[tokio::test]
    async fn test_mock_server_with_client() {
        use std::path::PathBuf;

        let mut server = MockDapServer::start();

        // Note: This test requires the adapter to be a TCP client, not stdio
        // The current DapClient uses stdio, so we can't test it directly with MockDapServer
        // This is here for future TCP-based testing

        server.stop();
    }
}
