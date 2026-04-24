//! DAP Client - communicates with debug adapters via JSON over stdio
//!
//! The Debug Adapter Protocol uses JSON messages with Content-Length headers.
//! Each message has a header "Content-Length: <bytes>" followed by "\r\n\r\n" and then the body.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command, ChildStdin, ChildStdout};
use tokio::sync::RwLock;
use tokio::time::Duration;

use crate::error::{DebugError, Result};

/// DAP Client - connects to a debug adapter and sends/receives messages
pub struct DapClient {
    /// The child process running the debug adapter
    child: Child,

    /// Standard input to the adapter
    stdin: RwLock<ChildStdin>,

    /// Buffered reader for stdout
    reader: RwLock<DapReader>,
}

/// DAP message reader that handles Content-Length protocol
struct DapReader {
    inner: BufReader<ChildStdout>,
}

impl DapReader {
    /// Read a single DAP message (response or event)
    async fn read_message(&mut self) -> Result<serde_json::Value> {
        // Read the Content-Length header line
        let mut header_line = String::new();
        self.inner.read_line(&mut header_line).await.map_err(DebugError::Io)?;

        if !header_line.starts_with("Content-Length:") {
            return Err(DebugError::Protocol(format!(
                "Expected Content-Length header, got: {}",
                header_line.trim()
            )));
        }

        let content_length = header_line
            .strip_prefix("Content-Length:")
            .ok_or_else(|| DebugError::Protocol("Invalid Content-Length".to_string()))?
            .trim()
            .parse::<usize>()
            .map_err(|e| DebugError::Protocol(format!("Invalid content length: {}", e)))?;

        // Read the empty line (\r\n\r\n)
        let mut empty_line = [0u8; 2];
        self.inner.read_exact(&mut empty_line).await.map_err(DebugError::Io)?;

        if &empty_line != b"\r\n" {
            return Err(DebugError::Protocol("Expected \\r\\n separator".to_string()));
        }

        // Read the JSON body
        let mut body = vec![0u8; content_length];
        self.inner.read_exact(&mut body).await.map_err(DebugError::Io)?;

        let json_str = String::from_utf8(body)
            .map_err(|e| DebugError::Protocol(format!("Invalid UTF-8 in response: {}", e)))?;

        let message: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| DebugError::Protocol(format!("Invalid JSON from adapter: {}", e)))?;

        Ok(message)
    }
}

impl DapClient {
    /// Connect to a debug adapter at the given path
    pub async fn connect(adapter_path: &PathBuf) -> Result<Self> {
        let mut child = Command::new(adapter_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| DebugError::ConnectionFailed(format!("Failed to spawn adapter: {}", e)))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| DebugError::ConnectionFailed("Failed to capture stdin".to_string()))?;

        let stdout = child.stdout.take()
            .ok_or_else(|| DebugError::ConnectionFailed("Failed to capture stdout".to_string()))?;

        let reader = DapReader {
            inner: BufReader::new(stdout),
        };

        Ok(Self {
            child,
            stdin: RwLock::new(stdin),
            reader: RwLock::new(reader),
        })
    }

    /// Send a DAP request and wait for response
    async fn send_request(&self, command: &str, args: serde_json::Value) -> Result<serde_json::Value> {
        let request = serde_json::json!({
            "command": command,
            "arguments": args,
            "type": "request",
            "seq": 1
        });

        let json_str = serde_json::to_string(&request)
            .map_err(|e| DebugError::Serialization(e.to_string()))?;

        let msg = format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str);

        let mut stdin = self.stdin.write().await;
        stdin.write_all(msg.as_bytes()).await.map_err(DebugError::Io)?;
        stdin.flush().await.map_err(DebugError::Io)?;

        // Read response
        let mut reader = self.reader.write().await;
        let response = reader.read_message().await?;

        // Check for error responses
        if let Some(success) = response.get("success").and_then(|v| v.as_bool()) {
            if !success {
                let message = response.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");
                return Err(DebugError::AdapterError(message.to_string()));
            }
        }

        Ok(response)
    }

    /// Send a raw DAP message (for events/commands without response)
    async fn send_raw(&self, msg: &str) -> Result<()> {
        let mut stdin = self.stdin.write().await;
        stdin.write_all(msg.as_bytes()).await.map_err(DebugError::Io)?;
        stdin.write_all(b"\n").await.map_err(DebugError::Io)?;
        stdin.flush().await.map_err(DebugError::Io)?;
        Ok(())
    }

    /// Initialize the debug session
    pub async fn initialize(&self) -> Result<Capabilities> {
        let args = serde_json::json!({
            "adapterId": "rcode-debug",
            "clientId": "rcode",
            "clientName": "CogniCode",
            "locale": "en"
        });

        let response = self.send_request("initialize", args).await?;

        // Extract capabilities from response body
        let capabilities = response.get("body")
            .and_then(|b| b.get("capabilities"))
            .map(|c| serde_json::from_value(c.clone()))
            .transpose()
            .map_err(|e| DebugError::Serialization(e.to_string()))?
            .unwrap_or(Capabilities::default());

        Ok(capabilities)
    }

    /// Launch the debuggee
    pub async fn launch(&self, config: &LaunchConfig) -> Result<()> {
        let args = serde_json::json!({
            "noDebug": config.no_debug,
            "program": config.program,
            "args": config.args,
            "cwd": config.cwd,
            "env": config.env,
        });

        self.send_request("launch", args).await?;
        Ok(())
    }

    /// Set breakpoints
    pub async fn set_breakpoints(&self, source: &str, lines: &[u32]) -> Result<Vec<BreakpointStatus>> {
        let args = serde_json::json!({
            "source": { "path": source },
            "breakpoints": lines.iter().map(|l| { serde_json::json!({ "line": l }) }).collect::<Vec<_>>(),
        });

        let response = self.send_request("setBreakpoints", args).await?;

        let breakpoints = response.get("body")
            .and_then(|b| b.get("breakpoints"))
            .map(|bp| {
                serde_json::from_value(bp.clone())
                    .map_err(|e| DebugError::Serialization(e.to_string()))
            })
            .ok_or_else(|| DebugError::Protocol("No breakpoints in response".to_string()))??;

        Ok(breakpoints)
    }

    /// Configuration done
    pub async fn configuration_done(&self) -> Result<()> {
        self.send_request("configurationDone", serde_json::json!({})).await?;
        Ok(())
    }

    /// Pause execution
    #[allow(dead_code)]
    pub async fn pause(&self, thread_id: Option<i64>) -> Result<()> {
        let args = serde_json::json!({
            "threadId": thread_id
        });
        self.send_request("pause", args).await?;
        Ok(())
    }

    /// Continue execution
    #[allow(dead_code)]
    pub async fn continue_(&self, thread_id: Option<i64>) -> Result<StoppedEvent> {
        let args = serde_json::json!({
            "threadId": thread_id.unwrap_or(1)
        });

        let _response = self.send_request("continue", args).await?;

        // Return a stopped event since we're waiting for the next stop
        Ok(StoppedEvent {
            thread_id: thread_id.unwrap_or(1),
            reason: "breakpoint".to_string(),
            all_threads_stopped: true,
            description: None,
            text: None,
        })
    }

    /// Get stack trace
    #[allow(dead_code)]
    pub async fn stack_trace(&self, thread_id: Option<i64>, levels: Option<i64>) -> Result<Vec<StackFrame>> {
        let args = serde_json::json!({
            "threadId": thread_id.unwrap_or(1),
            "levels": levels.unwrap_or(100)
        });

        let response = self.send_request("stackTrace", args).await?;

        let frames = response.get("body")
            .and_then(|b| b.get("stackFrames"))
            .map(|sf| {
                serde_json::from_value(sf.clone())
                    .map_err(|e| DebugError::Serialization(e.to_string()))
            })
            .ok_or_else(|| DebugError::Protocol("No stackFrames in response".to_string()))??;

        Ok(frames)
    }

    /// Get variables
    #[allow(dead_code)]
    pub async fn variables(&self, variables_reference: i64) -> Result<Vec<Variable>> {
        let args = serde_json::json!({
            "variablesReference": variables_reference
        });

        let response = self.send_request("variables", args).await?;

        let vars = response.get("body")
            .and_then(|b| b.get("variables"))
            .map(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| DebugError::Serialization(e.to_string()))
            })
            .ok_or_else(|| DebugError::Protocol("No variables in response".to_string()))??;

        Ok(vars)
    }

    /// Evaluate an expression
    #[allow(dead_code)]
    pub async fn evaluate(&self, expr: &str, frame_id: Option<i64>, context: Option<&str>) -> Result<EvalResult> {
        let args = serde_json::json!({
            "expression": expr,
            "frameId": frame_id,
            "context": context.unwrap_or("watch")
        });

        let response = self.send_request("evaluate", args).await?;

        let result = response.get("body")
            .map(|b| {
                serde_json::from_value(b.clone())
                    .map_err(|e| DebugError::Serialization(e.to_string()))
            })
            .ok_or_else(|| DebugError::Protocol("No body in evaluate response".to_string()))??;

        Ok(result)
    }

    /// Wait for the next DAP event
    #[allow(dead_code)]
    pub async fn wait_for_event(&mut self, timeout_duration: Duration) -> Result<DapEvent> {
        use tokio::time::timeout;

        let result = timeout(timeout_duration, async {
            let mut reader = self.reader.write().await;
            let value = reader.read_message().await?;
            let event: DapEvent = serde_json::from_value(value)
                .map_err(|e| DebugError::Serialization(e.to_string()))?;
            Ok::<DapEvent, DebugError>(event)
        })
        .await;

        match result {
            Ok(Ok(event)) => Ok(event),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(DebugError::Timeout("Timed out waiting for debug event".to_string())),
        }
    }

    /// Disconnect and terminate the adapter
    #[allow(dead_code)]
    pub async fn disconnect(&mut self) -> Result<()> {
        let _ = self.send_request("disconnect", serde_json::json!({})).await;
        self.child.kill().await.map_err(DebugError::Io)?;
        Ok(())
    }
}

// ============================================================================
// DAP Protocol Types
// ============================================================================

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            supports_configuration_done: true,
            supports_function_breakpoints: true,
            supports_conditional_breakpoints: true,
            supports_evaluate_for_hovers: true,
        }
    }
}

/// A DAP event
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DapEvent {
    pub event: String,
    #[serde(default)]
    pub body: serde_json::Value,
}

/// Stopped event body
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StoppedEvent {
    #[serde(rename = "threadId")]
    pub thread_id: i64,
    pub reason: String,
    #[serde(rename = "allThreadsStopped")]
    #[serde(default)]
    pub all_threads_stopped: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

/// Adapter capabilities
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Capabilities {
    #[serde(rename = "supportsConfigurationDoneRequest")]
    #[serde(default)]
    pub supports_configuration_done: bool,

    #[serde(rename = "supportsFunctionBreakpoints")]
    #[serde(default)]
    pub supports_function_breakpoints: bool,

    #[serde(rename = "supportsConditionalBreakpoints")]
    #[serde(default)]
    pub supports_conditional_breakpoints: bool,

    #[serde(rename = "supportsEvaluateForHovers")]
    #[serde(default)]
    pub supports_evaluate_for_hovers: bool,
}

/// Stack frame
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StackFrame {
    pub id: i64,
    pub name: String,
    pub line: u32,
    pub column: u32,
    #[serde(rename = "source")]
    #[serde(default)]
    pub source: Option<Source>,
}

/// Source file reference
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Source {
    pub path: Option<String>,
    pub name: Option<String>,
}

/// Variable
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Variable {
    pub name: String,
    #[serde(rename = "type")]
    #[serde(default)]
    pub type_: Option<String>,
    pub value: String,
    #[serde(rename = "variablesReference")]
    #[serde(default)]
    pub variables_reference: Option<i64>,
    #[serde(rename = "namedVariables")]
    #[serde(default)]
    pub named_variables: Option<i64>,
    #[serde(rename = "indexedVariables")]
    #[serde(default)]
    pub indexed_variables: Option<i64>,
    #[serde(rename = "presentationHint")]
    #[serde(default)]
    pub presentation_hint: Option<VariablePresentationHint>,
}

/// Presentation hint for a variable
#[derive(Debug, Clone, serde::Deserialize)]
pub struct VariablePresentationHint {
    pub kind: Option<String>,
    pub attributes: Option<Vec<String>>,
    pub visibility: Option<String>,
}

/// Evaluation result
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EvalResult {
    pub result: String,
    #[serde(rename = "type")]
    #[serde(default)]
    pub type_: Option<String>,
    #[serde(rename = "variablesReference")]
    #[serde(default)]
    pub variables_reference: Option<i64>,
}

/// Breakpoint status
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BreakpointStatus {
    pub id: Option<i64>,
    pub verified: bool,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub source: Option<Source>,
    pub line: Option<u32>,
}

/// Launch configuration
#[derive(Debug, Clone)]
pub struct LaunchConfig {
    /// Program to debug
    pub program: String,
    /// Arguments to the program
    pub args: Vec<String>,
    /// Working directory
    pub cwd: Option<String>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Skip debugging
    pub no_debug: bool,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            program: String::new(),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            no_debug: false,
        }
    }
}

impl LaunchConfig {
    /// Create a new launch config with required program
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            ..Default::default()
        }
    }

    /// Set program arguments
    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Set working directory
    pub fn cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }
}
