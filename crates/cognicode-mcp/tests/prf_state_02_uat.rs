//! PRF-STATE-02 UAT (binario real): namespace por
//! `canonical_root + analysis_config_digest`.
//!
//! 1. Dos workspaces distintos con nombres de símbolo IDÉNTICOS no se
//!    contaminan: digests y conjuntos de símbolos propios.
//! 2. El snapshot durable sobrevive al reinicio: digest y conteo estables.

mod common;
use common::McpSession;
use serde_json::Value;
use std::path::Path;

async fn build(ws: &Path) -> Value {
    let mut s = McpSession::spawn(ws).await.expect("spawn");
    let p = s
        .call_tool("build_graph", serde_json::json!({}))
        .await
        .expect("build_graph");
    s.shutdown().await;
    p
}

#[tokio::test]
async fn homonym_workspaces_do_not_contaminate_and_snapshot_survives_restart() {
    let base = std::env::temp_dir().join(format!("state02-uat-{}", std::process::id()));
    let ws1 = base.join("ws1");
    let ws2 = base.join("ws2");
    std::fs::create_dir_all(&ws1).unwrap();
    std::fs::create_dir_all(&ws2).unwrap();
    std::fs::write(ws1.join("lib.rs"), "fn alpha() -> i32 { 1 }\n").unwrap();
    std::fs::write(
        ws2.join("lib.rs"),
        "fn alpha() -> i32 { 2 }\nfn beta() {}\n",
    )
    .unwrap();

    let p1 = build(&ws1).await;
    let p2 = build(&ws2).await;

    let b1 = p1.get("basis").cloned().unwrap_or(Value::Null);
    let b2 = p2.get("basis").cloned().unwrap_or(Value::Null);
    let d1 = b1
        .get("source_manifest_digest")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let d2 = b2
        .get("source_manifest_digest")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(!d1.is_empty() && !d2.is_empty(), "deben existir digests");
    assert_ne!(
        d1, d2,
        "workspaces homónimos deben tener manifests distintos — PRF-STATE-02"
    );

    let s1 = p1
        .get("symbols_found")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let s2 = p2
        .get("symbols_found")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert_eq!(s1, 1, "ws1 solo ve su alpha");
    assert_eq!(s2, 2, "ws2 ve alpha+beta, no datos de ws1");

    // Snapshot durable: digest y conteo estables tras reinicio.
    let p1b = build(&ws1).await;
    let d1b = p1b
        .pointer("/basis/source_manifest_digest")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(d1, d1b, "digest estable tras reinicio");
    assert_eq!(
        s1,
        p1b.get("symbols_found")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    );

    let _ = std::fs::remove_dir_all(&base);
}
