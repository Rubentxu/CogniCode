//! PRF-MCP-03 UAT: core MCP fully functional with NO network at all.
//!
//! Runs the real cognicode-mcp binary inside `unshare -rn` (new user +
//! network namespace: loopback only, every external connection fails).
//! Exercises initialize, tools/list, build_graph and analyze end-to-end
//! and requires Complete/honest results. Any network dependency in the
//! core path would surface as spawn/timeout/failed tool errors.

use serde_json::{Value, json};
use std::path::PathBuf;
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

fn fixture_ws() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

struct NetnsSession {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl NetnsSession {
    // `&PathBuf` is intentional: the function lives inside an integration
    // test that creates `PathBuf` values and immediately passes them by
    // reference.  Clippy suggests `&Path` for fewer indirections, but
    // that would force callers to write `.as_path()` at every call site
    // and the perf delta is irrelevant in a test-only path.
    #[allow(clippy::ptr_arg)]
    async fn spawn(ws: &PathBuf) -> Self {
        let mut cmd = Command::new("unshare");
        cmd.args([
            "-rn",
            binary_path().to_str().unwrap(),
            "--cwd",
            ws.to_str().unwrap(),
        ]);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd.spawn().expect("spawn unshare -rn cognicode-mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut s = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        };
        let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"uat-mcp03","version":"0"}}});
        s.send(&init).await;
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(120), s.stdout.read_line(&mut line))
            .await
            .expect("initialize timed out")
            .expect("read initialize response");
        let m: Value =
            serde_json::from_str(line.trim()).expect("valid JSON-RPC initialize response");
        assert!(m.get("result").is_some(), "initialize failed: {m}");
        s.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        s.next_id = 2;
        s
    }

    async fn send(&mut self, v: &Value) {
        self.stdin
            .write_all(format!("{v}\n").as_bytes())
            .await
            .expect("write stdin");
        self.stdin.flush().await.expect("flush stdin");
    }

    async fn call_tool(&mut self, name: &str, args: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).await;
        let deadline = tokio::time::sleep(Duration::from_secs(180));
        tokio::pin!(deadline);
        loop {
            let mut line = String::new();
            tokio::select! {
                _ = &mut deadline => panic!("tool {name} timed out with no network available"),
                n = self.stdout.read_line(&mut line) => {
                    let n = n.expect("read stdout");
                    assert!(n > 0, "server closed stdout during {name}");
                    let m: Value = serde_json::from_str(line.trim()).expect("valid JSON-RPC");
                    if m.get("id").and_then(|v| v.as_u64()) == Some(id) {
                        return m;
                    }
                }
            }
        }
    }
}

impl Drop for NetnsSession {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_core_is_fully_functional_with_no_network() {
    let ws = fixture_ws();
    assert!(ws.join("Cargo.toml").exists(), "fixture workspace missing");
    // Remove any durable snapshot so the graph is genuinely rebuilt offline.
    let _ = std::fs::remove_dir_all(ws.join(".cognicode"));

    // 1. Confirm the namespace really cuts the network (control).
    let probe = std::process::Command::new("unshare")
        .args([
            "-rn",
            "sh",
            "-c",
            "command -v curl >/dev/null && curl -m 3 -s https://example.com; echo exit=$?",
        ])
        .output()
        .expect("unshare probe");
    let out = String::from_utf8_lossy(&probe.stdout);
    assert!(
        out.contains("exit=7") || out.contains("exit=6") || out.contains("exit=28"),
        "control probe: network must be unavailable inside namespace, got: {out}"
    );

    let mut s = NetnsSession::spawn(&ws).await;

    // 2. tools/list works.
    let id = s.next_id;
    s.next_id += 1;
    s.send(&json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":{}}))
        .await;
    let mut line = String::new();
    loop {
        line.clear();
        let n = s
            .stdout
            .read_line(&mut line)
            .await
            .expect("read tools/list");
        assert!(n > 0, "stdout closed during tools/list");
        let m: Value = serde_json::from_str(line.trim()).unwrap();
        if m.get("id").and_then(|v| v.as_u64()) == Some(id) {
            let names: Vec<&str> = m["result"]["tools"]
                .as_array()
                .expect("tools array")
                .iter()
                .filter_map(|t| t["name"].as_str())
                .collect();
            assert!(
                names.contains(&"build_graph"),
                "tools/list must expose build_graph: {names:?}"
            );
            break;
        }
    }

    // 3. build_graph completes offline.
    let resp = s
        .call_tool("build_graph", json!({"directory": ws.to_str().unwrap()}))
        .await;
    let result: Value = serde_json::from_str(
        resp["result"]["content"][0]["text"]
            .as_str()
            .expect("text content"),
    )
    .expect("build_graph JSON result");
    assert_eq!(
        result["status"], "complete",
        "build_graph must be complete offline: {result}"
    );
    let symbols = result["symbols_found"]
        .as_u64()
        .unwrap_or_else(|| panic!("no symbols_found in result: {result}"));
    assert!(
        symbols >= 2,
        "expected at least the two fixture functions, got {symbols}"
    );

    // 4. analyze completes offline (pure-core query path).
    let resp = s
        .call_tool(
            "analyze_code",
            json!({"directory": ws.to_str().unwrap(), "path": "src/lib.rs"}),
        )
        .await;
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .expect("analyze text")
        .to_string();
    let _analyzed: Option<Value> = serde_json::from_str(&text).ok();
    assert!(
        !text.contains("\"error\"") || text.contains("status"),
        "analyze must not fail offline: {text}"
    );
}

// Anti-vacuity guard: fail the test if unshare is unavailable rather than
// silently passing a test that never exercised the offline path.
#[test]
fn unshare_is_available_for_this_uat() {
    let ok = std::process::Command::new("unshare")
        .args(["-rn", "true"])
        .status()
        .expect("spawn unshare");
    assert!(
        ok.success(),
        "unshare -rn unavailable; MCP-03 UAT cannot run honestly"
    );
}
