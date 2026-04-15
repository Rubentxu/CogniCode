//! MCP Lifecycle Core — extracted and reusable MCP client logic
//!
//! This module provides the shared MCP client functionality used by both
//! `mcp_client.rs` (preserved for backward compatibility) and `sandbox_orchestrator`.
//!
//! Handles: server spawn, initialize handshake, request/response correlation,
//! notification separation, stdout contamination filtering, and timeouts.

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

/// Errors that can occur during MCP lifecycle operations.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("server binary not found at {0}")]
    ServerNotFound(PathBuf),
    #[error("failed to spawn server: {0}")]
    SpawnError(String),
    #[error("timeout waiting for response with id={0} after {1}s")]
    Timeout(u64, u64),
    #[error("server closed stdout (EOF)")]
    Eof,
    #[error("server closed stdout (EOF) with stderr: {0}")]
    EofWithStderr(String),
    #[error("server emitted non-JSON line (protocol violation): {0}")]
    ProtocolViolation(String),
    #[error("MCP error response: {0}")]
    JsonRpcError(Value),
    #[error("failed to write to server stdin: {0}")]
    WriteError(String),
    #[error("IO error reading server stdout: {0}")]
    ReadError(String),
    #[error("server returned error code {0}")]
    ServerExitCode(i32),
}

impl From<std::io::Error> for McpError {
    fn from(e: std::io::Error) -> Self {
        McpError::SpawnError(e.to_string())
    }
}

/// Handle to a running MCP server process.
pub struct McpServer {
    child: Child,
    stdout: BufReader<ChildStdout>,
    stdin: std::process::ChildStdin,
    stderr: Option<ChildStderr>,
    request_id: u64,
}

/// Type alias for clarity
type ChildStdout = std::process::ChildStdout;
/// Type alias for child stderr
type ChildStderr = std::process::ChildStderr;

impl McpServer {
    /// Spawn a new MCP server process.
    ///
    /// - `server_path`: path to the cognicode-mcp binary
    /// - `workspace`: workspace directory passed as --cwd
    pub fn spawn(server_path: &PathBuf, workspace: &PathBuf) -> Result<Self, McpError> {
        Self::spawn_with_env(server_path, workspace, &[], &[])
    }

    /// Spawn with additional environment variables and arguments.
    pub fn spawn_with_env(
        server_path: &PathBuf,
        workspace: &PathBuf,
        env: &[(String, String)],
        extra_args: &[&str],
    ) -> Result<Self, McpError> {
        if !server_path.exists() {
            return Err(McpError::ServerNotFound(server_path.clone()));
        }

        let mut cmd = Command::new(server_path);
        cmd.arg("--cwd").arg(workspace);
        for arg in extra_args {
            cmd.arg(arg);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }

        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| McpError::SpawnError(e.to_string()))?;

        let stdin = child.stdin.take().expect("no stdin");
        let stdout = child.stdout.take().expect("no stdout");
        let stderr = child.stderr.take();

        Ok(Self {
            child,
            stdout: BufReader::new(stdout),
            stdin,
            stderr,
            request_id: 0,
        })
    }

    /// Perform the MCP handshake: send initialize, read response, send initialized notification.
    pub fn initialize(
        &mut self,
        protocol_version: &str,
        timeout_secs: u64,
    ) -> Result<Value, McpError> {
        self.request_id = 1;
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "initialize",
            "params": {
                "protocolVersion": protocol_version,
                "capabilities": {},
                "clientInfo": {
                    "name": "cognicode-sandbox-orchestrator",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });

        self.send_raw(&init_request)?;
        let resp = self.read_response(self.request_id, timeout_secs)?;
        self.request_id += 1;

        // Send initialized notification (no response expected)
        let notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        self.send_raw(&notif)?;

        Ok(resp)
    }

    /// Send a JSON-RPC request and wait for its response.
    ///
    /// `method` is the RPC method name, `params` is the params object.
    /// Returns the full JSON-RPC response Value, or an error if the server
    /// returned an error or we timed out.
    pub fn call(
        &mut self,
        method: &str,
        params: Value,
        timeout_secs: u64,
    ) -> Result<Value, McpError> {
        self.request_id += 1;
        let id = self.request_id;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        self.send_raw(&request)?;
        let response = self.read_response(id, timeout_secs)?;

        // Check for JSON-RPC error
        if let Some(err_obj) = response.get("error") {
            return Err(McpError::JsonRpcError(err_obj.clone()));
        }

        // Return full response for artifact capture; callers extract result as needed
        Ok(response)
    }

    /// Send a JSON-RPC notification (no response expected).
    pub fn notify(&mut self, method: &str, params: Value) -> Result<(), McpError> {
        let notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        self.send_raw(&notif)
    }

    /// Read all notifications from stdout, returning them and draining the buffer.
    /// Call this after `call()` to drain any notifications that arrived.
    /// Uses a short timeout to avoid blocking indefinitely if no notifications arrive.
    pub fn drain_notifications(&mut self) -> Vec<Value> {
        let mut notifications = Vec::new();
        let deadline = Instant::now() + Duration::from_millis(100); // 100ms max

        loop {
            if Instant::now() > deadline {
                break; // Timeout - stop waiting for more notifications
            }

            // read_line() blocks until a newline is received
            let mut line = String::new();
            let read_result = self.stdout.read_line(&mut line);

            match read_result {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    if let Ok(msg) = serde_json::from_str::<Value>(trimmed) {
                        if msg.get("id").is_none() {
                            notifications.push(msg);
                        }
                    }
                }
                Err(_) => break, // Error reading, stop draining
            }
        }
        notifications
    }

    /// Send a raw JSON-RPC message (newline-delimited JSON).
    fn send_raw(&mut self, msg: &Value) -> Result<(), McpError> {
        let mut line =
            serde_json::to_string(msg).map_err(|e| McpError::WriteError(e.to_string()))?;
        line.push('\n');
        std::io::Write::write_all(&mut self.stdin, line.as_bytes())
            .map_err(|e| McpError::WriteError(e.to_string()))?;
        std::io::Write::flush(&mut self.stdin).map_err(|e| McpError::WriteError(e.to_string()))?;
        Ok(())
    }

    /// Drain any available stderr content from the child process.
    /// Returns the captured stderr as a string, or empty string if no stderr was captured.
    fn drain_stderr(&mut self) -> String {
        if let Some(ref mut stderr) = self.stderr {
            let mut buf = String::new();
            // Use read_to_string to drain the stderr pipe
            if let Ok(n) = stderr.read_to_string(&mut buf) {
                if n > 0 {
                    return buf;
                }
            }
        }
        String::new()
    }

    /// Read responses until we find one with the expected id.
    /// Returns the full response Value (including jsonrpc, id, result/error).
    fn read_response(&mut self, expected_id: u64, timeout_secs: u64) -> Result<Value, McpError> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);

        loop {
            if Instant::now() > deadline {
                return Err(McpError::Timeout(expected_id, timeout_secs));
            }

            // read_line() blocks until a newline is received
            // but ANSI log lines from the server provide frequent newlines
            let mut line = String::new();
            let read_result = self.stdout.read_line(&mut line);

            // Check deadline after blocking read
            if Instant::now() > deadline {
                return Err(McpError::Timeout(expected_id, timeout_secs));
            }

            match read_result {
                Ok(0) => {
                    // Server closed stdout — drain stderr to capture crash message
                    let stderr = self.drain_stderr();
                    if stderr.is_empty() {
                        return Err(McpError::Eof);
                    } else {
                        return Err(McpError::EofWithStderr(stderr));
                    }
                }
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    let msg: Value = match serde_json::from_str(trimmed) {
                        Ok(v) => v,
                        Err(_) => {
                            // Non-JSON line (e.g., ANSI log from tracing), skip
                            continue;
                        }
                    };

                    // Skip notifications (no id field)
                    if msg.get("id").is_none() {
                        continue;
                    }

                    // Check if this response matches our expected id
                    let matches = msg
                        .get("id")
                        .map(|id_val| match id_val {
                            Value::Number(n) => n.as_u64() == Some(expected_id),
                            Value::String(s) => s == &expected_id.to_string(),
                            _ => false,
                        })
                        .unwrap_or(false);

                    if matches {
                        return Ok(msg);
                    }
                    // Not our response, keep reading
                }
                Err(e) => return Err(McpError::ReadError(e.to_string())),
            }
        }
    }

    /// Kill the server process.
    pub fn kill(&mut self) -> Result<(), std::io::Error> {
        self.child.kill()
    }

    /// Wait for the server to exit and return its exit status.
    pub fn wait(&mut self) -> Result<std::process::ExitStatus, std::io::Error> {
        self.child.wait()
    }

    /// Returns true if the child process has exited.
    pub fn is_dead(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_some()
    }
}

/// JSON-RPC 2.0 request/response pair captured for artifact storage.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CapturedCall {
    pub request: Value,
    pub response: Value,
    /// Notification messages received between request and response
    #[serde(default)]
    pub notifications: Vec<Value>,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_error_display() {
        let err = McpError::Timeout(42, 30);
        assert!(err.to_string().contains("42"));
        assert!(err.to_string().contains("30"));
    }

    #[test]
    fn test_captured_call_serde() {
        let call = CapturedCall {
            request: serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call"}),
            response: serde_json::json!({"jsonrpc": "2.0", "id": 1, "result": {}}),
            notifications: vec![],
            duration_ms: 50,
        };
        let json = serde_json::to_string(&call).unwrap();
        let parsed: CapturedCall = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.request["method"], "tools/call");
    }
}
