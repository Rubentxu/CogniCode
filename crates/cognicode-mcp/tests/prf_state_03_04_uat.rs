//! PRF-STATE-03/04 UAT (real binary): durable snapshot semantics.
//!
//! 1. A workspace build persists `.cognicode/graph.cache`; a fresh
//!    server session over the same workspace loads it and reports
//!    `source: cache (durable snapshot)` — real persistence, not a
//!    deterministic rebuild.
//! 2. A corrupt (truncated) snapshot is never presented as valid:
//!    the next build treats it as absent and rebuilds successfully
//!    (`source: built`), leaving a fresh valid snapshot behind.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .parent()
        .expect("repo root")
        .join("target/release/cognicode-mcp")
}

/// Spawn a fresh server session, run build_graph, return the payload.
async fn build_and_read(workspace: &Path) -> Result<Value, String> {
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

    async fn send(stdin: &mut tokio::process::ChildStdin, v: &Value) -> Result<(), String> {
        stdin
            .write_all(format!("{}\n", v).as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        stdin.flush().await.map_err(|e| e.to_string())
    }

    send(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05", "capabilities": {},
                "clientInfo": { "name": "state04-uat", "version": "0" }
            }
        }),
    )
    .await?;
    let mut line = String::new();
    stdout.read_line(&mut line).await.map_err(|e| e.to_string())?;
    send(&mut stdin, &json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await?;
    send(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "build_graph", "arguments": {} }
        }),
    )
    .await?;
    let mut resp = String::new();
    let payload = loop {
        resp.clear();
        let n = stdout.read_line(&mut resp).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("server closed stdout".into());
        }
        let m: Value = serde_json::from_str(resp.trim()).map_err(|e| e.to_string())?;
        if m.get("id").and_then(|v| v.as_u64()) == Some(2) {
            let text = m["result"]["content"][0]["text"].as_str().ok_or("no content")?;
            break serde_json::from_str::<Value>(text).map_err(|e| e.to_string())?;
        }
    };
    let _ = child.kill().await;
    Ok(payload)
}

fn snapshot_path(ws: &Path) -> PathBuf {
    ws.join(".cognicode").join("graph.cache")
}

#[tokio::test]
async fn durable_snapshot_loads_on_restart_and_corruption_rebuilds() {
    let ws = std::env::temp_dir().join(format!("state04-uat-{}", std::process::id()));
    std::fs::create_dir_all(&ws).expect("mkdir ws");
    std::fs::write(ws.join("lib.rs"), "fn alpha() -> i32 { 1 }\n").expect("source");

    // 1. First build: creates the durable snapshot.
    let p1 = build_and_read(&ws).await.expect("first build");
    assert!(
        p1.get("message").and_then(|v| v.as_str()).unwrap_or("").contains("loaded from built"),
        "first build must build from source, got: {p1}"
    );
    assert!(
        snapshot_path(&ws).exists(),
        "build must persist .cognicode/graph.cache"
    );

    // 2. Fresh session: loads the durable snapshot (real persistence,
    //    not a deterministic rebuild).
    let p2 = build_and_read(&ws).await.expect("restart build");
    let msg2 = p2.get("message").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        msg2.contains("cache (durable snapshot)"),
        "restart must load the durable snapshot, got: {p2}"
    );

    // 3. Corrupt the snapshot: must never be presented as valid.
    let sp = snapshot_path(&ws);
    let orig = std::fs::read(&sp).expect("read snapshot");
    std::fs::write(&sp, &orig[..orig.len() / 2]).expect("truncate snapshot");
    let p3 = build_and_read(&ws).await.expect("corrupt-snapshot build");
    let msg3 = p3.get("message").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        msg3.contains("loaded from built"),
        "corrupt snapshot must be treated as absent and rebuilt, got: {p3}"
    );
    assert!(
        snapshot_path(&ws).exists(),
        "rebuild must leave a fresh valid snapshot"
    );

    let _ = std::fs::remove_dir_all(&ws);
}
