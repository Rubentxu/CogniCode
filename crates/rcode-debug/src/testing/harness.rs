//! Test harness for rcode-debug integration tests

use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};

/// A test harness that spawns a debug adapter and validates the DAP protocol
pub struct TestHarness {
    /// Path to the debug adapter
    adapter_path: PathBuf,
    /// The spawned child process
    child: Option<Child>,
}

impl TestHarness {
    /// Create a new test harness with the given adapter
    pub fn new(adapter_path: PathBuf) -> Self {
        Self {
            adapter_path,
            child: None,
        }
    }

    /// Start the adapter process
    pub async fn start(&mut self) -> std::io::Result<()> {
        let mut child = Command::new(&self.adapter_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        self.child = Some(child);
        Ok(())
    }

    /// Stop the adapter process
    pub async fn stop(&mut self) -> std::io::Result<()> {
        if let Some(mut child) = self.child.take() {
            child.kill().await?;
        }
        Ok(())
    }

    /// Send a DAP request and read response
    pub async fn send_request(
        &mut self,
        command: &str,
        args: serde_json::Value,
    ) -> std::io::Result<serde_json::Value> {
        let child = self.child.as_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotConnected, "Adapter not started")
        })?;

        let stdin = child.stdin.as_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotConnected, "stdin not captured")
        })?;

        let stdout = child.stdout.as_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotConnected, "stdout not captured")
        })?;

        let request = serde_json::json!({
            "type": "request",
            "command": command,
            "arguments": args,
            "seq": 1
        });

        let json_str = serde_json::to_string(&request).unwrap();
        let msg = format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str);

        stdin.write_all(msg.as_bytes()).await?;
        stdin.flush().await?;

        // Read response headers
        let mut header_buf = vec![0u8; 256];
        let bytes_read = stdout.read(&mut header_buf).await?;
        let header_str = String::from_utf8_lossy(&header_buf[..bytes_read]);

        // Parse Content-Length
        let content_length = header_str
            .lines()
            .find(|l| l.starts_with("Content-Length:"))
            .and_then(|l| l.strip_prefix("Content-Length:"))
            .and_then(|l| l.trim().parse::<usize>().ok())
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid Content-Length header")
            })?;

        // Skip to body (after \r\n\r\n)
        let body_start = header_str
            .find("\r\n\r\n")
            .map(|p| p + 4)
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Missing header separator")
            })?;

        // Read body
        let mut body = header_buf[..bytes_read].to_vec();
        if body.len() < body_start + content_length {
            let mut additional = vec![0u8; body_start + content_length - body.len()];
            stdout.read(&mut additional).await?;
            body.extend(additional);
        }

        let body_str = String::from_utf8_lossy(&body[body_start..body_start + content_length]);
        let response: serde_json::Value = serde_json::from_str(&body_str).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Invalid JSON: {}", e))
        })?;

        Ok(response)
    }
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        // Ensure process is killed
        if self.child.is_some() {
            let _ = self.child.as_mut().map(|c| c.start_kill());
        }
    }
}

/// Check if a debug adapter is available in PATH
pub fn adapter_available(name: &str) -> bool {
    which::which(name).is_ok()
}

/// Get the expected path for common debug adapters
pub fn expected_adapter_path(name: &str) -> Option<PathBuf> {
    which::which(name).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_lookup() {
        // Check if common tools are available
        println!("cargo available: {}", adapter_available("cargo"));
        println!("python3 available: {}", adapter_available("python3"));
        println!("node available: {}", adapter_available("node"));
    }
}
