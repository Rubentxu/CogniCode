//! PRF-STATE-03/04 — Reproducción mínima y aislada (protocolo del operador):
//! 1. Proceso 1: build sobre workspace temporal → snapshot en disco.
//! 2. Verificar hash/identidad del snapshot.
//! 3. Proceso cerrado; Proceso 2: build sobre el MISMO workspace sin cambios
//!    → debe recuperar snapshot (identidad y contenido correctos).
//! 4. Modificar contenido → consulta siguiente debe reconstruir (invalidación).

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/release/cognicode-mcp")
}

/// Un proceso MCP aislado: spawn, initialize, build_graph, leer payload, kill.
async fn one_session(ws: &Path) -> Result<Value, String> {
    let mut cmd = Command::new(binary_path());
    cmd.arg("--cwd").arg(ws);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut stdout = BufReader::new(stdout);

    async fn send(stdin: &mut tokio::process::ChildStdin, v: &Value) -> Result<(), String> {
        stdin
            .write_all(format!("{}\n", v).as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        stdin.flush().await.map_err(|e| e.to_string())
    }

    send(&mut stdin, &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"iso","version":"0"}}})).await?;
    let mut line = String::new();
    stdout.read_line(&mut line).await.map_err(|e| e.to_string())?;
    send(&mut stdin, &json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await?;
    send(&mut stdin, &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"build_graph","arguments":{}}})).await?;

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
            break serde_json::from_str(text).map_err(|e| e.to_string())?;
        }
    };
    // Cierre COMPLETO del proceso (stdin close + kill + wait).
    let _ = stdin.shutdown().await;
    let _ = child.kill().await;
    let _ = child.wait().await;
    Ok(payload)
}

/// Vista semántica sin campos variantes (message/elapsed).
fn view(p: &Value) -> Value {
    json!({
        "status": p.pointer("/status"),
        "symbols_found": p.pointer("/symbols_found"),
        "relationships_found": p.pointer("/relationships_found"),
        "source_manifest_digest": p.pointer("/basis/source_manifest_digest"),
        "skipped_files": p.pointer("/skipped_files"),
    })
}

#[tokio::test]
async fn isolated_two_process_snapshot_lifecycle() {
    let ws = std::env::temp_dir().join(format!("state04-iso-{}", std::process::id()));
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("lib.rs"), "fn alpha() -> i32 { 1 }\n").unwrap();
    let snap = ws.join(".cognicode").join("graph.cache");

    // (1) Proceso 1: build desde fuente.
    let p1 = one_session(&ws).await.expect("p1");
    assert_eq!(p1.pointer("/status"), Some(&json!("complete")), "p1: {p1}");
    assert!(snap.exists(), "p1 debe persistir snapshot");
    let snap_bytes_1 = std::fs::read(&snap).unwrap();

    // (3) Proceso 2 (proceso 1 ya cerrado dentro de one_session): mismo workspace sin cambios.
    let p2 = one_session(&ws).await.expect("p2");
    let v2 = view(&p2);
    let v1 = view(&p1);
    assert!(
        p2.pointer("/message").and_then(|m| m.as_str()).unwrap().contains("durable snapshot"),
        "p2 debe comunicar snapshot durable: {p2}"
    );
    // Identidad y contenido: idénticos entre build y recuperación.
    assert_eq!(v1, v2, "contenido recuperado debe ser idéntico al construido");

    // (4) Modificación de contenido → invalidación → reconstrucción.
    std::fs::write(ws.join("lib.rs"), "fn alpha() -> i32 { 2 }\nfn beta() {}\n").unwrap();
    let p3 = one_session(&ws).await.expect("p3");
    assert_eq!(p3.pointer("/status"), Some(&json!("complete")), "p3: {p3}");
    assert!(
        p3.pointer("/message").and_then(|m| m.as_str()).unwrap().contains("built"),
        "cambio de contenido debe forzar reconstrucción: {p3}"
    );
    assert_eq!(
        p3.pointer("/symbols_found"), Some(&json!(2)),
        "grafo reconstruido debe reflejar el nuevo contenido"
    );
    // El snapshot quedó reemplazado (no es el hash original).
    let snap_bytes_3 = std::fs::read(&snap).unwrap();
    assert_ne!(
        snap_bytes_1, snap_bytes_3,
        "snapshot debe haberse reemplazado tras el cambio de contenido"
    );

    let _ = std::fs::remove_dir_all(&ws);
}
