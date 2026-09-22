//! PRF-STATE-03 UAT (binario real, aislado): dos procesos MCP
//! concurrentes sobre el MISMO workspace no sobreescriben ni pierden
//! datos; stale identificado tras modificación de contenido.

mod common;
use common::McpSession;
use serde_json::{Value, json};
use std::path::Path;

async fn build(ws: &Path) -> Value {
    let mut s = McpSession::spawn(ws).await.expect("spawn");
    let p = s
        .call_tool("build_graph", json!({}))
        .await
        .expect("build_graph");
    s.shutdown().await;
    p
}

#[tokio::test]
async fn two_concurrent_processes_same_workspace_no_data_loss() {
    let ws = std::env::temp_dir().join(format!("state03-conc-{}", std::process::id()));
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("a.rs"), "fn alpha() {}\n").unwrap();

    // Lanzar DOS procesos MCP sobre el MISMO workspace en paralelo.
    let ws_a = ws.clone();
    let ws_b = ws.clone();
    let (a, b) = tokio::join!(build(&ws_a), build(&ws_b));

    // Ambos construyen sin pérdida: cada uno ve su(s) símbolo(s).
    assert_eq!(a.pointer("/status"), Some(&json!("complete")), "A: {a}");
    assert_eq!(b.pointer("/status"), Some(&json!("complete")), "B: {b}");
    assert_eq!(a.pointer("/symbols_found"), Some(&json!(1)), "A: {a}");
    assert_eq!(b.pointer("/symbols_found"), Some(&json!(1)), "B: {b}");

    // El snapshot final existe y es un snapshot válido (atómico: nunca
    // una mezcla a medias de dos escrituras).
    let snap = ws.join(".cognicode").join("graph.cache");
    assert!(
        snap.exists() && std::fs::read(&snap).map(|b| !b.is_empty()).unwrap_or(false),
        "snapshot final debe ser válido tras concurrencia"
    );
    let leftovers = std::fs::read_dir(ws.join(".cognicode"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("tmp"))
        .count();
    assert_eq!(
        leftovers, 0,
        "no deben quedar .tmp de escrituras interrumpidas"
    );

    // STALE identificado: modificación de contenido → el siguiente proceso
    // reconstruye (no sirve el snapshot viejo) y el resultado refleja el cambio.
    std::fs::write(ws.join("a.rs"), "fn alpha() {}\nfn gamma() {}\n").unwrap();
    let c = build(&ws).await;
    assert_eq!(
        c.pointer("/symbols_found"),
        Some(&json!(2)),
        "proceso C debe ver 2 símbolos tras modificación: {c}"
    );

    let _ = std::fs::remove_dir_all(&ws);
}
