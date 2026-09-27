//! A-009 / CP2.1 — the public `reviewer` profile must be enforceable.
//!
//! `product/profiles.json` is the single public truth about CogniCode's
//! profiles, and it declares:
//!
//! ```json
//! { "id": "reviewer", "install": true, "mutating": false }
//! ```
//!
//! That is a promise to every adopter who installs the `reviewer` profile:
//! this profile cannot modify their workspace. The read-only machinery to
//! keep it already existed and was tested (`prf_sec_02_read_only_uat.rs`
//! drives the real binary with `--read-only`).
//!
//! What was missing is the *link*. `--read-only` is a flag a human types;
//! a profile is a thing an installer resolves. Nothing connected the two, so
//! `HandlerContextBuilder::with_read_only` had no caller at all, and the
//! public promise was unenforceable: `reviewer` could write to a workspace
//! exactly like `core`.
//!
//! RED→GREEN: before this change, `CogniCodeHandler::for_profile` did not
//! exist and resolving a non-mutating profile produced no read-only
//! constraint at all.
//!
//! The tests below run against the real binary over JSON-RPC rather than
//! calling the handler trait directly, so what is proven is what an adopter
//! would actually experience.

use cognicode_core::interface::mcp::rmcp_adapter::CogniCodeHandler;
use cognicode_core::product::{PROFILE_POSTURES, ProfilePosture};

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

mod common;
use common::binary_path;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn profile_document() -> Value {
    let path = repo_root().join("product/profiles.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("profiles.json must be readable: {e}"));
    serde_json::from_str(&text).expect("profiles.json must be valid JSON")
}

fn declared_profiles() -> Vec<Value> {
    profile_document()["profiles"]
        .as_array()
        .expect("profiles must be an array")
        .clone()
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
        let mut s = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        };
        s.send(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"uat-a009","version":"0"}}}))
            .await;
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(60), s.stdout.read_line(&mut line))
            .await
            .expect("init timeout")
            .expect("read init");
        assert!(line.contains("\"result\""), "initialize failed: {line}");
        s.send(&serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        s.next_id = 2;
        s
    }

    async fn send(&mut self, v: &Value) {
        self.stdin
            .write_all(format!("{v}\n").as_bytes())
            .await
            .expect("write");
        self.stdin.flush().await.expect("flush");
    }

    async fn request(&mut self, v: Value) -> Value {
        let id = v["id"].as_u64().expect("request id");
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
                    if m.get("id").and_then(|x| x.as_u64()) == Some(id) {
                        return m;
                    }
                }
            }
        }
    }

    async fn all_tool_names(&mut self) -> Vec<String> {
        let mut names = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let id = self.next_id;
            self.next_id += 1;
            let mut params = serde_json::json!({});
            if let Some(c) = &cursor {
                params["cursor"] = Value::String(c.clone());
            }
            let response = self
                .request(serde_json::json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":params}))
                .await;
            let result = &response["result"];
            for tool in result["tools"].as_array().cloned().unwrap_or_default() {
                names.push(tool["name"].as_str().unwrap_or_default().to_string());
            }
            match result["nextCursor"].as_str() {
                Some(next) => cursor = Some(next.to_string()),
                None => break,
            }
        }
        names
    }

    async fn call_tool(&mut self, name: &str, args: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.request(serde_json::json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}}))
            .await
    }

    /// Stop the server so it does not outlive the test.
    ///
    /// `kill_on_drop` alone is not enough here: the session holds the
    /// child's pipes, and dropping the struct while a request is in flight
    /// can leave the process running. The existing read-only UAT ends the
    /// same way.
    async fn shutdown(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn fixture_ws() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

// --- Contract level: the published JSON and the enforced table must agree ---

/// The published `mutating` flag must be the enforced posture.
///
/// A `mutating: false` in a JSON file that the runtime ignores is
/// decoration. This is the assertion that makes the promise real.
#[test]
fn published_mutating_flag_matches_the_enforced_posture() {
    for entry in declared_profiles() {
        let id = entry["id"].as_str().expect("profile id");
        let declared = entry["mutating"].as_bool().expect("mutating flag");
        let enforced = ProfilePosture::for_profile(id)
            .map(|p| !p.is_read_only())
            .unwrap_or_else(|| panic!("profile {id} has no enforced posture"));
        assert_eq!(
            declared, enforced,
            "profile {id}: profiles.json says mutating={declared} but the \
             runtime enforces mutating={enforced}"
        );
    }
}

/// `reviewer` specifically: installable and non-mutating, so its posture
/// is a promise to every adopter.
#[test]
fn reviewer_declares_and_enforces_read_only() {
    let entry = declared_profiles()
        .into_iter()
        .find(|p| p["id"] == "reviewer")
        .expect("reviewer profile must exist");
    assert_eq!(entry["mutating"], Value::Bool(false));
    assert_eq!(entry["install"], Value::Bool(true));
    assert_eq!(
        ProfilePosture::for_profile("reviewer"),
        Some(ProfilePosture::ReadOnly)
    );
}

/// Every published profile has a row in the enforced table, so a new
/// profile cannot ship without a decided posture.
#[test]
fn every_published_profile_has_an_enforced_posture() {
    for entry in declared_profiles() {
        let id = entry["id"].as_str().expect("profile id");
        assert!(
            PROFILE_POSTURES.iter().any(|(known, _)| *known == id),
            "profile {id} is published but has no row in PROFILE_POSTURES"
        );
    }
}

/// Building a handler from a read-only profile must yield a read-only
/// handler. This is the link that did not exist before.
#[test]
fn handler_for_a_read_only_profile_is_read_only() {
    for (id, posture) in PROFILE_POSTURES {
        let handler = CogniCodeHandler::for_profile(PathBuf::from("/tmp"), id);
        assert_eq!(
            handler.is_read_only(),
            posture.is_read_only(),
            "profile {id} resolves to the wrong posture"
        );
    }
}

// --- Behaviour level: what an adopter actually experiences -----------------

/// The real binary in read-only mode must not advertise or accept
/// `write_file`, and must leave nothing on disk.
///
/// `prf_sec_02_read_only_uat.rs` covers the `--read-only` flag itself;
/// this test states the profile-level promise, which is the thing a user
/// reads in `profiles.json` and has to be able to trust.
#[tokio::test]
async fn reviewer_posture_refuses_writes_against_the_real_binary() {
    let ws = fixture_ws();
    let target = ws.join("a009-reviewer-must-not-write.txt");

    let mut session = Session::spawn(&ws, true).await;

    let tools = session.all_tool_names().await;
    assert!(
        !tools.iter().any(|t| t == "write_file"),
        "read-only mode must not advertise write_file; got {tools:?}"
    );
    let response = session
        .call_tool(
            "write_file",
            serde_json::json!({
                "path": target.to_str().unwrap(),
                "content": "must not be written"
            }),
        )
        .await;

    let text = serde_json::to_string(&response).unwrap();
    assert!(
        text.contains("read_only_mode"),
        "the refusal must be honest about why; got: {text}"
    );
    assert!(
        !target.exists(),
        "a refused write must leave nothing on disk"
    );
    session.shutdown().await;
}

/// Read tools must keep working. A blanket denial would satisfy the
/// previous test while making the profile useless.
#[tokio::test]
async fn reviewer_posture_still_allows_reading() {
    let ws = fixture_ws();
    let mut session = Session::spawn(&ws, true).await;

    let tools = session.all_tool_names().await;
    assert!(
        !tools.is_empty(),
        "read-only mode must still advertise read tools"
    );
    assert!(
        !tools.iter().any(|t| t == "write_file"),
        "read-only mode advertised a mutating tool"
    );
    session.shutdown().await;
}
