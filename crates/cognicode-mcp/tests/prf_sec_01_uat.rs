//! PRF-SEC-01 UAT (real binary): build_graph directory is validated
//! against the workspace allowlist — traversal, absolute escapes and
//! symlinked entry points are rejected with honest errors; the
//! workspace root itself is allowed.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .parent()
        .expect("repo root")
        .join("target/release/cognicode-mcp")
}

async fn probe(workspace: &Path, directory: &str) -> Result<String, String> {
    let mut cmd = Command::new(binary_path());
    cmd.arg("--cwd").arg(workspace);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| format!("spawn: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let mut stdout = BufReader::new(stdout);

    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "sec01-uat", "version": "0" }
        }
    });
    send(&mut stdin, &init).await?;
    let mut line = String::new();
    stdout
        .read_line(&mut line)
        .await
        .map_err(|e| e.to_string())?;
    let inited = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
    send(&mut stdin, &inited).await?;

    let call = json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "build_graph", "arguments": { "directory": directory } }
    });
    send(&mut stdin, &call).await?;

    let mut resp = String::new();
    loop {
        resp.clear();
        let n = stdout
            .read_line(&mut resp)
            .await
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("server closed stdout before response".into());
        }
        let m: Value = serde_json::from_str(resp.trim()).map_err(|e| format!("bad json: {e}"))?;
        if m.get("id").and_then(|v| v.as_u64()) == Some(2) {
            let text = m["result"]["content"][0]["text"]
                .as_str()
                .ok_or("no content")?
                .to_string();
            let _ = child.kill().await;
            return Ok(text);
        }
    }
}

async fn send(stdin: &mut tokio::process::ChildStdin, v: &Value) -> Result<(), String> {
    stdin
        .write_all(format!("{}\n", v).as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    stdin.flush().await.map_err(|e| e.to_string())
}

#[tokio::test]
async fn build_graph_rejects_workspace_escapes() {
    let base = std::env::temp_dir().join(format!("sec01-uat-{}", std::process::id()));
    let ws = base.join("ws");
    let outside = base.join("outside");
    std::fs::create_dir_all(ws.join("src")).expect("mkdir ws");
    std::fs::create_dir_all(&outside).expect("mkdir outside");
    std::fs::write(ws.join("src/lib.rs"), "fn alpha() {}\n").expect("write lib");
    std::fs::write(outside.join("evil.rs"), "fn evil() {}\n").expect("write evil");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, ws.join("link-out")).expect("symlink");

    let traversal = probe(&ws, "../outside").await.expect("traversal probe");
    assert!(
        traversal.contains("rejected") || traversal.contains("error"),
        "traversal must be rejected: {traversal}"
    );

    let abs = probe(&ws, outside.to_str().unwrap())
        .await
        .expect("absolute probe");
    assert!(
        abs.contains("rejected") || abs.contains("outside workspace"),
        "absolute escape must be rejected: {abs}"
    );

    let link = probe(&ws, "link-out").await.expect("symlink probe");
    assert!(
        link.contains("rejected") || link.contains("Symlink"),
        "symlink entry must be rejected: {link}"
    );

    let self_ok = probe(&ws, ".").await.expect("self probe");
    assert!(
        !self_ok.contains("rejected"),
        "workspace root must be allowed: {self_ok}"
    );

    let _ = std::fs::remove_dir_all(&base);
}
