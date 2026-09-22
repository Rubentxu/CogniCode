//! PRF-STATE-02 UAT (real binary): namespace by
//! `canonical_root + analysis_config_digest`.
//!
//! 1. Two distinct workspaces with IDENTICAL symbol names must not
//!    contaminate each other: each build reports its own basis digests
//!    and its own symbol set.
//! 2. A durable snapshot must survive a server restart: a second
//!    session over the same workspace loads the cached graph and keeps
//!    the same basis digests.

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

async fn build_and_read_basis(workspace: &Path) -> Result<Value, String> {
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
                "clientInfo": { "name": "state02-uat", "version": "0" }
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
        let n = stdout
            .read_line(&mut resp)
            .await
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("server closed stdout".into());
        }
        let m: Value = serde_json::from_str(resp.trim()).map_err(|e| e.to_string())?;
        if m.get("id").and_then(|v| v.as_u64()) == Some(2) {
            let text = m["result"]["content"][0]["text"]
                .as_str()
                .ok_or("no content")?;
            break serde_json::from_str::<Value>(text).map_err(|e| e.to_string())?;
        }
    };
    let _ = child.kill().await;
    Ok(payload)
}

#[tokio::test]
async fn homonym_workspaces_do_not_contaminate_and_snapshot_survives_restart() {
    let base = std::env::temp_dir().join(format!("state02-uat-{}", std::process::id()));
    let ws1 = base.join("ws1");
    let ws2 = base.join("ws2");
    std::fs::create_dir_all(&ws1).expect("mkdir ws1");
    std::fs::create_dir_all(&ws2).expect("mkdir ws2");
    // Identical symbol NAME, different bodies → different manifests.
    std::fs::write(ws1.join("lib.rs"), "fn alpha() -> i32 { 1 }\n").expect("ws1");
    std::fs::write(ws2.join("lib.rs"), "fn alpha() -> i32 { 2 }\nfn beta() {}\n").expect("ws2");

    let p1 = build_and_read_basis(&ws1).await.expect("ws1 build");
    let p2 = build_and_read_basis(&ws2).await.expect("ws2 build");

    // 1. Namespace separation: distinct canonical roots → distinct digests.
    let b1 = p1.get("basis").cloned().unwrap_or(Value::Null);
    let b2 = p2.get("basis").cloned().unwrap_or(Value::Null);
    let d1 = b1.get("source_manifest_digest").and_then(|v| v.as_str()).unwrap_or("");
    let d2 = b2.get("source_manifest_digest").and_then(|v| v.as_str()).unwrap_or("");
    assert!(!d1.is_empty() && !d2.is_empty(), "basis digests must exist");
    assert_ne!(
        d1, d2,
        "homonym workspaces must have distinct source manifests — PRF-STATE-02 RED"
    );

    // 2. No contamination: ws2 has 2 symbols, ws1 has 1.
    let s1 = p1.get("symbols_found").and_then(|v| v.as_u64()).unwrap_or(0);
    let s2 = p2.get("symbols_found").and_then(|v| v.as_u64()).unwrap_or(0);
    assert_eq!(s1, 1, "ws1 must see only its own alpha");
    assert_eq!(s2, 2, "ws2 must see alpha+beta, not ws1's data");

    // 3. Durable snapshot: a fresh session over ws1 loads the cache and
    //    keeps the same basis digests.
    let p1b = build_and_read_basis(&ws1).await.expect("ws1 rebuild");
    let b1b = p1b.get("basis").cloned().unwrap_or(Value::Null);
    let d1b = b1b.get("source_manifest_digest").and_then(|v| v.as_str()).unwrap_or("");
    assert_eq!(d1, d1b, "snapshot digests must be stable across restart");
    let s1b = p1b.get("symbols_found").and_then(|v| v.as_u64()).unwrap_or(0);
    assert_eq!(s1, s1b, "symbol counts must be stable across restart");

    let _ = std::fs::remove_dir_all(&base);
}
