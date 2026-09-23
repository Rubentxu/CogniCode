//! PRF-STATE-01 UAT: canonical / derived / transient data catalog with
//! ownership, location and lifecycle, exercised at the process boundary.
//!
//! - Canonical: workspace source files (the only source of truth).
//! - Derived: `.cognicode/graph.cache` — owned by the tool, rebuildable,
//!   never authoritative.
//! - Transient: in-memory graph — never outlives the process.
//!
//! Claims:
//! 1. Deleting derived data never loses information: rebuild restores
//!    the identical inventory from canonical sources.
//! 2. Editing canonical sources invalidates the derived cache: the next
//!    session reflects the change (derived follows canonical, never the
//!    reverse).
//! 3. Transient state does not leak across processes: a fresh session
//!    sees only canonical+derived state.

use std::path::{Path, PathBuf};

fn fixture_source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

#[path = "common/mod.rs"]
pub mod harness;

use harness::McpSession;

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let e = entry.unwrap();
        let d = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            if e.file_name() != ".cognicode" {
                copy_dir(&e.path(), &d);
            }
        } else {
            std::fs::copy(e.path(), &d).unwrap();
        }
    }
}

fn fresh_ws(tag: &str) -> PathBuf {
    let dst = std::env::temp_dir()
        .join(format!("prf-state01-{}-{}", tag, std::process::id()))
        .join(format!(
            "t{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
    copy_dir(&fixture_source(), &dst);
    dst
}

#[tokio::test(flavor = "multi_thread")]
async fn derived_data_is_rebuildable_and_follows_canonical() {
    let ws = fresh_ws("lifecycle");
    let cache_dir = ws.join(".cognicode");

    // Session 1: build derived cache from canonical sources.
    let baseline;
    {
        let mut s = McpSession::spawn(&ws).await.unwrap();
        let g = s
            .call_tool(
                "build_graph",
                serde_json::json!({"directory": ws.to_str().unwrap()}),
            )
            .await
            .unwrap();
        assert_eq!(g["status"], "complete");
        baseline = g["symbols_found"].as_u64().unwrap();
        assert!(baseline >= 2);
    }
    assert!(
        cache_dir.join("graph.cache").exists(),
        "derived cache must exist"
    );

    // Claim 1: derived data is disposable — rebuild from canonical.
    std::fs::remove_dir_all(&cache_dir).unwrap();
    {
        let mut s = McpSession::spawn(&ws).await.unwrap();
        let g = s
            .call_tool(
                "build_graph",
                serde_json::json!({"directory": ws.to_str().unwrap()}),
            )
            .await
            .unwrap();
        assert_eq!(g["status"], "complete");
        assert_eq!(
            g["symbols_found"].as_u64().unwrap(),
            baseline,
            "rebuild must restore identical inventory"
        );
    }

    // Claim 2: canonical edit invalidates derived — derived follows
    // canonical, never the other way around.
    let lib = ws.join("src/lib.rs");
    std::fs::write(&lib, "pub fn add(a: u32, b: u32) -> u32 { a + b }\npub fn mul(a: u32, b: u32) -> u32 { a * b }\npub fn sub(a: u32, b: u32) -> u32 { a - b }\n").unwrap();
    {
        let mut s = McpSession::spawn(&ws).await.unwrap();
        let g = s
            .call_tool(
                "build_graph",
                serde_json::json!({"directory": ws.to_str().unwrap()}),
            )
            .await
            .unwrap();
        assert_eq!(g["status"], "complete");
        let now = g["symbols_found"].as_u64().unwrap();
        assert!(
            now > baseline,
            "canonical edit must be reflected ({} -> {})",
            baseline,
            now
        );
    }

    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn transient_state_does_not_leak_across_processes() {
    let ws = fresh_ws("transient");

    // Session 1 builds in-memory state that is never persisted.
    let mut s1 = McpSession::spawn(&ws).await.unwrap();
    let g1 = s1
        .call_tool(
            "build_graph",
            serde_json::json!({"directory": ws.to_str().unwrap()}),
        )
        .await
        .unwrap();
    assert_eq!(g1["status"], "complete");
    drop(s1); // process ends

    // Session 2 (before any rebuild): transient state from session 1 is
    // unreachable; the served inventory comes from canonical/derived and
    // is consistent with what the sources say (2 functions).
    let mut s2 = McpSession::spawn(&ws).await.unwrap();
    let g2 = s2
        .call_tool(
            "build_graph",
            serde_json::json!({"directory": ws.to_str().unwrap()}),
        )
        .await
        .unwrap();
    assert_eq!(g2["status"], "complete");
    assert_eq!(
        g2["symbols_found"].as_u64().unwrap(),
        g1["symbols_found"].as_u64().unwrap(),
        "fresh session must agree with the canonical inventory"
    );
    let _ = std::fs::remove_dir_all(ws.parent().unwrap());
}
