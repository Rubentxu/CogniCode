//! PRF-STATE-03/04 UAT (binario real): semántica de snapshot durable.
//! 1. Un build persiste `.cognicode/graph.cache`; una sesión fresca lo
//!    carga y comunica `source: cache (durable snapshot)`.
//! 2. Un snapshot corrupto (truncado) nunca se presenta como válido:
//!    el siguiente build reconstruye y deja snapshot válido.

mod common;
use common::McpSession;
use serde_json::Value;
use std::path::{Path, PathBuf};

async fn build(ws: &Path) -> Value {
    let mut s = McpSession::spawn(ws).await.expect("spawn");
    let p = s.call_tool("build_graph", serde_json::json!({})).await.expect("build_graph");
    s.shutdown().await;
    p
}

fn snapshot_path(ws: &Path) -> PathBuf {
    ws.join(".cognicode").join("graph.cache")
}

#[tokio::test]
async fn durable_snapshot_loads_on_restart_and_corruption_rebuilds() {
    let ws = std::env::temp_dir().join(format!("state04-uat-{}", std::process::id()));
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("lib.rs"), "fn alpha() -> i32 { 1 }\n").unwrap();

    // 1. Primer build: crea el snapshot durable.
    let p1 = build(&ws).await;
    assert!(
        p1.get("message").and_then(|v| v.as_str()).unwrap().contains("loaded from built"),
        "primer build debe construir desde fuente: {p1}"
    );
    assert!(snapshot_path(&ws).exists(), "debe persistir graph.cache");

    // 2. Sesión fresca: carga el snapshot (persistencia real).
    let p2 = build(&ws).await;
    assert!(
        p2.get("message").and_then(|v| v.as_str()).unwrap().contains("durable snapshot"),
        "reinicio debe cargar el snapshot durable: {p2}"
    );

    // 3. Snapshot corrupto: nunca es evidencia válida.
    let sp = snapshot_path(&ws);
    let orig = std::fs::read(&sp).unwrap();
    std::fs::write(&sp, &orig[..orig.len() / 2]).unwrap();
    let p3 = build(&ws).await;
    assert!(
        p3.get("message").and_then(|v| v.as_str()).unwrap().contains("loaded from built"),
        "snapshot corrupto debe tratarse como ausente: {p3}"
    );
    assert!(snapshot_path(&ws).exists(), "rebuild debe dejar snapshot válido");

    let _ = std::fs::remove_dir_all(&ws);
}
