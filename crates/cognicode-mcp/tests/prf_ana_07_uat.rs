//! PRF-ANA-07 UAT (real binary, stdio JSON-RPC):
//! against the massive-collision corpus (51 `init` homonyms: 1 local +
//! 50 sibling `d{1..50}.rs`), the published call graph must apply:
//!   1. visibility rule — `caller_in_lib -> init` resolves to the LOCAL
//!      `src/lib.rs:init`, never to a sibling module;
//!   2. single-candidate rule — `caller_in_lib -> compute` resolves to
//!      the unique cross-file `sibling_unique_compute.rs::compute`;
//!   3. ambiguity visibility — only 2 relationships are published (no
//!      fan-out to any of the 50 sibling `init` homonyms).

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

mod common;
use common::binary_path;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .parent()
        .expect("repo root")
        .join("docs/prf/fixtures/massive_collision_corpus")
}

struct McpChild {
    // `child` is consumed during `spawn()` via `child.stdin.take()` /
    // `child.stdout.take()`.  After construction the field is never
    // read directly — clippy `-D warnings` flags this as `dead_code`
    // even though it is load-bearing for the spawn flow.  The allow
    // below scopes the suppression to the field only; if the struct
    // grows a method that uses `self.child` it must be removed.
    #[allow(dead_code)]
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl McpChild {
    async fn spawn(workspace: &Path) -> Result<Self, String> {
        let mut cmd = Command::new(binary_path());
        cmd.arg("--cwd").arg(workspace);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let stdout = BufReader::new(stdout);
        let mut me = Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        me.initialize().await?;
        Ok(me)
    }

    async fn initialize(&mut self) -> Result<(), String> {
        let req = json!({
            "jsonrpc": "2.0",
            "id": self.next_id,
            "method": "initialize",
            "params": {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "ana07-uat", "version": "0.0.0" }
            }
        });
        self.next_id += 1;
        self.send_raw(&req).await?;
        let _ = self.read_response(self.next_id - 1).await?;
        let notif = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        self.send_raw(&notif).await
    }

    async fn send_raw(&mut self, value: &Value) -> Result<(), String> {
        let line = serde_json::to_string(value).map_err(|e| e.to_string())?;
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| e.to_string())?;
        self.stdin.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn read_response(&mut self, expected_id: u64) -> Result<Value, String> {
        let mut line = String::new();
        loop {
            line.clear();
            let n = self
                .stdout
                .read_line(&mut line)
                .await
                .map_err(|e| format!("read_line: {e}"))?;
            if n == 0 {
                return Err("server closed stdout before sending response".into());
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(trimmed)
                .map_err(|e| format!("non-JSON line {trimmed:?}: {e}"))?;
            match v.get("id").and_then(|x| x.as_u64()) {
                Some(id) if id == expected_id => return Ok(v),
                Some(_) => continue,
                None => continue,
            }
        }
    }

    async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        });
        self.send_raw(&req).await?;
        self.read_response(id).await
    }

    /// Call a tool and parse the JSON payload embedded in content[0].text.
    async fn call_tool_payload(&mut self, name: &str, args: Value) -> Result<Value, String> {
        let raw = self.call_tool(name, args).await?;
        let content = raw
            .pointer("/result/content/0/text")
            .and_then(|t| t.as_str())
            .unwrap_or_default();
        serde_json::from_str(content).map_err(|e| format!("{name}: payload not json: {e}"))
    }
}

#[tokio::test]
async fn massive_collision_resolution_over_real_binary() {
    let corpus = corpus_dir();
    assert!(
        corpus.join("src/lib.rs").exists(),
        "corpus fixture missing: {}",
        corpus.display()
    );
    assert!(
        binary_path().exists(),
        "release binary missing (build release first)"
    );

    let mut client = McpChild::spawn(&corpus).await.expect("spawn mcp binary");

    // Build the graph over the 51-homonym corpus.
    let build = client
        .call_tool_payload("build_graph", json!({}))
        .await
        .expect("build_graph");
    assert_eq!(
        build.get("status").and_then(|s| s.as_str()),
        Some("complete")
    );

    // The graph must contain exactly the 2 real call relationships —
    // none of the 50 sibling `init` declarations may leak in as targets.
    let edges = build
        .get("edges")
        .and_then(|e| e.as_array())
        .expect("edges array");
    assert_eq!(
        edges.len(),
        2,
        "exactly 2 relationships expected: {edges:?}"
    );

    // Ambiguity resolution over the real transport.
    let hier = client
        .call_tool_payload(
            "get_call_hierarchy",
            json!({ "symbol_name": "caller_in_lib", "direction": "outgoing", "depth": 1 }),
        )
        .await
        .expect("get_call_hierarchy");

    let calls = hier
        .get("calls")
        .and_then(|c| c.as_array())
        .expect("calls array");
    assert_eq!(calls.len(), 2, "2 resolved callees expected: {calls:?}");

    // Rule 1: visibility — init resolves to the LOCAL lib.rs, never a sibling.
    let init_call = calls
        .iter()
        .find(|c| c.get("symbol").and_then(|s| s.as_str()) == Some("init"))
        .expect("init callee must be resolved");
    let init_file = init_call.get("file").and_then(|f| f.as_str()).unwrap_or("");
    assert!(
        init_file.ends_with("src/lib.rs"),
        "visibility rule: init must resolve to local src/lib.rs, got {init_file}"
    );

    // Rule 2: single candidate — compute resolves to the unique cross-file decl.
    let compute_call = calls
        .iter()
        .find(|c| c.get("symbol").and_then(|s| s.as_str()) == Some("compute"))
        .expect("compute callee must be resolved");
    let compute_file = compute_call
        .get("file")
        .and_then(|f| f.as_str())
        .unwrap_or("");
    assert!(
        compute_file.ends_with("src/sibling_unique_compute.rs"),
        "single-candidate rule: compute must resolve to sibling_unique_compute.rs, got {compute_file}"
    );

    // Both resolutions carry full confidence (unambiguous per the rules).
    for c in [init_call, compute_call] {
        assert_eq!(
            c.get("confidence").and_then(|x| x.as_f64()),
            Some(1.0),
            "unambiguous resolution must publish confidence 1.0: {c:?}"
        );
    }
}
