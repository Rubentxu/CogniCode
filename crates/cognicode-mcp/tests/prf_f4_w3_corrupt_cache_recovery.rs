//! PRF-F4-W3: corrupt cache recovery — the durable snapshot at
//! `<workspace>/.cognicode/graph.cache` must never be presented as
//! valid evidence. Any parse failure (truncated, random bytes, wrong
//! schema version, foreign file type) must be treated as absent and
//! the next build must rebuild and leave a valid snapshot.
//!
//! Comparison surface (narrowest observable shared between an MCP
//! session and its successor after corruption): the
//! `build_graph` tool's `message` field on a freshly spawned MCP
//! session. After corruption, the message must say the build was
//! done from source (not loaded from the durable snapshot).
//!
//! Non-vacuity guards: each test asserts (a) the first build
//! actually persists a snapshot, (b) the corrupt file is actually
//! non-empty and non-equal to the original, (c) the rebuilt
//! snapshot equals the original in size (so we know the rebuild
//! is faithful, not a stub).
//!
//! RED/GREEN invariant: each corruption scenario is verified to
//! trigger a rebuild (not a load-from-cache); if any scenario
//! ever silently loads a corrupt cache, this test fails.

mod common;

use common::McpSession;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Spawn MCP, run build_graph, shut down. Returns the parsed
/// JSON payload the tool returned.
async fn build(ws: &Path) -> Value {
    let mut s = McpSession::spawn(ws).await.expect("spawn");
    let p = s
        .call_tool("build_graph", serde_json::json!({}))
        .await
        .expect("build_graph");
    s.shutdown().await;
    p
}

/// Path to the durable snapshot file the build pipeline writes.
fn snapshot_path(ws: &Path) -> PathBuf {
    ws.join(".cognicode").join("graph.cache")
}

/// Make a small corpus so the first build actually produces a
/// non-trivial snapshot. Two functions with one call edge so the
/// graph has both symbols and a relationship.
fn write_corpus(ws: &Path) {
    let _ = std::fs::remove_dir_all(ws);
    std::fs::create_dir_all(ws).unwrap();
    std::fs::write(
        ws.join("lib.rs"),
        "pub fn unique_w3_alpha(x: u32) -> u32 { x + 1 }\n\
         pub fn unique_w3_beta(x: u32) -> u32 { unique_w3_alpha(x) + 1 }\n",
    )
    .unwrap();
}

/// Assert that `message` in the build result says the build was
/// done from source, not loaded from a durable snapshot. This is
/// the F4 invariant: a corrupt snapshot must never be presented
/// as valid evidence.
fn assert_built_from_source(payload: &Value, ctx: &str) {
    let msg = payload
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("{ctx}: no message in {payload}"));
    assert!(
        msg.contains("loaded from built") || msg.contains("rebuilt"),
        "{ctx}: expected 'built from source' / 'rebuilt', got: {msg}"
    );
}

/// Assert that `message` says the build loaded a durable snapshot.
/// This is the "good path" — only seen on a clean restart with an
/// intact cache.
fn assert_loaded_from_snapshot(payload: &Value, ctx: &str) {
    let msg = payload
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("{ctx}: no message in {payload}"));
    assert!(
        msg.contains("durable snapshot"),
        "{ctx}: expected 'durable snapshot' load, got: {msg}"
    );
}

/// F4.W3 / test 1: corruption = binary random bytes. The next
/// build must rebuild (not load corrupt cache) and the rebuilt
/// snapshot must be the same size as the original (faithful
/// rebuild, not a stub).
#[tokio::test]
async fn prf_f4_w3_corrupt_random_bytes_triggers_rebuild() {
    let ws = std::env::temp_dir().join(format!("prf-f4-w3-random-{}", std::process::id()));
    write_corpus(&ws);

    let _p1 = build(&ws).await;
    assert!(snapshot_path(&ws).exists(), "first build must persist");
    let orig = std::fs::read(snapshot_path(&ws)).expect("read original");
    assert!(
        orig.len() > 50,
        "non-vacuity: snapshot too small to corrupt meaningfully: {} bytes",
        orig.len()
    );

    // Corrupt: replace with deterministic-but-arbitrary bytes
    // (NOT random — deterministic so the test is reproducible).
    let mut corrupted = Vec::with_capacity(orig.len());
    for i in 0..orig.len() {
        corrupted.push(((i as u32).wrapping_mul(2654435761) >> 24) as u8);
    }
    assert_ne!(
        orig, corrupted,
        "non-vacuity: corrupted bytes must differ from original"
    );
    std::fs::write(snapshot_path(&ws), &corrupted).unwrap();

    let p2 = build(&ws).await;
    assert_built_from_source(&p2, "corrupt random bytes");

    // Non-vacuity: rebuild must restore the snapshot to its
    // original size (faithful rebuild, not a stub).
    let rebuilt = std::fs::read(snapshot_path(&ws)).expect("read rebuilt");
    assert_eq!(
        rebuilt.len(),
        orig.len(),
        "non-vacuity: rebuilt snapshot must match original size"
    );

    let _ = std::fs::remove_dir_all(&ws);
}

/// F4.W3 / test 2: corruption = completely empty file. The next
/// build must rebuild and persist a valid snapshot.
#[tokio::test]
async fn prf_f4_w3_corrupt_empty_file_triggers_rebuild() {
    let ws = std::env::temp_dir().join(format!("prf-f4-w3-empty-{}", std::process::id()));
    write_corpus(&ws);

    let _p1 = build(&ws).await;
    assert!(snapshot_path(&ws).exists(), "first build must persist");
    let orig_len = std::fs::metadata(snapshot_path(&ws)).unwrap().len();
    assert!(
        orig_len > 0,
        "non-vacuity: original snapshot must be non-empty"
    );

    std::fs::write(snapshot_path(&ws), b"").unwrap();

    let p2 = build(&ws).await;
    assert_built_from_source(&p2, "empty file");

    let rebuilt_len = std::fs::metadata(snapshot_path(&ws)).unwrap().len();
    assert_eq!(
        rebuilt_len, orig_len,
        "non-vacuity: rebuilt snapshot must match original size"
    );

    let _ = std::fs::remove_dir_all(&ws);
}

/// F4.W3 / test 3: corruption = wrong schema version. The snapshot
/// envelope starts with a bincode-encoded `(String, ...)` tuple;
/// the loader rejects anything whose first field isn't exactly
/// `"cognicode.graph.cache/v1"`. We swap the prefix bytes to a
/// known-bad version string and confirm rebuild.
#[tokio::test]
async fn prf_f4_w3_corrupt_wrong_schema_version_triggers_rebuild() {
    let ws = std::env::temp_dir().join(format!("prf-f4-w3-version-{}", std::process::id()));
    write_corpus(&ws);

    let _p1 = build(&ws).await;
    assert!(snapshot_path(&ws).exists(), "first build must persist");
    let orig = std::fs::read(snapshot_path(&ws)).expect("read original");

    // Replace the first 32 bytes with the bincode encoding of a
    // known-bad version string `"cognicode.graph.cache/v999\n"`.
    // bincode default encoding for a String: 8-byte little-endian
    // length prefix, then UTF-8 bytes.
    let bad_version = b"cognicode.graph.cache/v999";
    let mut corrupted = Vec::with_capacity(orig.len());
    let len_bytes = (bad_version.len() as u64).to_le_bytes();
    corrupted.extend_from_slice(&len_bytes);
    corrupted.extend_from_slice(bad_version);
    // Pad with a deterministic filler so the file is at least as
    // long as the original (bincode parser doesn't care about
    // trailing bytes, but the loader reads the whole file).
    while corrupted.len() < orig.len() {
        corrupted.push(0xFF);
    }
    assert_ne!(orig, corrupted);
    std::fs::write(snapshot_path(&ws), &corrupted).unwrap();

    let p2 = build(&ws).await;
    assert_built_from_source(&p2, "wrong schema version");

    let rebuilt = std::fs::read(snapshot_path(&ws)).expect("read rebuilt");
    assert_eq!(
        rebuilt.len(),
        orig.len(),
        "non-vacuity: rebuilt snapshot must match original size"
    );

    let _ = std::fs::remove_dir_all(&ws);
}

/// F4.W3 / test 4: positive control — on a clean restart with an
/// intact cache, the loader MUST load from the snapshot (not
/// rebuild). This is the "good path" that the corruption tests
/// pivot around; without it, the corruption tests could pass
/// trivially if the loader always rebuilt.
#[tokio::test]
async fn prf_f4_w3_intact_cache_loads_from_snapshot() {
    let ws = std::env::temp_dir().join(format!("prf-f4-w3-intact-{}", std::process::id()));
    write_corpus(&ws);

    let _p1 = build(&ws).await;
    assert!(snapshot_path(&ws).exists(), "first build must persist");

    // Clean restart: cache is intact.
    let p2 = build(&ws).await;
    assert_loaded_from_snapshot(&p2, "intact cache");

    let _ = std::fs::remove_dir_all(&ws);
}
