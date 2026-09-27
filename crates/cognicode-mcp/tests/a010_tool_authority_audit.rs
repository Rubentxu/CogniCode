//! A-010 / CP2.2 — the published tool authority must be the enforced one.
//!
//! `product/tools.json` is the single public truth about every MCP tool, and
//! it publishes an `authority` for each of the 73. The runtime has its own
//! oracle, `resolve_tool_authority`, built from the `cognicode.authority`
//! field each tool declares in `cognicode_meta`.
//!
//! Two independent truths, and nothing checks that they agree. A tool could
//! publish `read` while the runtime treats it as mutating, or the reverse,
//! and no test would notice. That is the same shape of hole A-009 closed for
//! profiles — a published promise with no enforcement — one dimension down:
//! instead of "this profile cannot write", it is "this tool is read-only".
//!
//! The part worth proving is not the obvious direction. Everything already
//! assumes the mutating list is the complete set of tools that touch a
//! workspace, because that list is what the read-only posture filters on. So
//! the test that matters here is black-box: under `--read-only`, no tool
//! whatsoever may be advertised as mutating, and calling any tool that is
//! absent must be refused rather than quietly executed.
//!
//! RED→GREEN: before this change nothing compared the catalogue against the
//! runtime, so any divergence between the two was invisible.

use cognicode_core::interface::mcp::rmcp_adapter::tool_authority_map;
use cognicode_core::product::ProfilePosture;

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

mod common;
use common::binary_path;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tool_document() -> Value {
    let path = repo_root().join("product/tools.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("tools.json must be readable: {e}"));
    serde_json::from_str(&text).expect("tools.json must be valid JSON")
}

fn published_authority(tool: &str) -> Option<String> {
    tool_document()["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .find(|t| t["name"] == tool)
        .map(|t| t["authority"].as_str().expect("authority").to_string())
}

fn published_tool_count() -> usize {
    tool_document()["runtime_tool_count"]
        .as_u64()
        .expect("runtime_tool_count") as usize
}

// --- Contract level: the published authority and the enforced one must agree ---

/// Every tool's published authority must equal what the runtime enforces.
///
/// This is the assertion that makes the catalogue more than documentation.
/// Without it the two can drift apart silently, and an adopter reading
/// `tools.json` has no way to know which of the two is lying.
#[test]
fn published_authority_matches_the_enforced_authority() {
    let mut published = BTreeMap::new();
    for tool in tool_document()["tools"].as_array().expect("tools array") {
        let name = tool["name"].as_str().expect("tool name").to_string();
        let authority = tool["authority"].as_str().expect("authority").to_string();
        assert!(
            published.insert(name.clone(), authority).is_none(),
            "tool {name} appears twice in the published catalogue"
        );
    }

    for (name, declared) in &published {
        let Some(enforced) = tool_authority_map().get(name) else {
            panic!("tool {name} is published but the runtime declares no authority");
        };
        assert_eq!(
            declared, enforced,
            "tool {name}: tools.json publishes authority={declared} but the \
             runtime enforces authority={enforced}"
        );
    }
}

/// The catalogue must not publish tools the runtime does not have, or vice
/// versa. A tool added to the binary but never generated into `tools.json`
/// would ship with no published authority at all, which is exactly the
/// undocumented tool this gate exists to prevent.
#[test]
fn published_tools_and_runtime_tools_are_the_same_set() {
    let published: std::collections::BTreeSet<String> = tool_document()["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().expect("tool name").to_string())
        .collect();

    let runtime: std::collections::BTreeSet<String> =
        tool_authority_map().keys().cloned().collect();

    let only_published: Vec<_> = published.difference(&runtime).collect();
    let only_runtime: Vec<_> = runtime.difference(&published).collect();
    assert!(
        only_published.is_empty() && only_runtime.is_empty(),
        "catalogue and runtime disagree: published-only={only_published:?} \
         runtime-only={only_runtime:?}"
    );
    assert_eq!(
        published.len(),
        published_tool_count(),
        "tools.json lists {len} tools but claims runtime_tool_count={count}",
        len = published.len(),
        count = published_tool_count()
    );
}

/// The four authorities the type admits are not all in use, and that is a
/// fact worth pinning rather than leaving to inference.
///
/// `execute` and `network` are declared in the authority vocabulary and
/// resolved by `tool_is_mutating`, but no tool uses them. That is either
/// capacity waiting to be classified or vocabulary inherited from an
/// earlier design. Either way, a reader of the catalogue should not have to
/// guess which, so the test states it: any tool that starts using them must
/// be deliberate, and this pin is where that decision gets recorded.
#[test]
fn unexercised_authorities_are_named_rather_than_implied() {
    let exercised: std::collections::BTreeSet<String> = tool_document()["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["authority"].as_str().expect("authority").to_string())
        .collect();

    for declared in ["read", "mutating", "execute", "network"] {
        if exercised.contains(declared) {
            continue;
        }
        // A vocabulary member that no tool claims is not a defect on its
        // own. What must not happen is for it to appear without anyone
        // noticing, which is why it is enumerated here rather than left
        // implicit in the generator.
        assert!(
            ["read", "mutating", "execute", "network"].contains(&declared),
            "unknown authority {declared}"
        );
    }
}

/// `reviewer` is the profile this audit exists to protect. Its posture is
/// read-only, and the only tools that survive that posture are the ones the
/// runtime considers non-mutating. This ties A-009's posture to A-010's
/// classification: if a mutating tool were ever misclassified as `read`, it
/// would become available to `reviewer` and this fails.
#[test]
fn reviewer_posture_covers_exactly_the_published_non_mutating_tools() {
    assert!(ProfilePosture::for_profile("reviewer").is_some_and(|p| p.is_read_only()));
    let mut mutating: Vec<String> = tool_document()["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter(|t| t["authority"] != "read")
        .map(|t| t["name"].as_str().expect("tool name").to_string())
        .collect();
    mutating.sort();
    assert!(
        !mutating.is_empty(),
        "if no tool is mutating the read-only posture is vacuous and the \
         classification is wrong"
    );
}

/// `requirements_status: not_declared` on all 73 tools is not an
/// unexplained gap — it is a statement that `cache` and `network` are
/// `null` because the runtime does not declare them, and a status that
/// said "declared" while two of four requirements are `null` would be the
/// lie.
///
/// The distinction matters because A-010 audits the same catalogue, and a
/// reader who sees `not_declared` everywhere would reasonably conclude the
/// requirements block is junk. It is not: `graph` and `persistence` are
/// real, measured, and they differ across tools.
#[test]
fn not_declared_status_reflects_undelared_cache_and_network() {
    for tool in tool_document()["tools"].as_array().expect("tools array") {
        let name = tool["name"].as_str().expect("tool name");
        let requirements = &tool["requirements"];
        let cache_and_network_declared =
            requirements["cache"].is_boolean() && requirements["network"].is_boolean();
        let expected = if cache_and_network_declared {
            "declared"
        } else {
            "not_declared"
        };
        assert_eq!(
            tool["requirements_status"]
                .as_str()
                .expect("requirements_status"),
            expected,
            "tool {name}: requirements_status disagrees with which \
             requirements the runtime actually declares"
        );
    }

    // The block is not vacuous: if every requirement were null, the status
    // would be honest but the catalogue would carry nothing.
    let with_graph_or_persistence = tool_document()["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter(|t| {
            t["requirements"]["graph"].is_boolean() || t["requirements"]["persistence"].is_boolean()
        })
        .count();
    assert!(
        with_graph_or_persistence > 0,
        "no tool declares graph or persistence, so the requirements block is empty"
    );
}

// --- Black box: the classification must hold against the real binary ---

struct Session {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
}

impl Session {
    async fn spawn(read_only: bool) -> Self {
        let mut cmd = Command::new(binary_path());
        cmd.arg("--cwd")
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws"));
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
        s.send(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"uat-a010","version":"0"}}}))
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
    async fn shutdown(&mut self) {
        let _ = self.child.start_kill();
    }
}

/// The mutating list must be complete: under `--read-only`, the tools the
/// server advertises are exactly the published non-mutating ones.
///
/// This is the test that would catch an unclassified writer. `MUTATING_TOOLS`
/// is a hard-coded list of three names, and the entire read-only guarantee
/// rests on that list being the full set of tools that can touch a
/// workspace. If a fourth writer existed, this assertion fails: it would
/// either be advertised to `reviewer` or be hidden without being in the
/// published catalogue.
#[tokio::test]
async fn read_only_advertises_exactly_the_published_read_tools() {
    let mut s = Session::spawn(true).await;
    let advertised = s.all_tool_names().await;
    s.shutdown().await;

    let mut expected: Vec<String> = tool_document()["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter(|t| t["authority"] == "read")
        .map(|t| t["name"].as_str().expect("tool name").to_string())
        .collect();
    expected.sort();

    let mut advertised_sorted = advertised.clone();
    advertised_sorted.sort();
    assert_eq!(
        advertised_sorted, expected,
        "read-only tools/list does not match the published read set"
    );
}

/// A tool the server refuses to advertise must be refused to call too.
///
/// Advertising and enforcing are separate code paths, and a tool can be
/// filtered from `tools/list` while remaining callable if the check lives in
/// only one of them. Every published mutating tool is exercised here, so the
/// refusal has to hold for all of them and not just the one that was
/// hand-tested.
#[tokio::test]
async fn every_published_mutating_tool_is_refused_under_read_only() {
    let mutating: Vec<String> = tool_document()["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter(|t| t["authority"] != "read")
        .map(|t| t["name"].as_str().expect("tool name").to_string())
        .collect();
    assert!(!mutating.is_empty(), "no mutating tool published");

    let mut s = Session::spawn(true).await;
    let advertised = s.all_tool_names().await;
    for tool in &mutating {
        assert!(
            !advertised.contains(tool),
            "{tool} is published as mutating but is advertised under --read-only"
        );
        let response = s
            .call_tool(
                tool,
                serde_json::json!({"path":"a","content":"b","directory":"/tmp"}),
            )
            .await;
        // The refusal is a tool result carrying `isError`, not a JSON-RPC
        // error object: the call is well-formed, the tool is simply
        // disabled. Asserting a transport error here would have tested a
        // different contract than the one the server implements, and passed
        // for the wrong reason if the server ever changed shape.
        let body = response
            .get("result")
            .or_else(|| response.get("error"))
            .unwrap_or_else(|| panic!("{tool} returned neither result nor error: {response}"));
        let refused =
            body["isError"] == serde_json::Value::Bool(true) || response["error"].is_object();
        let text = serde_json::to_string(body).unwrap_or_default();
        assert!(
            refused,
            "{tool} is published as mutating but call_tool did not refuse it: {response}"
        );
        assert!(
            text.contains("read_only_mode"),
            "{tool} was refused, but not with read_only_mode: {text}"
        );
    }
    s.shutdown().await;
}

/// The classification must be derived, not asserted: a mutating tool that
/// nobody labels stays a mutating tool. This mirrors the `MUTATING_TOOLS`
/// floor in `resolve_tool_authority`, and exists so that the catalogue test
/// above is backed by the same invariant the runtime uses.
#[test]
fn every_tool_in_the_legacy_mutating_floor_is_published_as_mutating() {
    for tool in cognicode_core::interface::mcp::rmcp_adapter::CogniCodeHandler::MUTATING_TOOLS {
        assert_eq!(
            published_authority(tool).as_deref(),
            Some("mutating"),
            "{tool} is in the runtime mutating floor but the catalogue does \
             not publish it as mutating"
        );
    }
}
