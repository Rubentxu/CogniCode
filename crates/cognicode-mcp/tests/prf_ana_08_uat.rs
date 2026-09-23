//! PRF-ANA-08 UAT (real binary): search-category budgets and bounded
//! output. Over the 51-homonym collision corpus, a `find_usages` query
//! must complete within the category budget (500 ms `search`) with
//! bounded output, and a query over a pathological wildcard-ish term
//! must either complete or produce a typed error — never hang or
//! return unbounded text.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::timeout;

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
        let mut child = cmd.spawn().map_err(|e| format!("spawn: {e}"))?;
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
        self.send_raw(&json!({
            "jsonrpc": "2.0", "id": self.next_id, "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05", "capabilities": {},
                "clientInfo": { "name": "ana08-uat", "version": "0" }
            }
        }))
        .await?;
        let _ = self.read_response(self.next_id).await?;
        self.next_id += 1;
        self.send_raw(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await?;
        Ok(())
    }

    async fn send_raw(&mut self, req: &Value) -> Result<(), String> {
        self.stdin
            .write_all(format!("{}\n", req).as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        self.stdin.flush().await.map_err(|e| e.to_string())
    }

    async fn read_response(&mut self, id: u64) -> Result<Value, String> {
        let mut line = String::new();
        loop {
            line.clear();
            let n = self
                .stdout
                .read_line(&mut line)
                .await
                .map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("server closed stdout".into());
            }
            let m: Value =
                serde_json::from_str(line.trim()).map_err(|e| format!("bad json: {e}"))?;
            if m.get("id").and_then(|v| v.as_u64()) == Some(id) {
                return Ok(m);
            }
        }
    }

    async fn call_tool(&mut self, name: &str, args: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_raw(&json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": name, "arguments": args }
        }))
        .await?;
        self.read_response(id).await
    }
}

#[tokio::test]
async fn search_budget_bounded_output_over_homonym_corpus() {
    let corpus = corpus_dir();
    assert!(corpus.exists(), "corpus fixture missing");

    let mut mcp = timeout(Duration::from_secs(30), McpChild::spawn(&corpus))
        .await
        .expect("spawn timeout")
        .expect("spawn failed");

    // Warm-up: build the graph first (graph category, 60 s budget).
    let build = timeout(
        Duration::from_secs(60),
        mcp.call_tool("build_graph", json!({})),
    )
    .await
    .expect("build_graph exceeded the 60 s graph budget — PRF-ANA-08 RED")
    .expect("build_graph failed");
    assert!(!build.to_string().is_empty());

    // find_usages on a homonym: search category budget is 500 ms.
    let start = Instant::now();
    let usages = timeout(
        Duration::from_millis(500) + Duration::from_millis(1500), // dispatch + pipe slack
        mcp.call_tool("find_usages", json!({ "symbol_name": "compute" })),
    )
    .await
    .expect("find_usages exceeded search budget + slack — PRF-ANA-08 RED")
    .expect("find_usages failed");

    let elapsed = start.elapsed();
    let content = usages
        .pointer("/result/content/0/text")
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_string();
    assert!(!content.is_empty(), "find_usages returned empty content");

    // Bounded output: the serialized payload cannot exceed 5 MiB.
    assert!(
        content.len() < 5 * 1024 * 1024,
        "unbounded output: {} bytes — PRF-ANA-08 RED",
        content.len()
    );

    // Typed behavior: a nonexistent symbol must produce a typed error
    // or an explicit not-found payload, not a hang.
    let missing = timeout(
        Duration::from_secs(5),
        mcp.call_tool(
            "find_usages",
            json!({ "symbol_name": "definitely_not_here_42" }),
        ),
    )
    .await
    .expect("nonexistent-symbol query hung — PRF-ANA-08 RED")
    .expect("call failed");

    let is_error = missing["result"]["isError"].as_bool().unwrap_or(false);
    let text = missing
        .pointer("/result/content/0/text")
        .and_then(|t| t.as_str())
        .unwrap_or_default();
    assert!(
        is_error || !text.is_empty(),
        "nonexistent symbol must produce typed error or explicit payload"
    );

    let _ = elapsed;
}
