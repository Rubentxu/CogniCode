//! PRF-SEC-02 UAT: read/write capability differentiation in MCP.
//!
//! Real-binary tests proving the `--read-only` contract:
//! 1. Default mode: mutating tools (write_file) ARE available (documented
//!    behavior preserved).
//! 2. Read-only mode: mutating tools are NOT advertised in tools/list
//!    (all pages) and a direct tools/call to write_file is rejected with
//!    an honest error, writing NOTHING to disk.
//! 3. Read-only mode: read tools (build_graph, get_file_symbols) keep
//!    working (differentiated, not blanket denial).
//!
//! RED→GREEN: before this change there was no way to run the MCP server
//! read-only; write_file was always callable.

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

fn fixture_ws() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

struct Session {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl Session {
    async fn spawn(ws: &Path, read_only: bool) -> Self {
        let mut cmd = Command::new(binary_path());
        cmd.arg("--cwd").arg(ws);
        if read_only {
            cmd.arg("--read-only");
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd.spawn().expect("spawn cognicode-mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut s = Self { child, stdin, stdout: BufReader::new(stdout), next_id: 1 };
        s.send(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"uat-sec02","version":"0"}}})).await;
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(60), s.stdout.read_line(&mut line))
            .await.expect("init timeout").expect("read init");
        assert!(line.contains("\"result\""), "initialize failed: {line}");
        s.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await;
        s.next_id = 2;
        s
    }

    async fn send(&mut self, v: &Value) {
        self.stdin.write_all(format!("{v}\n").as_bytes()).await.expect("write");
        self.stdin.flush().await.expect("flush");
    }

    async fn request(&mut self, v: Value) -> Value {
        let id = v["id"].as_u64().unwrap();
        self.send(&v).await;
        let deadline = tokio::time::sleep(Duration::from_secs(120));
        tokio::pin!(deadline);
        loop {
            let mut line = String::new();
            tokio::select! {
                _ = &mut deadline => panic!("timeout waiting for id {id}"),
                n = self.stdout.read_line(&mut line) => {
                    assert!(n.expect("read") > 0, "stdout closed");
                    let m: Value = serde_json::from_str(line.trim()).expect("json");
                    if m.get("id").and_then(|x| x.as_u64()) == Some(id) { return m; }
                }
            }
        }
    }

    /// Collect every tool name across all tools/list pages.
    async fn all_tool_names(&mut self) -> Vec<String> {
        let mut names = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let id = self.next_id;
            self.next_id += 1;
            let mut params = json!({});
            if let Some(c) = &cursor {
                params["cursor"] = json!(c);
            }
            let resp = self.request(json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":params})).await;
            for t in resp["result"]["tools"].as_array().expect("tools") {
                names.push(t["name"].as_str().unwrap().to_string());
            }
            cursor = resp["result"]["nextCursor"]
                .as_str()
                .or_else(|| resp["result"]["next_cursor"].as_str())
                .map(|s| s.to_string());
            if cursor.is_none() { break; }
        }
        names
    }

    async fn call_tool(&mut self, name: &str, args: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.request(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).await
    }
}

impl Drop for Session {
    fn drop(&mut self) { let _ = self.child.start_kill(); }
}

#[tokio::test(flavor = "multi_thread")]
async fn read_only_mode_hides_and_rejects_mutating_tools_but_keeps_reads() {
    let ws = fixture_ws();
    let canary = ws.join("sec02_canary.txt");
    let _ = std::fs::remove_file(&canary);

    let mut s = Session::spawn(&ws, true).await;

    // 1. tools/list (paginated) must NOT advertise write_file/edit_file.
    let names = s.all_tool_names().await;
    assert!(!names.contains(&"write_file".to_string()), "write_file must not be advertised read-only: {names:?}");
    assert!(!names.contains(&"edit_file".to_string()), "edit_file must not be advertised read-only");
    assert!(names.contains(&"build_graph".to_string()), "read tools must remain advertised");

    // 2. Direct tools/call to write_file is rejected; nothing written.
    let resp = s.call_tool("write_file", json!({"path": "sec02_canary.txt", "content": "pwned"})).await;
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or_default();
    let is_error = resp["result"]["isError"].as_bool().unwrap_or(false)
        || text.contains("read_only_mode");
    assert!(is_error, "write_file must fail in read-only mode: {resp}");
    assert!(!canary.exists(), "read-only mode must not write the canary file");

    // 2b. PRF-EXT-01: metadata must expose the permission flag for every
    // advertised tool, with mutators exactly matching the declared set.
    let id = s.next_id;
    s.next_id += 1;
    let mut cursor: Option<String> = None;
    let mut flagged = Vec::new();
    let mut total_flagged = 0;
    loop {
        let mut params = json!({});
        if let Some(c) = &cursor {
            params["cursor"] = json!(c);
        }
        let resp = s.request(json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":params})).await;
        for t in resp["result"]["tools"].as_array().unwrap() {
            let name = t["name"].as_str().unwrap();
            let m = &t["_meta"]["cognicode"]["mutates_workspace"];
            assert!(!m.is_null(), "tool {name} must expose mutates_workspace metadata");
            if m.as_bool().unwrap() {
                flagged.push(name.to_string());
            }
            total_flagged += 1;
        }
        cursor = resp["result"]["nextCursor"]
            .as_str()
            .or_else(|| resp["result"]["next_cursor"].as_str())
            .map(|s| s.to_string());
        if cursor.is_none() { break; }
    }
    assert!(
        flagged.is_empty(),
        "read-only mode advertises no mutating tools, so none may be flagged: {flagged:?}"
    );
    assert!(total_flagged >= 70, "expected the full read tool surface, got {total_flagged}");

    // 3. Read tools still work: build_graph completes.
    let resp = s.call_tool("build_graph", json!({"directory": ws.to_str().unwrap()})).await;
    let result: Value = serde_json::from_str(resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(result["status"], "complete", "build_graph must work read-only: {result}");

    let _ = std::fs::remove_file(&canary);
}

#[tokio::test(flavor = "multi_thread")]
async fn default_mode_keeps_write_file_available() {
    let ws = fixture_ws();
    let target = ws.join("sec02_default_canary.txt");
    let _ = std::fs::remove_file(&target);

    let mut s = Session::spawn(&ws, false).await;
    let names = s.all_tool_names().await;
    assert!(names.contains(&"write_file".to_string()), "default mode must keep write_file advertised");

    // PRF-EXT-01: default mode flags exactly the three mutating tools.
    let mut flagged = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let id = s.next_id;
        s.next_id += 1;
        let mut params = json!({});
        if let Some(c) = &cursor {
            params["cursor"] = json!(c);
        }
        let resp = s.request(json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":params})).await;
        for t in resp["result"]["tools"].as_array().unwrap() {
            if t["_meta"]["cognicode"]["mutates_workspace"].as_bool().unwrap_or(false) {
                flagged.push(t["name"].as_str().unwrap().to_string());
            }
        }
        cursor = resp["result"]["nextCursor"]
            .as_str()
            .or_else(|| resp["result"]["next_cursor"].as_str())
            .map(|s| s.to_string());
        if cursor.is_none() { break; }
    }
    flagged.sort();
    assert_eq!(
        flagged,
        vec!["edit_file".to_string(), "reparse_on_edit".to_string(), "write_file".to_string()],
        "exactly the declared mutating tools must be flagged"
    );

    let resp = s.call_tool("write_file", json!({"path": "sec02_default_canary.txt", "content": "ok"})).await;
    assert!(!resp["result"]["isError"].as_bool().unwrap_or(false), "write_file must work in default mode: {resp}");
    assert!(target.exists(), "default-mode write must create the file");

    let _ = std::fs::remove_file(&target);
}
