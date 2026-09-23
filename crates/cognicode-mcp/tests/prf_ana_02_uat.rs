//! PRF-ANA-02 UAT (real binary, stdio JSON-RPC):
//! `build_graph` must NOT silently drop files it cannot read or parse.
//! A corpus with one permission-000 file must yield a Partial status with
//! that file surfaced in `skipped` (never a silent `complete` with the
//! file invisibly missing).

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

mod common;
use common::binary_path;

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .parent()
        .expect("repo root")
        .join("docs/prf/fixtures/ana02_corpus")
}

struct McpChild {
    // `child` is consumed during `spawn()` via `child.stdin.take()` /
    // `child.stdout.take()`.  After construction the field is never
    // read directly — clippy `-D warnings` flags this as `dead_code`
    // even though it is load-bearing for the spawn flow.  The allow
    // below scopes the suppression to the field only; if the struct
    // grows a method that uses `self.child` it must be removed.
    #[allow(dead_code)]
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl McpChild {
    async fn spawn(workspace: &Path) -> Result<Self, String> {
        let mut cmd = Command::new(binary_path());
        cmd.arg("--cwd").arg(workspace);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let stdout = BufReader::new(stdout);
        let mut me = Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        me.initialize().await?;
        Ok(me)
    }

    async fn initialize(&mut self) -> Result<(), String> {
        self.send_raw(&json!({
            "jsonrpc": "2.0",
            "id": self.next_id,
            "method": "initialize",
            "params": {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "prf-ana02-uat", "version": "0" }
            }
        }))
        .await?;
        let _ = self.read_response(self.next_id).await?;
        self.next_id += 1;
        self.send_raw(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))
        .await?;
        Ok(())
    }

    async fn send_raw(&mut self, req: &Value) -> Result<(), String> {
        let mut line = req.to_string();
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("write: {e}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| format!("flush: {e}"))?;
        Ok(())
    }

    async fn read_response(&mut self, id: u64) -> Result<Value, String> {
        let mut line = String::new();
        loop {
            line.clear();
            let n = self
                .stdout
                .read_line(&mut line)
                .await
                .map_err(|e| format!("read_line: {e}"))?;
            if n == 0 {
                return Err("server closed stdout before sending response".into());
            }
            let msg: Value =
                serde_json::from_str(line.trim()).map_err(|e| format!("parse: {e}: {line}"))?;
            if msg.get("id").and_then(|v| v.as_u64()) == Some(id) {
                return Ok(msg);
            }
        }
    }

    async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        });
        self.send_raw(&req).await?;
        self.read_response(id).await
    }
}

/// One file in the corpus is chmod-000. The graph build must report it
/// as skipped: status != complete, or a non-empty skipped list naming
/// the file. A silent `complete` is a RED failure of PRF-ANA-02.
#[tokio::test]
async fn unreadable_file_is_reported_not_silently_dropped() {
    let ws = workspace();
    let locked = ws.join("locked/secret.rs");
    assert!(
        locked.exists(),
        "corpus fixture missing: {}",
        locked.display()
    );

    // Deny read access for the duration of the build.
    let mut perms = std::fs::metadata(&locked).expect("metadata").permissions();
    use std::os::unix::fs::PermissionsExt;
    let original = perms.mode();
    perms.set_mode(0o000);
    std::fs::set_permissions(&locked, perms).expect("chmod 000");

    let result = McpChild::spawn(&ws).await;

    let outcome = async {
        let mut mcp = result?;
        let resp = mcp
            .call_tool("build_graph", json!({ "directory": ws.to_string_lossy() }))
            .await?;
        let text = resp["result"]["content"][0]["text"]
            .as_str()
            .ok_or("no content text")?;
        let payload: Value = serde_json::from_str(text).map_err(|e| format!("payload: {e}"))?;
        Ok::<Value, String>(payload)
    }
    .await;

    // Restore permissions before asserting so failures don't poison reruns.
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(original))
        .expect("restore perms");

    let payload = outcome.expect("build_graph call failed");
    let status = payload.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let skipped = payload.get("skipped_files").cloned().unwrap_or(Value::Null);
    let skipped_str = serde_json::to_string(&skipped).unwrap_or_default();

    let reported = status != "complete" || skipped_str.contains("secret.rs");
    assert!(
        reported,
        "PRF-ANA-02 RED: unreadable file silently dropped — status={status}, skipped={skipped_str}"
    );
    if status != "complete" {
        assert!(
            skipped_str.contains("secret.rs") || skipped_str.contains("locked"),
            "Partial status must name the skipped file: {skipped_str}"
        );
    }
}
