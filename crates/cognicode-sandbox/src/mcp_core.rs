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
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
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
///
/// Reads from the server's stdout are performed by a dedicated reader
/// thread spawned at construction time. The thread pushes complete lines
/// (or read errors) into a `mpsc::channel`; `read_response` and
/// `drain_notifications` poll the channel with `recv_timeout` so the
/// caller can enforce a deadline while the read itself is in progress.
///
/// This replaces the previous design where `read_line` was called
/// directly on the child's `ChildStdout`, which is a blocking syscall:
/// if the server stayed alive but emitted no further data, the read
/// blocked indefinitely and the deadline check before/after the read
/// never fired (P0 #1 in the sandbox harness audit).
pub struct McpServer {
    child: Child,
    line_rx: mpsc::Receiver<Result<String, std::io::Error>>,
    _reader_handle: Option<thread::JoinHandle<()>>,
    stdin: std::process::ChildStdin,
    stderr: Option<ChildStderr>,
    request_id: u64,
}

/// Type alias for clarity
type ChildStdout = std::process::ChildStdout;
/// Type alias for child stderr
type ChildStderr = std::process::ChildStderr;

impl McpServer {
    /// Construct from an already-spawned child process (used by sandbox container spawn).
    pub fn from_child(
        child: Child,
        stdin: std::process::ChildStdin,
        stdout: ChildStdout,
        stderr: Option<ChildStderr>,
    ) -> Self {
        let (_tx, line_rx, reader_handle) = spawn_reader_thread(stdout);
        Self {
            child,
            line_rx,
            _reader_handle: Some(reader_handle),
            stdin,
            stderr,
            request_id: 0,
        }
    }

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

        let stdin = child.stdin.take().ok_or_else(|| {
            McpError::SpawnError("failed to capture stdin from child process".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            McpError::SpawnError("failed to capture stdout from child process".into())
        })?;
        let stderr = child.stderr.take();

        let (_tx, line_rx, reader_handle) = spawn_reader_thread(stdout);
        Ok(Self {
            child,
            line_rx,
            _reader_handle: Some(reader_handle),
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
        let poll_interval = Duration::from_millis(20);
        // Forensic counter for non-JSON lines that arrive during the
        // drain window. The 100ms cap bounds the total, so this is
        // for visibility, not enforcement. Without it, a server
        // that emits non-protocol output during drain would lose
        // that signal entirely (P1 of the sandbox harness audit).
        let mut non_json_count: usize = 0;

        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let wait = poll_interval.min(remaining);
            match self.line_rx.recv_timeout(wait) {
                Ok(Ok(line)) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<Value>(trimmed) {
                        Ok(msg) => {
                            if msg.get("id").is_none() {
                                notifications.push(msg);
                            }
                        }
                        Err(_) => {
                            non_json_count += 1;
                            eprintln!(
                                "mcp_core: drain_notifications: non-JSON line #{}: {}",
                                non_json_count, trimmed
                            );
                        }
                    }
                }
                Ok(Err(_)) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => continue,
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
            if let Ok(n) = stderr.read_to_string(&mut buf)
                && n > 0
            {
                return buf;
            }
        }
        String::new()
    }

    /// Read responses until we find one with the expected id.
    /// Returns the full response Value (including jsonrpc, id, result/error).
    fn read_response(&mut self, expected_id: u64, timeout_secs: u64) -> Result<Value, McpError> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        // Poll interval is short relative to typical timeouts; the deadline
        // is checked at every iteration, so the wall-clock cost of a
        // full-timeout wait is bounded by (timeout + poll_interval).
        let poll_interval = Duration::from_millis(50);
        // Per-call counter for non-JSON lines. Occasional ANSI log
        // output (tracing, etc.) is tolerated; sustained non-JSON
        // output is a protocol violation. The pre-fix code silently
        // dropped every non-JSON line, so a server that does not
        // speak MCP at all could pass through undetected (P1 of the
        // sandbox harness audit).
        const MAX_NON_JSON_LINES: usize = 10;
        let mut non_json_count: usize = 0;

        loop {
            if Instant::now() > deadline {
                return Err(McpError::Timeout(expected_id, timeout_secs));
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            let wait = poll_interval.min(remaining);

            match self.line_rx.recv_timeout(wait) {
                Ok(Ok(line)) => {
                    // Check the deadline as soon as a line arrives; even
                    // one blocking read was enough to overshoot the
                    // budget in the pre-fix code.
                    if Instant::now() > deadline {
                        return Err(McpError::Timeout(expected_id, timeout_secs));
                    }

                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    let msg: Value = match serde_json::from_str(trimmed) {
                        Ok(v) => v,
                        Err(_) => {
                            // Non-JSON line. The pre-fix code silently
                            // continued, which meant a server that
                            // never speaks MCP would pass through with
                            // no signal. Count the occurrences and trip
                            // McpError::ProtocolViolation once the
                            // threshold is exceeded. A few ANSI log
                            // lines from tracing are still tolerated.
                            non_json_count += 1;
                            if non_json_count > MAX_NON_JSON_LINES {
                                return Err(McpError::ProtocolViolation(format!(
                                    "server emitted {} non-JSON lines in a single read_response (expected JSON-RPC)",
                                    non_json_count
                                )));
                            }
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
                Ok(Err(e)) => {
                    // The reader thread saw an IO error. EOF is
                    // signalled by UnexpectedEof; anything else is
                    // surfaced as a ReadError.
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        let stderr = self.drain_stderr();
                        if stderr.is_empty() {
                            return Err(McpError::Eof);
                        } else {
                            return Err(McpError::EofWithStderr(stderr));
                        }
                    }
                    return Err(McpError::ReadError(e.to_string()));
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    // Reader thread exited without sending an error.
                    // Most likely the process closed stdout and the
                    // thread's last send was treated as a normal
                    // termination.
                    let stderr = self.drain_stderr();
                    if stderr.is_empty() {
                        return Err(McpError::Eof);
                    } else {
                        return Err(McpError::EofWithStderr(stderr));
                    }
                }
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

/// Spawn a background reader thread that pulls complete lines (or read
/// errors) from the child's stdout and pushes them into the returned
/// `mpsc::Receiver`. The thread terminates when the pipe closes
/// (server exit) or the receiver is dropped (`McpServer` dropped).
///
/// Returns a tuple `(Sender, Receiver, JoinHandle)`; the `JoinHandle`
/// is the second field of the tuple, stored in `McpServer`. We do not
/// join it on `Drop` because the child's pipe closure already causes
/// the thread to exit on its own.
///
/// The thread is the only piece of code that calls blocking
/// `read_line` on the child's stdout. The `read_response` and
/// `drain_notifications` paths use `recv_timeout` on the receiver,
/// which is the deadline-enforcing primitive called for by P0 #1 of
/// the sandbox harness audit.
#[allow(clippy::type_complexity)]
// The return type is the natural shape of mpsc::channel + spawn:
// the writer end, the reader end, and the join handle. Extracting
// type aliases for a private helper would be over-engineering; the
// reader thread is called from one site and the alias would not
// improve readability.
fn spawn_reader_thread(
    stdout: ChildStdout,
) -> (
    mpsc::Sender<Result<String, std::io::Error>>,
    mpsc::Receiver<Result<String, std::io::Error>>,
    thread::JoinHandle<()>,
) {
    let (tx, rx) = mpsc::channel();
    let tx_clone = tx.clone();
    let handle = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    // EOF: signal and exit. The receiver treats
                    // UnexpectedEof as the EOF indication.
                    let _ = tx_clone.send(Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "server closed stdout",
                    )));
                    break;
                }
                Ok(_) => {
                    if tx_clone.send(Ok(line.clone())).is_err() {
                        // Receiver dropped (McpServer dropped). Exit.
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx_clone.send(Err(e));
                    break;
                }
            }
        }
    });
    (tx, rx, handle)
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

    /// Behavioral test for P0 #1 of the sandbox harness audit:
    /// read_response must enforce the deadline when the server is
    /// alive but emits no data. The pre-fix code called read_line
    /// directly on the child's stdout (a blocking syscall), so a
    /// silent server would block forever and the deadline check
    /// would never fire.
    ///
    /// The test spawns `cat` as the "server". cat opens stdout,
    /// reads from stdin, and never writes to stdout. With the
    /// pre-fix code, read_response would block indefinitely and
    /// the test would hang (or the test runner would time out
    /// externally). With the post-fix code, recv_timeout polls
    /// the channel with a 50ms tick and the deadline is checked
    /// at every iteration, so the call returns Timeout within
    /// `timeout + poll_interval` (here 1s + 50ms, so < 1.5s in
    /// practice).
    #[cfg(unix)]
    #[test]
    fn read_response_enforces_deadline_against_silent_server() {
        use std::process::{Command, Stdio};
        use std::time::Instant;

        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn cat as silent server");

        let stdin = child.stdin.take().expect("cat stdin");
        let stdout = child.stdout.take().expect("cat stdout");
        let stderr = child.stderr.take();

        let mut server = McpServer::from_child(child, stdin, stdout, stderr);

        let start = Instant::now();
        let result = server.read_response(1, 1);
        let elapsed = start.elapsed();

        // Cleanup: the child is still running. Kill it.
        let _ = server.kill();
        let _ = server.wait();

        let err = result.expect_err("read_response against silent server must return Err");
        assert!(
            matches!(err, McpError::Timeout(1, 1)),
            "expected Timeout(1, 1), got {:?}",
            err
        );
        // Budget = 1s timeout + 50ms poll interval + 200ms slack
        // for CI noise. Pre-fix this would block indefinitely.
        assert!(
            elapsed < std::time::Duration::from_millis(1250),
            "timeout took {:?}, expected < 1.25s",
            elapsed
        );
    }

    /// Behavioral test for P1 of the sandbox harness audit: a
    /// server that streams non-JSON output to stdout must trigger
    /// McpError::ProtocolViolation once the per-call threshold
    /// (10 lines) is exceeded. The pre-fix code silently dropped
    /// every non-JSON line, so a server that did not speak MCP
    /// at all could pass through with no signal.
    ///
    /// The test spawns `sh -c "yes noise | head -n 30"` as the
    /// "server". `yes` prints its argument repeatedly; `head -30`
    /// limits it to 30 lines. The server emits "noise\n" 30 times
    /// and closes. With the post-fix code, read_response should
    /// see 11+ non-JSON lines and return ProtocolViolation.
    #[cfg(unix)]
    #[test]
    fn read_response_protocol_violation_on_sustained_non_json() {
        use std::process::{Command, Stdio};
        use std::time::Instant;

        // Generate 30 lines of "noise" (not JSON-RPC) to stdout.
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("i=0; while [ $i -lt 30 ]; do echo 'noise is not json-rpc'; i=$((i+1)); done")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn noise generator");

        let stdin = child.stdin.take().expect("noise stdin");
        let stdout = child.stdout.take().expect("noise stdout");
        let stderr = child.stderr.take();

        let mut server = McpServer::from_child(child, stdin, stdout, stderr);

        let start = Instant::now();
        let result = server.read_response(42, 5);
        let elapsed = start.elapsed();

        let _ = server.kill();
        let _ = server.wait();

        let err = result.expect_err("sustained non-JSON must produce Err");
        match &err {
            McpError::ProtocolViolation(msg) => {
                // The threshold is 10; the message should report
                // at least 11 occurrences. Some lines may still
                // be in the channel buffer when we error, so
                // allow >= 11.
                assert!(
                    msg.contains("non-JSON"),
                    "ProtocolViolation message must mention non-JSON, got: {}",
                    msg
                );
            }
            other => panic!(
                "expected ProtocolViolation, got {:?} after {:?}",
                other, elapsed
            ),
        }
        // The server produces 30 lines quickly, then closes.
        // We should see the violation well within the 5s timeout.
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "protocol violation detection took {:?}, expected < 2s",
            elapsed
        );
    }
}
