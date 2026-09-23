//! Harness compartido para UATs de persistencia (binario real).
//! Duplicación eliminada tras §70-§73: un único spawn + call_tool.

#![allow(dead_code)]

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

/// Resolve the absolute path to the `cognicode-mcp` binary under test.
///
/// Resolution order (first match wins):
///
/// 1. `CARGO_BIN_EXE_cognicode-mcp` — set by Cargo when an integration test in
///    the same crate as the binary is run; always correct, no filesystem
///    traversal required. Falls back gracefully when unset (e.g. when running
///    the binary tests from outside Cargo).
/// 2. `CARGO_TARGET_DIR/release/cognicode-mcp` — honors the user's cargo
///    target-dir override (e.g. `~/.cargo/config.toml` redirecting
///    `target-dir` away from the workspace `target/`).
/// 3. `<workspace_root>/target/release/cognicode-mcp` — historical fallback
///    (works when cargo and the workspace agree on `target/`).
///
/// Resolution is delegated to `resolve_binary_path` so the precedence list is
/// the single source of truth across every test that needs the binary.
pub fn binary_path() -> PathBuf {
    resolve_binary_path()
}

fn resolve_binary_path() -> PathBuf {
    if let Some(p) = option_env!("CARGO_BIN_EXE_cognicode-mcp") {
        return PathBuf::from(p);
    }

    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let candidate = PathBuf::from(target).join("release").join("cognicode-mcp");
        if candidate.exists() {
            return candidate;
        }
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/release/cognicode-mcp")
}

pub struct McpSession {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl McpSession {
    /// Spawn + initialize. `kill_on_drop` como red de seguridad.
    pub async fn spawn(ws: &Path) -> Result<Self, String> {
        let mut cmd = Command::new(binary_path());
        cmd.arg("--cwd").arg(ws);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut s = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        };
        s.send(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"uat","version":"0"}}})).await?;
        let mut line = String::new();
        s.stdout
            .read_line(&mut line)
            .await
            .map_err(|e| e.to_string())?;
        if line.is_empty() {
            return Err("sin respuesta a initialize".into());
        }
        s.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await?;
        s.next_id = 2;
        Ok(s)
    }

    async fn send(&mut self, v: &Value) -> Result<(), String> {
        self.stdin
            .write_all(format!("{}\n", v).as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        self.stdin.flush().await.map_err(|e| e.to_string())
    }

    /// tools/call + lectura de la respuesta con el id esperado.
    pub async fn call_tool(&mut self, name: &str, args: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).await?;
        let mut resp = String::new();
        loop {
            resp.clear();
            let n = self
                .stdout
                .read_line(&mut resp)
                .await
                .map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("server cerró stdout".into());
            }
            let m: Value = serde_json::from_str(resp.trim()).map_err(|e| e.to_string())?;
            if m.get("id").and_then(|v| v.as_u64()) == Some(id) {
                let text = m["result"]["content"][0]["text"]
                    .as_str()
                    .ok_or("no content")?;
                return serde_json::from_str(text).map_err(|e| e.to_string());
            }
        }
    }

    /// Cierre completo: stdin shutdown + kill + wait (no huérfanos).
    pub async fn shutdown(mut self) {
        let _ = self.stdin.shutdown().await;
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `binary_path` must return an absolute, well-formed path that ends with
    /// the binary's filename. The actual existence check is environment
    /// dependent (CARGO_BIN_EXE_* may or may not be set, the workspace target
    /// may or may not be built); this test only pins the path shape so a
    /// refactor that, say, dropped the file name is caught immediately.
    #[test]
    fn binary_path_resolves_to_cognicode_mcp_filename() {
        let path = binary_path();
        assert!(
            path.is_absolute() || path.starts_with("/"),
            "binary path must be absolute, got: {}",
            path.display()
        );
        assert_eq!(
            path.file_name().and_then(|s| s.to_str()),
            Some("cognicode-mcp"),
            "binary path must end with the binary name, got: {}",
            path.display()
        );
    }

    /// When `CARGO_BIN_EXE_cognicode-mcp` is set at compile time, the path
    /// returned must point at that env var. The env var is set by Cargo for
    /// integration tests in the same crate as the binary; without it the
    /// resolution falls through to the other branches.
    #[test]
    fn binary_path_prefers_cargo_bin_exe_env_var_when_set() {
        if let Some(expected) = option_env!("CARGO_BIN_EXE_cognicode-mcp") {
            let path = binary_path();
            assert_eq!(
                path.to_str(),
                Some(expected),
                "binary_path must return CARGO_BIN_EXE_cognicode-mcp when set at compile time"
            );
        }
        // If the env var is unset at compile time, the test is a no-op:
        // `binary_path` falls back to the other resolution branches, which
        // are environment dependent and not worth pinning here.
    }
}
