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

use serde_json::{Value, json};
use std::path::PathBuf;

mod common;
use common::McpSession;

fn fixture_ws() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

/// Helper: enumerate every tool the runtime advertises, following the
/// `nextCursor` chain in the response. Specific to PRF-SEC-02: the
/// paginated walk is required by the contract ("advertised in tools/list
/// (all pages)").
async fn all_tool_names(session: &mut McpSession) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut params = serde_json::json!({});
        if let Some(c) = &cursor {
            params["cursor"] = json!(c);
        }
        let resp = session
            .request("tools/list", params)
            .await
            .expect("tools/list request");
        for t in resp["result"]["tools"].as_array().expect("tools") {
            names.push(t["name"].as_str().unwrap().to_string());
        }
        cursor = resp["result"]["nextCursor"]
            .as_str()
            .or_else(|| resp["result"]["next_cursor"].as_str())
            .map(|s| s.to_string());
        if cursor.is_none() {
            break;
        }
    }
    names
}

/// Helper: paginate through every tool and extract (name, mutates_workspace).
async fn all_tools_with_mutates(session: &mut McpSession) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut params = serde_json::json!({});
        if let Some(c) = &cursor {
            params["cursor"] = json!(c);
        }
        let resp = session
            .request("tools/list", params)
            .await
            .expect("tools/list request");
        for t in resp["result"]["tools"].as_array().expect("tools") {
            let name = t["name"].as_str().unwrap().to_string();
            let mutates = t["_meta"]["cognicode"]["mutates_workspace"]
                .as_bool()
                .unwrap_or(false);
            out.push((name, mutates));
        }
        cursor = resp["result"]["nextCursor"]
            .as_str()
            .or_else(|| resp["result"]["next_cursor"].as_str())
            .map(|s| s.to_string());
        if cursor.is_none() {
            break;
        }
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn read_only_mode_hides_and_rejects_mutating_tools_but_keeps_reads() {
    let ws = fixture_ws();
    let canary = ws.join("sec02_canary.txt");
    let _ = std::fs::remove_file(&canary);

    let mut s = McpSession::spawn_with_flags(&ws, &["--read-only"])
        .await
        .expect("spawn cognicode-mcp --read-only");

    // 1. tools/list (paginated) must NOT advertise write_file/edit_file.
    let names = all_tool_names(&mut s).await;
    assert!(
        !names.contains(&"write_file".to_string()),
        "write_file must not be advertised read-only: {names:?}"
    );
    assert!(
        !names.contains(&"edit_file".to_string()),
        "edit_file must not be advertised read-only"
    );
    assert!(
        names.contains(&"build_graph".to_string()),
        "read tools must remain advertised"
    );

    // 2. Direct tools/call to write_file is rejected; nothing written.
    let resp = s
        .request(
            "tools/call",
            json!({"name": "write_file", "arguments": {"path": "sec02_canary.txt", "content": "pwned"}}),
        )
        .await
        .expect("tools/call request");
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    let is_error =
        resp["result"]["isError"].as_bool().unwrap_or(false) || text.contains("read_only_mode");
    assert!(is_error, "write_file must fail in read-only mode: {resp}");
    assert!(
        !canary.exists(),
        "read-only mode must not write the canary file"
    );

    // 2b. PRF-EXT-01: metadata must expose the permission flag for every
    // advertised tool, with mutators exactly matching the declared set.
    let tools = all_tools_with_mutates(&mut s).await;
    let total = tools.len();
    let flagged: Vec<&str> = tools
        .iter()
        .filter(|(_, m)| *m)
        .map(|(n, _)| n.as_str())
        .collect();
    assert!(
        flagged.is_empty(),
        "read-only mode advertises no mutating tools, so none may be flagged: {flagged:?}"
    );
    assert!(
        total >= 70,
        "expected the full read tool surface, got {total}"
    );

    // 3. Read tools still work: build_graph completes.
    let resp = s
        .request(
            "tools/call",
            json!({"name": "build_graph", "arguments": {"directory": ws.to_str().unwrap()}}),
        )
        .await
        .expect("tools/call request");
    let result: Value =
        serde_json::from_str(resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        result["status"], "complete",
        "build_graph must work read-only: {result}"
    );

    let _ = std::fs::remove_file(&canary);
}

#[tokio::test(flavor = "multi_thread")]
async fn default_mode_keeps_write_file_available() {
    let ws = fixture_ws();
    let target = ws.join("sec02_default_canary.txt");
    let _ = std::fs::remove_file(&target);

    let mut s = McpSession::spawn(&ws)
        .await
        .expect("spawn cognicode-mcp default mode");
    let names = all_tool_names(&mut s).await;
    assert!(
        names.contains(&"write_file".to_string()),
        "default mode must keep write_file advertised"
    );

    // PRF-EXT-01: default mode flags exactly the three mutating tools.
    let tools = all_tools_with_mutates(&mut s).await;
    let mut flagged: Vec<String> = tools
        .into_iter()
        .filter(|(_, m)| *m)
        .map(|(n, _)| n)
        .collect();
    flagged.sort();
    assert_eq!(
        flagged,
        vec![
            "edit_file".to_string(),
            "reparse_on_edit".to_string(),
            "write_file".to_string()
        ],
        "exactly the declared mutating tools must be flagged"
    );

    let resp = s
        .request(
            "tools/call",
            json!({"name": "write_file", "arguments": {"path": "sec02_default_canary.txt", "content": "ok"}}),
        )
        .await
        .expect("tools/call request");
    assert!(
        !resp["result"]["isError"].as_bool().unwrap_or(false),
        "write_file must work in default mode: {resp}"
    );
    assert!(target.exists(), "default-mode write must create the file");

    let _ = std::fs::remove_file(&target);
}
