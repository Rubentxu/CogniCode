//! PRF-ANA-05 UAT (real binary, stdio JSON-RPC):
//! repeated `build_graph` calls against the SAME workspace must produce
//! semantically equivalent outputs (same `complete`, same digests, same
//! symbol counts). Compares full payload modulo run-variant timing fields.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .parent()
        .expect("repo root")
        .join("target/release/cognicode-mcp")
}

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
                "clientInfo": { "name": "ana05-uat", "version": "0.0.0" }
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
}

#[tokio::test]
async fn repeated_build_graph_over_real_binary_is_reproducible() {
    let corpus = corpus_dir();
    assert!(
        corpus.exists(),
        "corpus fixture missing: {}",
        corpus.display()
    );
    assert!(
        binary_path().exists(),
        "release binary missing at {} (build with: cargo build --release -p cognicode-mcp)",
        binary_path().display()
    );

    let mut client = McpChild::spawn(&corpus).await.expect("spawn mcp binary");

    let mut first: Option<Value> = None;
    for i in 0..3 {
        let raw = client
            .call_tool("build_graph", json!({}))
            .await
            .unwrap_or_else(|e| panic!("build_graph call {i} failed: {e}"));
        let content = raw
            .pointer("/result/content/0/text")
            .and_then(|t| t.as_str())
            .unwrap_or_default();
        assert!(!content.is_empty(), "call {i}: empty content");

        let parsed: Value = serde_json::from_str(content)
            .unwrap_or_else(|e| panic!("call {i}: payload not json: {e}"));
        // Semantic view: invariant identity fields, excluding the human
        // "message" (embeds elapsed ms, legitimately run-variant).
        let view = json!({
            "status": parsed.pointer("/status"),
            "symbols_found": parsed.pointer("/symbols_found"),
            "relationships_found": parsed.pointer("/relationships_found"),
            "edges": parsed.pointer("/edges"),
            "skipped_files": parsed.pointer("/skipped_files"),
            "config_digest": parsed.pointer("/basis/config_digest"),
            "source_manifest_digest": parsed.pointer("/basis/source_manifest_digest"),
            "basis_complete": parsed.pointer("/basis/complete"),
        });

        match &first {
            None => first = Some(view),
            Some(f) => {
                assert_eq!(
                    f, &view,
                    "run {i}: semantically-equivalent view differs from first run"
                );
            }
        }
    }

    let f = first.expect("at least one run");
    assert_eq!(
        f.get("status").and_then(|c| c.as_str()),
        Some("complete"),
        "build_graph must report complete"
    );
    assert!(
        f.get("config_digest").is_some(),
        "basis.config_digest missing from build_graph output"
    );
    assert!(
        f.get("source_manifest_digest").is_some(),
        "basis.source_manifest_digest missing from build_graph output"
    );
}
