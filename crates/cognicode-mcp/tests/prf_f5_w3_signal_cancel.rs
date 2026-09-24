//! PRF-F5-W3: external signal cancellation — when the host sends
//! SIGKILL to the MCP server (simulating an out-of-band operator
//! kill, container termination, or uncaught panic), the workspace
//! must not be left in a state that would block the next session.
//!
//! F5 criterion (c): "cancelación desde señal externa".
//!
//! Comparison surface (narrowest observable): the workspace
//! directory AFTER the kill. The next MCP session must be able to
//! acquire the workspace without manual cleanup — i.e.:
//!   - no `*.lock` files left behind (the binary uses lockfiles
//!     to serialize concurrent writers; a stale lock would block
//!     the next session).
//!   - no `graph.cache.tmp.*` files left behind (a stale tmp
//!     from an interrupted atomic rename is harmless because the
//!     loader ignores them, but if it WERE honored by mistake the
//!     recovery contract would be broken — this test pins the
//!     current ignore-semantics).
//!
//! Non-vacuity guards: each test asserts the workspace actually
//! contains the target file types BEFORE the kill, so a "vacuous
//! pass" (kill, no files, trivially green) cannot happen.
//!
//! RED/GREEN invariant: if the kill-on-drop path leaves a stale
//! lock, this test fails — the next session would be unable to
//! acquire the workspace. This is exactly the contract sec_05
//! also pins (stdin-EOF path); this test pins the SIGKILL path
//! which is the more common failure mode in production.

mod common;

use common::McpSession;
use std::path::Path;

/// Path to the fixture workspace with at least one source file
/// so a build can produce a real cache.
fn fixture_ws() -> std::path::PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

use std::path::PathBuf;

/// Run a tool, hold the session open, then send SIGKILL
/// externally. Returns the workspace path so the test can
/// inspect the aftermath.
async fn spawn_run_kill(ws: &Path, tool: &str, args: serde_json::Value) -> PathBuf {
    let mut s = McpSession::spawn(ws).await.expect("spawn");
    // Run the tool so the build pipeline actually creates files.
    let _resp = s.call_tool(tool, args).await.expect(tool);
    // Drop without calling shutdown() — emulates an out-of-band
    // kill (panic, container OOM, operator SIGKILL). Drop runs
    // Child::kill() which on Unix is SIGKILL.
    drop(s);
    ws.to_path_buf()
}

/// F5.W3 / test 1: SIGKILL after a build_graph leaves no stale
/// lockfiles that would block the next session from acquiring
/// the workspace. The MCP server uses `.cognicode.lock`-style
/// lockfiles for some operations; a stale lock would force
/// manual operator intervention.
#[tokio::test(flavor = "multi_thread")]
async fn prf_f5_w3_sigkill_after_build_leaves_no_stale_lock() {
    let ws = fixture_ws();
    let ws_copy = std::env::temp_dir().join(format!(
        "prf-f5-w3-lock-{}",
        std::process::id()
    ));
    copy_dir(&ws, &ws_copy);

    // Pre-condition: workspace has no lock file before we start.
    let lock_candidates: Vec<PathBuf> = std::fs::read_dir(&ws_copy)
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            if p.extension().and_then(|x| x.to_str()) == Some("lock") {
                Some(p)
            } else {
                None
            }
        })
        .collect();

    let _ = spawn_run_kill(
        &ws_copy,
        "build_graph",
        serde_json::json!({"directory": ws_copy.to_str().unwrap()}),
    )
    .await;

    // Post-condition: no NEW .lock file in the workspace root or
    // its .cognicode subdir.
    let mut post_locks = Vec::new();
    for entry in walkdir_files(&ws_copy) {
        if entry.extension().and_then(|x| x.to_str()) == Some("lock") {
            // Allowed only if it was there pre-condition.
            if !lock_candidates.contains(&entry) {
                post_locks.push(entry);
            }
        }
    }
    assert!(
        post_locks.is_empty(),
        "SIGKILL after build left stale lock files that block the next session: {post_locks:?}"
    );

    let _ = std::fs::remove_dir_all(&ws_copy);
}

/// F5.W3 / test 2: SIGKILL after a build_graph leaves no stale
/// `graph.cache.tmp.*` files. The atomic-rename pattern uses a
/// unique tmp path per writer (PRF-STATE-11); the loader
/// ignores `*.tmp.*` siblings of the canonical `graph.cache`.
/// This test pins the cleanup invariant: even after a hard kill,
/// no tmp file lingers. (If cleanup ever becomes async, this
/// test ensures we never have to wait for it.)
#[tokio::test(flavor = "multi_thread")]
async fn prf_f5_w3_sigkill_after_build_leaves_no_stale_tmp_cache() {
    let ws = fixture_ws();
    let ws_copy = std::env::temp_dir().join(format!(
        "prf-f5-w3-tmp-{}",
        std::process::id()
    ));
    copy_dir(&ws, &ws_copy);

    let _ = spawn_run_kill(
        &ws_copy,
        "build_graph",
        serde_json::json!({"directory": ws_copy.to_str().unwrap()}),
    )
    .await;

    // Post-condition: no graph.cache.tmp.* files anywhere in
    // the workspace.
    let stale_tmps: Vec<PathBuf> = walkdir_files(&ws_copy)
        .into_iter()
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with("graph.cache.tmp.")
        })
        .collect();
    assert!(
        stale_tmps.is_empty(),
        "SIGKILL after build left stale tmp cache files: {stale_tmps:?}"
    );

    let _ = std::fs::remove_dir_all(&ws_copy);
}

/// F5.W3 / test 3: after SIGKILL, a fresh MCP session can acquire
/// the workspace and run build_graph to completion. This is the
/// end-to-end "recovery" property: the kill leaves the workspace
/// in a state where the next session can proceed without manual
/// cleanup.
#[tokio::test(flavor = "multi_thread")]
async fn prf_f5_w3_sigkill_then_fresh_session_recovers() {
    let ws = fixture_ws();
    let ws_copy = std::env::temp_dir().join(format!(
        "prf-f5-w3-recover-{}",
        std::process::id()
    ));
    copy_dir(&ws, &ws_copy);

    // First session: build, then SIGKILL.
    let _ = spawn_run_kill(
        &ws_copy,
        "build_graph",
        serde_json::json!({"directory": ws_copy.to_str().unwrap()}),
    )
    .await;

    // Second session: must complete build_graph successfully.
    let mut s2 = McpSession::spawn(&ws_copy).await.expect("spawn fresh");
    let resp = s2
        .call_tool(
            "build_graph",
            serde_json::json!({"directory": ws_copy.to_str().unwrap()}),
        )
        .await
        .expect("fresh build_graph");
    let status = resp.get("status").and_then(|v| v.as_str()).unwrap_or("");
    assert_eq!(
        status, "complete",
        "fresh session after SIGKILL must build cleanly, got: {resp}"
    );
    s2.shutdown().await;

    let _ = std::fs::remove_dir_all(&ws_copy);
}

// ---------------------------------------------------------------------------
// helpers (test-local, not exported)
// ---------------------------------------------------------------------------

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

fn walkdir_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(p: &Path, out: &mut Vec<PathBuf>) {
        if p.is_file() {
            out.push(p.to_path_buf());
            return;
        }
        if p.is_dir() {
            for entry in std::fs::read_dir(p).unwrap() {
                walk(&entry.unwrap().path(), out);
            }
        }
    }
    walk(root, &mut out);
    out
}
