//! Harness compartido para UATs de persistencia (binario real).
//! Duplicación eliminada tras §70-§73: un único spawn + call_tool.

#![allow(dead_code)]

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

pub fn binary_path() -> PathBuf {
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
