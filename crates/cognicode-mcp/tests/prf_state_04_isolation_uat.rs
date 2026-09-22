//! PRF-STATE-03/04 — Reproducción mínima aislada: dos procesos MCP
//! secuenciales sobre el mismo workspace. Causa raíz §72 demostrada
//! aquí: el snapshot recuperado debe informar complete/idéntico al build.

mod common;
use common::McpSession;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Un proceso MCP completo: spawn + build_graph + cierre total.
async fn one_session(ws: &Path) -> Value {
    let mut s = McpSession::spawn(ws).await.expect("spawn");
    let p = s.call_tool("build_graph", json!({})).await.expect("build_graph");
    s.shutdown().await;
    p
}

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
    let p1 = one_session(&ws).await;
    assert_eq!(p1.pointer("/status"), Some(&json!("complete")), "p1: {p1}");
    assert!(snap.exists(), "p1 debe persistir snapshot");
    let snap_bytes_1 = std::fs::read(&snap).unwrap();

    // (3) Proceso 2: mismo workspace sin cambios → recupera snapshot.
    let p2 = one_session(&ws).await;
    assert!(
        p2.pointer("/message").and_then(|m| m.as_str()).unwrap().contains("durable snapshot"),
        "p2 debe comunicar snapshot durable: {p2}"
    );
    assert_eq!(view(&p1), view(&p2), "contenido recuperado idéntico al construido");

    // (4) Cambio de contenido → invalidación → reconstrucción.
    std::fs::write(ws.join("lib.rs"), "fn alpha() -> i32 { 2 }\nfn beta() {}\n").unwrap();
    let p3 = one_session(&ws).await;
    assert_eq!(p3.pointer("/status"), Some(&json!("complete")), "p3: {p3}");
    assert!(
        p3.pointer("/message").and_then(|m| m.as_str()).unwrap().contains("built"),
        "cambio de contenido fuerza reconstrucción: {p3}"
    );
    assert_eq!(p3.pointer("/symbols_found"), Some(&json!(2)));
    let snap_bytes_3 = std::fs::read(&snap).unwrap();
    assert_ne!(snap_bytes_1, snap_bytes_3, "snapshot reemplazado");

    let _ = std::fs::remove_dir_all(&ws);
}
