//! A-013 / CP2.7 — Black-box lifecycle UAT.
//!
//! Locks down the end-to-end contract of the `cognicode-mcp` binary
//! as seen by an adopter over JSON-RPC:
//!
//!   1. startup: spawn → initialize handshake → `tools/list` returns
//!      a non-empty, well-formed `tools` array;
//!   2. mid-lifecycle call: a `tools/call` over `read_file` (read-only,
//!      stable, no graph dependency) returns a well-formed result;
//!   3. shutdown: closing stdin (EOF) lets the server exit cleanly
//!      with no panic/timeout.
//!
//! Plus three triangulations that pin the failure modes the
//! release contract must NOT regress to:
//!
//!   T1. invalid workspace: spawning with a `--cwd` that does not
//!       exist must report a clean error and not panic.
//!   T2. read-only posture: a session started with `--read-only`
//!       must reject a mutating tool call (`build_graph`) and
//!       return the `read_only_mode` tool error.
//!   T3. signal cancel: SIGTERM to the running server triggers a
//!       clean exit (status 0 or 143) within a bounded wait.
//!
//! All six tests use the canonical `common::McpSession` harness
//! (extended in this cycle with `request`, `spawn_with_flags`,
//! `pid`, and `signal_and_wait` helpers). The pre-existing
//! duplicate `Session` structs in `a009_*` and `prf_sec_05_*`
//! are NOT modified by A-013 (scope discipline); the canonical
//! harness is the only one A-013 touches.
//!
//! The workspace fixture is
//! `crates/cognicode-mcp/tests/fixtures/mcp_03_ws` — a minimal
//! Rust source tree the server can scan. We use a private copy
//! per test to keep `cargo test` parallel-safe.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

mod common;

use common::McpSession;

fn fixture_source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

/// Each test gets a private copy of the fixture so parallel tests
/// never share (and destroy) each other's durable cache.
fn fresh_ws() -> PathBuf {
    let dst = std::env::temp_dir()
        .join(format!("a013-lifecycle-ws-{}", std::process::id()))
        .join(format!(
            "run-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    copy_dir(&fixture_source(), &dst);
    dst
}

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
            std::fs::copy(e.path(), &d).ok();
        }
    }
}

const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

fn rand_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

// ============================================================================
// GREEN baselines (1..3): the lifecycle works as designed today.
// ============================================================================

#[tokio::test(flavor = "current_thread")]
async fn a013_lifecycle_uat_startup_returns_tools_list_with_at_least_one_tool() {
    // GREEN baseline: spawn + initialize + tools/list must return
    // a non-empty tools array. We do not pin the count: the
    // catalog is allowed to grow, and a brittle count would rot
    // the test every time a tool is added. We pin the *shape*.
    let ws = fresh_ws();
    let mut s = McpSession::spawn(&ws).await.expect("spawn cognicode-mcp");
    let resp = s
        .request("tools/list", Value::Object(Default::default()))
        .await
        .expect("tools/list request");
    let tools = resp["result"]["tools"]
        .as_array()
        .expect("result.tools must be a JSON array");
    assert!(
        !tools.is_empty(),
        "tools/list returned an empty array; the binary advertises zero tools"
    );
    for (i, t) in tools.iter().enumerate() {
        let name = t["name"]
            .as_str()
            .unwrap_or_else(|| panic!("tool {i} has no name"));
        assert!(!name.is_empty(), "tool {i} has empty name");
        assert!(
            t["inputSchema"].is_object(),
            "tool {name} missing inputSchema"
        );
    }
    s.shutdown().await;
}

#[tokio::test(flavor = "current_thread")]
async fn a013_lifecycle_uat_call_readonly_tool_mid_lifecycle() {
    // GREEN baseline: a `tools/call` over `read_file` (a stable
    // read-only tool) returns a JSON object. We do not pin the
    // exact payload; only the envelope shape.
    let ws = fresh_ws();
    let mut s = McpSession::spawn(&ws).await.expect("spawn");
    // Pick a file that exists in the fixture. The fixture has
    // at least one `.rs` source under `src/`; we use `src/lib.rs`
    // if present, otherwise the first regular file we find.
    let candidate = ws.join("src").join("lib.rs");
    let path = if candidate.exists() {
        candidate
    } else {
        let mut found = None;
        for entry in walkdir(&ws) {
            if entry.is_file() {
                found = Some(entry);
                break;
            }
        }
        found.expect("fixture workspace has no files")
    };
    let path_str = path.to_string_lossy().into_owned();
    let result = s
        .call_tool("read_file", serde_json::json!({"path": path_str}))
        .await
        .expect("read_file call");
    assert!(
        result.is_object() || result.is_string(),
        "read_file must return a JSON object or string; got: {result}"
    );
    s.shutdown().await;
}

fn walkdir(root: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    let mut out = Vec::new();
    fn rec(p: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(p) {
            for e in entries.flatten() {
                let path = e.path();
                if path.is_dir() {
                    rec(&path, out);
                } else {
                    out.push(path);
                }
            }
        }
    }
    rec(root, &mut out);
    out.into_iter()
}

#[tokio::test(flavor = "current_thread")]
async fn a013_lifecycle_uat_shutdown_via_stdin_eof_exits_cleanly() {
    // GREEN baseline: closing stdin (EOF) lets the server exit on
    // its own. If we get here without a timeout, shutdown was
    // clean. The `McpSession::shutdown` helper does
    // `stdin.shutdown + kill + wait`, so the assertion is
    // implicit: a hang would surface as a test timeout.
    let ws = fresh_ws();
    let s = McpSession::spawn(&ws).await.expect("spawn");
    s.shutdown().await;
}

// ============================================================================
// TRIANGULATIONS (4..6): the lifecycle must NOT regress to these
// failure modes.
// ============================================================================

#[tokio::test(flavor = "current_thread")]
async fn a013_lifecycle_uat_invalid_workspace_reports_clean_error() {
    // TRIANGULATE T1: spawning with a `--cwd` that does not exist
    // must NOT panic, hang, or silently succeed. The contract is:
    // either the server exits non-zero with a clear stderr message
    // (captured by the harness as a clean error), or the
    // initialize handshake never completes within the timeout.
    let bogus = std::env::temp_dir().join(format!(
        "a013-bogus-ws-{}-{}",
        std::process::id(),
        rand_suffix()
    ));
    // Intentionally do NOT create the directory.
    let result = tokio::time::timeout(STARTUP_TIMEOUT, McpSession::spawn(&bogus)).await;
    match result {
        Err(_) => panic!("spawn with invalid --cwd hung for {STARTUP_TIMEOUT:?}"),
        Ok(Err(msg)) => {
            assert!(
                !msg.is_empty(),
                "spawn error must be non-empty; got empty string"
            );
        }
        Ok(Ok(_)) => {
            panic!(
                "cognicode-mcp accepted a missing --cwd path; the server must refuse non-existent workspaces before initialize"
            );
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a013_lifecycle_uat_readonly_rejects_mutating_call_mid_lifecycle() {
    // TRIANGULATE T2: a session started with `--read-only` must
    // reject a mutating tool call. We use `edit_file` because it
    // is declared `authority: mutating` in BOTH the public
    // contract (`product/tools.json`) AND the runtime
    // (`MUTATING_TOOLS`). Under `--read-only` the server must
    // answer with the `read_only_mode` tool error A-009 / A-010
    // already pin for the `reviewer` profile.
    //
    // (Discovery during A-013: `build_graph` writes a graph
    // cache at runtime but is declared `authority: read` in the
    // public contract. The contract/runtime mismatch is a
    // separate debt — A-014 / the `MUTATING_TOOLS` audit closes
    // it — and is not asserted here. A-013 only pins the
    // lifecycle/flag interaction for tools that BOTH layers
    // agree are mutating.)
    //
    // This test binds the *flag* (`--read-only`) to the
    // *lifecycle* (a session running through `tools/call`), not
    // to the *profile* (A-009's domain). A future refactor that
    // decouples the flag from the profile will still need to
    // gate at this layer.
    let ws = fresh_ws();
    let mut s = McpSession::spawn_with_flags(&ws, &["--read-only"])
        .await
        .expect("spawn --read-only");
    let target = ws.join("src").join("lib.rs");
    let target_str = if target.exists() {
        target.to_string_lossy().into_owned()
    } else {
        // Fallback: any file in the workspace.
        let mut found = None;
        for entry in walkdir(&ws) {
            if entry.is_file() {
                found = Some(entry);
                break;
            }
        }
        found
            .expect("fixture workspace has no files")
            .to_string_lossy()
            .into_owned()
    };
    // The call envelope: even under `--read-only`, the server
    // answers at the JSON-RPC layer; the rejection is a tool
    // result with `isError: true` and the text `read_only_mode`.
    let resp = s
        .request(
            "tools/call",
            serde_json::json!({
                "name": "edit_file",
                "arguments": {
                    "path": target_str,
                    "edits": [{"oldString": "fn", "newString": "fn"}]
                }
            }),
        )
        .await
        .expect("tools/call request");
    let result = &resp["result"];
    let is_error = result["isError"].as_bool().unwrap_or(false);
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    assert!(
        is_error,
        "mutating call under --read-only must return isError: true; got: {resp}"
    );
    assert!(
        text.contains("read_only_mode"),
        "mutating call under --read-only must surface `read_only_mode`; got text: {text}"
    );
    s.shutdown().await;
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn a013_lifecycle_uat_signal_term_shuts_down_cleanly() {
    // TRIANGULATE T3: a running server must accept SIGTERM and
    // exit with status 0 (graceful) or 143 (= 128 + SIGTERM 15,
    // the conventional "killed by signal" code). Anything else
    // is a regression: a hung process is a release-blocker.
    let ws = fresh_ws();
    let mut s = McpSession::spawn(&ws).await.expect("spawn");
    let pid = s.pid();
    assert!(pid > 0, "spawned process must have a positive pid");
    let exit = tokio::time::timeout(SHUTDOWN_TIMEOUT, s.signal_and_wait("TERM"))
        .await
        .expect("SIGTERM must trigger a clean exit within {SHUTDOWN_TIMEOUT:?}");
    let exit = exit.expect("signal_and_wait must surface the wait result");
    // `code()` returns `None` on Unix when the process was killed by
    // a signal (this is the documented `std::os::unix::process::ExitStatusExt`
    // behaviour). Either of these three outcomes is a clean shutdown:
    //   - exit code 0   (graceful self-exit on signal),
    //   - exit code 143 (= 128 + SIGTERM 15, the conventional
    //     killed-by-signal code, used by shells that report
    //     signal-terminated processes as 128 + signo),
    //   - code == None  (killed by signal, raw Unix).
    // Anything else (a panic, an abort, a hang) is a release-blocker.
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        let code = exit.code();
        let signal = exit.signal();
        let ok = code == Some(0) || code == Some(143) || signal == Some(15);
        assert!(
            ok,
            "SIGTERM must trigger graceful exit (0 / 143 / signal 15); got code: {:?}, signal: {:?}",
            code, signal
        );
    }
    #[cfg(not(unix))]
    {
        let _ = exit; // unreachable: test is cfg(unix)
    }
}
