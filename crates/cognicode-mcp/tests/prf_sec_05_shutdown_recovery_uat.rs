//! PRF-SEC-05 / PRF-MCP-06 UAT: shutdown and crash leave no corruption.
//!
//! Real-binary scenarios over the durable graph cache:
//! 1. A crashed writer (SIGKILL-equivalent: leftover `graph.cache.tmp`)
//!    must not corrupt the next session: the stale temp is ignored and
//!    the workspace rebuilds to a correct, complete graph.
//! 2. Graceful shutdown (stdin EOF): the server exits on its own and
//!    leaves no lock files behind.
//! 3. A failed/corrupt cache never yields a wrong-but-successful graph:
//!    the next build rebuilds from sources (correctness floor from
//!    §72/§73, re-verified at the process boundary).

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
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

fn fixture_source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

/// Each test gets a private copy of the fixture so parallel tests never
/// share (and destroy) each other's durable cache.
fn fresh_ws() -> PathBuf {
    let dst = std::env::temp_dir().join(format!("prf-sec05-ws-{}", std::process::id()))
        .join(format!("run-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()));
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
            std::fs::copy(e.path(), &d).unwrap();
        }
    }
}

struct Session {
    child: Child,
    stdin: Option<tokio::process::ChildStdin>,
    stdout: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl Session {
    async fn spawn(ws: &Path) -> Self {
        let mut cmd = Command::new(binary_path());
        cmd.arg("--cwd").arg(ws);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd.spawn().expect("spawn cognicode-mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut s = Self { child, stdin: Some(stdin), stdout: BufReader::new(stdout), next_id: 1 };
        s.send(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"uat-sec05","version":"0"}}})).await;
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(60), s.stdout.read_line(&mut line))
            .await.expect("init timeout").expect("read init");
        assert!(line.contains("\"result\""), "initialize failed: {line}");
        s.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await;
        s.next_id = 2;
        s
    }

    async fn send(&mut self, v: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        stdin.write_all(format!("{v}\n").as_bytes()).await.expect("write");
        stdin.flush().await.expect("flush");
    }

    async fn call_tool(&mut self, name: &str, args: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).await;
        let deadline = tokio::time::sleep(Duration::from_secs(120));
        tokio::pin!(deadline);
        loop {
            let mut line = String::new();
            tokio::select! {
                _ = &mut deadline => panic!("{name} timed out"),
                n = self.stdout.read_line(&mut line) => {
                    assert!(n.expect("read") > 0, "stdout closed during {name}");
                    let m: Value = serde_json::from_str(line.trim()).expect("json");
                    if m.get("id").and_then(|x| x.as_u64()) == Some(id) { return m; }
                }
            }
        }
    }

    async fn build_graph(&mut self, ws: &Path) -> Value {
        let resp = self.call_tool("build_graph", json!({"directory": ws.to_str().unwrap()})).await;
        serde_json::from_str(resp["result"]["content"][0]["text"].as_str().expect("text"))
            .expect("build_graph JSON")
    }
}

impl Drop for Session {
    fn drop(&mut self) { let _ = self.child.start_kill(); }
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_temp_file_from_crashed_writer_is_not_trusted() {
    let ws = fresh_ws();
    let cache_dir = ws.join(".cognicode");
    let _ = std::fs::remove_dir_all(&cache_dir);

    // Session 1: build a good cache and record the correct inventory.
    let expected;
    {
        let mut s = Session::spawn(&ws).await;
        let g = s.build_graph(&ws).await;
        assert_eq!(g["status"], "complete");
        expected = g["symbols_found"].as_u64().unwrap();
    }

    // Simulate a crash mid-persistence: a leftover temp with GARBAGE
    // bytes (what a SIGKILL between write and rename would leave behind).
    std::fs::write(cache_dir.join("graph.cache.tmp"), b"half-written-garbage").unwrap();

    // Session 2: must recover — rebuild correctly, never load garbage.
    let mut s2 = Session::spawn(&ws).await;
    let g2 = s2.build_graph(&ws).await;
    assert_eq!(g2["status"], "complete", "recovery build must complete: {g2}");
    assert_eq!(
        g2["symbols_found"].as_u64().unwrap(),
        expected,
        "recovered inventory must match the good build"
    );

    let _ = std::fs::remove_dir_all(&cache_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn graceful_stdin_shutdown_exits_cleanly_without_locks() {
    let ws = fresh_ws();
    let _ = std::fs::remove_dir_all(ws.join(".cognicode"));

    let mut s = Session::spawn(&ws).await;
    let g = s.build_graph(&ws).await;
    assert_eq!(g["status"], "complete");

    // Close stdin (client disconnect). The server must exit on its own
    // within a bounded time — no orphaned process.
    drop(s.stdin.take()); // drop stdin -> EOF
    let exited = tokio::time::timeout(Duration::from_secs(30), s.child.wait()).await;
    assert!(
        exited.is_ok() && exited.unwrap().is_ok(),
        "server must exit after client disconnect (no orphan)"
    );

    // No lock or temp residue after a clean exit.
    let cache_dir = ws.join(".cognicode");
    if cache_dir.exists() {
        for entry in std::fs::read_dir(&cache_dir).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().to_string();
            assert!(
                !name.ends_with(".tmp") && !name.contains(".lock"),
                "clean shutdown must leave no temp/lock residue, found: {name}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&cache_dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn corrupt_cache_rebuilds_to_correct_inventory() {
    let ws = fresh_ws();
    let cache_dir = ws.join(".cognicode");
    let _ = std::fs::remove_dir_all(&cache_dir);

    // Build once to get the truthful inventory.
    let expected;
    {
        let mut s = Session::spawn(&ws).await;
        let g = s.build_graph(&ws).await;
        expected = g["symbols_found"].as_u64().unwrap();
    }
    assert!(expected >= 2);

    // Corrupt the durable cache in place.
    let cache = cache_dir.join("graph.cache");
    assert!(cache.exists(), "durable cache must exist after a build");
    std::fs::write(&cache, b"c0rrupt-not-a-snapshot").unwrap();

    // The next session must not serve the corrupt bytes as a graph:
    // rebuild must restore the identical inventory.
    let mut s = Session::spawn(&ws).await;
    let g = s.build_graph(&ws).await;
    assert_eq!(g["status"], "complete");
    assert_eq!(
        g["symbols_found"].as_u64().unwrap(),
        expected,
        "corrupt cache must never change the served inventory"
    );
    let _ = std::fs::remove_dir_all(&cache_dir);
}
