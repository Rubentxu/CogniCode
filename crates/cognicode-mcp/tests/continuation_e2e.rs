//! H4.4 — MCP Continuation Chain Real Server Verification.
//!
//! End-to-end integration test that drives the **real** `cognicode-mcp`
//! binary over its stdio JSON-RPC transport, follows the `read_file`
//! continuation chain to its natural end, and asserts that the accumulated
//! SHA-256 matches the file on disk byte-exact.
//!
//! This harness deliberately does NOT use the sandbox orchestrator, HTTP,
//! or the reconstructor's disk-mode fallback. The whole point is to test
//! the server's chain behaviour directly, page by page.
//!
//! Spec: `openspec/specs/mcp-continuation-chain-real-server/spec.md`
//! Tasks: `openspec/changes/h44-mcp-continuation-chain-real-server/tasks.md`

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

// MCP protocol version we use for the handshake. Matches what the rest of
// the workspace uses (see crates/cognicode-sandbox/src/main.rs).
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

// Where the binary lives. We resolve relative to CARGO_MANIFEST_DIR so the
// test works whether you run it from the workspace root or from the crate
// directory.
fn binary_path() -> PathBuf {
    // tests live in `crates/cognicode-mcp/tests/`, so CARGO_MANIFEST_DIR is
    // `crates/cognicode-mcp/` and the binary lives at the repo root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .expect("workspace root")
        .parent() // repo root
        .expect("repo root")
        .join("target/release/cognicode-mcp")
}

/// Result of a chain traversal.
#[derive(Debug)]
struct ChainResult {
    /// Pages actually consumed from the MCP server.
    pages: u32,
    /// Concatenated bytes of every page's content.
    accumulated: Vec<u8>,
    /// SHA-256 of the concatenation.
    accumulated_sha: String,
    /// SHA-256 of the file on disk (computed by the test, not the server).
    disk_sha: String,
    /// Per-page latencies in milliseconds.
    per_page_latency_ms: Vec<u64>,
    /// The final response's `has_more` and `next_token`.
    final_has_more: bool,
    final_next_token: Option<String>,
}

#[derive(Debug)]
#[allow(dead_code)] // Some variants are reserved for future adversarial tests
enum ChainError {
    /// First call returned a JSON-RPC error.
    InitialCall(String),
    /// A continuation call returned a JSON-RPC error.
    ContinuationCall { page: u32, message: String },
    /// `has_more=true` but `next_token` is missing.
    MissingNextToken { page: u32 },
    /// A page reported `has_more=false` AND `next_token=Some` — malformed.
    UnwantedNextToken { page: u32 },
    /// Empty `content` field on a non-final page.
    EmptyContent { page: u32 },
    /// Repeated `next_token` (loop).
    RepeatedToken { page: u32, token: String },
    /// Per-page SHA prefix mismatch — the server returned bytes that don't
    /// match the file on disk.
    BoundaryMismatch {
        page: u32,
        accumulated_sha: String,
        expected_sha: String,
        offset: usize,
    },
}

/// Spawn the MCP binary and drive it over stdio JSON-RPC.
struct McpChild {
    child: Child,
    /// Buffered stdin (we own the writer end).
    stdin: tokio::process::ChildStdin,
    /// Buffered stdout line reader.
    stdout: BufReader<tokio::process::ChildStdout>,
    /// Next JSON-RPC id we will send.
    next_id: u64,
}

impl McpChild {
    async fn spawn(workspace: &Path) -> Result<Self, String> {
        let bin = binary_path();
        if !bin.exists() {
            return Err(format!(
                "MCP binary not found at {}. Run `cargo build --release -p cognicode-mcp` first.",
                bin.display()
            ));
        }
        let mut cmd = Command::new(&bin);
        cmd.arg("--cwd")
            .arg(workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("spawn failed: {e}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "no stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "no stdout".to_string())?;
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
                "clientInfo": { "name": "h44-continuation-e2e", "version": "0.0.0" }
            }
        });
        self.next_id += 1;
        self.send_raw(&req).await?;
        let _ = self.read_response(self.next_id - 1).await?;

        // notifications/initialized has no response.
        let notif = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        self.send_raw(&notif).await?;
        Ok(())
    }

    async fn send_raw(&mut self, value: &Value) -> Result<(), String> {
        let line = serde_json::to_string(value).map_err(|e| e.to_string())?;
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        self.stdin.write_all(b"\n").await.map_err(|e| e.to_string())?;
        self.stdin.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Read the next JSON object whose top-level `id` matches `expected_id`.
    /// Skips notifications and server-initiated requests (which have no id).
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
                Some(_) => continue, // different id; skip
                None => continue,    // notification; skip
            }
        }
    }

    async fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        });
        self.send_raw(&req).await?;
        let response = self.read_response(id).await?;
        if let Some(err) = response.get("error") {
            return Err(format!("JSON-RPC error: {err}"));
        }
        response
            .get("result")
            .cloned()
            .ok_or_else(|| format!("no result in response: {response}"))
    }

    async fn shutdown(&mut self) {
        let _ = self.child.kill().await;
    }
}

impl Drop for McpChild {
    fn drop(&mut self) {
        // Best-effort: kill the child if shutdown wasn't called.
        let _ = self.child.start_kill();
    }
}

/// Decode the inner JSON text from a `read_file` response.
///
/// The MCP `read_file` response shape is:
/// ```json
/// { "content": [ { "type": "text", "text": "<JSON string>" } ] }
/// ```
/// where `<JSON string>` is itself a JSON object with `content`,
/// `truncated`, `has_more`, `next_token`, etc. We extract both layers.
fn decode_page_text(result: &Value) -> Result<(String, bool, Option<String>), String> {
    let content_arr = result
        .get("content")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "no content[] in result".to_string())?;
    let first = content_arr
        .first()
        .ok_or_else(|| "empty content[]".to_string())?;
    let text = first
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "no text in content[0]".to_string())?;
    let inner: Value = serde_json::from_str(text)
        .map_err(|e| format!("inner text is not JSON: {e}"))?;
    let has_more = inner
        .get("has_more")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let next_token = inner
        .get("next_token")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let content_str = inner
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "no inner content string".to_string())?
        .to_string();
    Ok((content_str, has_more, next_token))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in out {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Follow the continuation chain starting from the initial response.
/// Returns `ChainResult` on success or `ChainError` on any failure.
async fn follow_chain(
    client: &mut McpChild,
    initial: Value,
    disk_path: &Path,
    rel_path: &str,
) -> Result<ChainResult, ChainError> {
    let disk_bytes = std::fs::read(disk_path)
        .map_err(|e| ChainError::InitialCall(format!("read disk: {e}")))?;
    let disk_sha = sha256_hex(&disk_bytes);

    let (mut page_content, mut has_more, mut next_token) =
        decode_page_text(&initial).map_err(ChainError::InitialCall)?;

    let mut accumulated = Vec::new();
    let mut pages: u32 = 0;
    let mut per_page_latency_ms = Vec::new();
    let mut seen_tokens: std::collections::HashSet<String> = std::collections::HashSet::new();
    let (final_has_more, final_next_token);

    loop {
        pages += 1;
        let page_start = Instant::now();

        // Per-page invariant check: SHA of accumulated bytes vs SHA of
        // disk prefix of the same length.
        accumulated.extend_from_slice(page_content.as_bytes());
        let offset = accumulated.len();
        if offset <= disk_bytes.len() {
            let accumulated_sha = sha256_hex(&accumulated);
            let expected_sha = sha256_hex(&disk_bytes[..offset]);
            if accumulated_sha != expected_sha {
                return Err(ChainError::BoundaryMismatch {
                    page: pages,
                    accumulated_sha,
                    expected_sha,
                    offset,
                });
            }
        } else {
            return Err(ChainError::BoundaryMismatch {
                page: pages,
                accumulated_sha: sha256_hex(&accumulated),
                expected_sha: sha256_hex(&disk_bytes[..disk_bytes.len()]),
                offset,
            });
        }
        per_page_latency_ms.push(page_start.elapsed().as_millis() as u64);

        if !has_more {
            final_has_more = false;
            final_next_token = next_token;
            break;
        }

        // Need a continuation token.
        let token = match next_token.take() {
            Some(t) if !t.is_empty() => t,
            _ => return Err(ChainError::MissingNextToken { page: pages }),
        };
        if !seen_tokens.insert(token.clone()) {
            return Err(ChainError::RepeatedToken {
                page: pages,
                token,
            });
        }

        let response = client
            .call_tool(
                "read_file",
                json!({
                    "path": rel_path,
                    "continuation_token": token,
                }),
            )
            .await
            .map_err(|e| ChainError::ContinuationCall {
                page: pages + 1,
                message: e,
            })?;

        let (next_page_content, next_has_more, next_token_v) = {
            let raw_text = response
                .get("content")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|c| c.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if std::env::var("H44_DEBUG").is_ok() {
                eprintln!(
                    "[h44-debug] page {} response: raw_text={:?}",
                    pages + 1,
                    &raw_text[..raw_text.len().min(200)]
                );
            }
            decode_page_text(&response).map_err(|e| ChainError::ContinuationCall {
                page: pages + 1,
                message: e,
            })?
        };
        if next_page_content.is_empty() {
            return Err(ChainError::EmptyContent { page: pages + 1 });
        }
        page_content = next_page_content;
        has_more = next_has_more;
        next_token = next_token_v;
    }

    let accumulated_sha = sha256_hex(&accumulated);
    Ok(ChainResult {
        pages,
        accumulated,
        accumulated_sha,
        disk_sha,
        per_page_latency_ms,
        final_has_more,
        final_next_token,
    })
}

// ----- Workspace roots for the five Tier-1 read_source scenarios -----
//
// These mirror the paths the sandbox orchestrator uses to clone each repo
// at scenario setup time. Kept here so the harness can be run without
// going through the orchestrator.

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .expect("workspace root")
        .parent() // repo root
        .expect("repo root")
        .to_path_buf()
}

fn scenario_paths(name: &str) -> Option<(PathBuf, &'static str, &'static str)> {
    let root = workspace_root().join("sandbox/repos");
    let paths: &[(&str, &str, &str)] = &[
        (
            "tier1_serde_read_source",
            "serde/serde",
            "src/lib.rs",
        ),
        (
            "tier1_ripgrep_read_source",
            "ripgrep/crates/cli",
            "src/lib.rs",
        ),
        (
            "tier1_anyhow_read_source",
            "anyhow",
            "src/lib.rs",
        ),
        (
            "tier1_tokio_read_source",
            "tokio/tokio",
            "src/lib.rs",
        ),
        (
            "tier1_clap_read_source",
            "clap/clap_builder",
            "src/lib.rs",
        ),
    ];
    for (n, ws, rel) in paths {
        if *n == name {
            return Some((root.join(ws), ws, *rel));
        }
    }
    None
}

/// Helper for tests: spawn a child rooted at the right workspace, follow
/// the chain for the given scenario, and assert byte-equality.
async fn run_scenario(name: &str) -> Result<ChainResult, Box<dyn std::error::Error>> {
    let (workspace, _ws_label, rel_path) =
        scenario_paths(name).ok_or_else(|| format!("unknown scenario: {name}"))?;
    let disk_path = workspace.join(rel_path);
    if !disk_path.exists() {
        return Err(format!("disk file missing: {}", disk_path.display()).into());
    }

    let mut client = McpChild::spawn(&workspace).await?;
    let initial = client
        .call_tool("read_file", json!({ "path": rel_path, "mode": "raw" }))
        .await?;
    let result = follow_chain(&mut client, initial, &disk_path, rel_path)
        .await
        .map_err(|e| format!("chain error in {name}: {e:?}"))?;
    client.shutdown().await;
    Ok(result)
}

// =============================================================================
// Tests
// =============================================================================
//
// All five tests are gated on the binary existing (`binary_path().exists()`)
// so that `cargo test` in a fresh checkout does not fail spuriously — only
// run with `--ignored` after a release build.

fn require_binary() -> Option<&'static str> {
    let bin = binary_path();
    if !bin.exists() {
        eprintln!(
            "[skip] MCP binary not found at {}. Build with `cargo build --release -p cognicode-mcp`.",
            bin.display()
        );
        None
    } else {
        Some("ok")
    }
}

/// anyhow: 730-line file, MUST paginate.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn h44_anyhow_read_source_chain_is_byte_exact() {
    if require_binary().is_none() {
        return;
    }
    let r = run_scenario("tier1_anyhow_read_source")
        .await
        .expect("anyhow chain failed");
    assert!(
        r.pages >= 2,
        "anyhow (730 lines) MUST paginate; got {} pages",
        r.pages
    );
    assert!(r.final_has_more == false, "final page must end the chain");
    assert!(
        r.accumulated_sha == r.disk_sha,
        "anyhow SHA mismatch: accumulated={} disk={}",
        r.accumulated_sha,
        r.disk_sha
    );
    eprintln!(
        "[h44] anyhow: {} pages, {} bytes, sha={}, latencies_ms={:?}",
        r.pages, r.accumulated.len(), r.accumulated_sha, r.per_page_latency_ms
    );
}

/// tokio: 709-line file, MUST paginate.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn h44_tokio_read_source_chain_is_byte_exact() {
    if require_binary().is_none() {
        return;
    }
    let r = run_scenario("tier1_tokio_read_source")
        .await
        .expect("tokio chain failed");
    assert!(
        r.pages >= 2,
        "tokio (709 lines) MUST paginate; got {} pages",
        r.pages
    );
    assert!(r.final_has_more == false, "final page must end the chain");
    assert!(
        r.accumulated_sha == r.disk_sha,
        "tokio SHA mismatch: accumulated={} disk={}",
        r.accumulated_sha,
        r.disk_sha
    );
    eprintln!(
        "[h44] tokio: {} pages, {} bytes, sha={}, latencies_ms={:?}",
        r.pages, r.accumulated.len(), r.accumulated_sha, r.per_page_latency_ms
    );
}

/// serde: 334 lines, MUST be single-page.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn h44_serde_read_source_is_single_page() {
    if require_binary().is_none() {
        return;
    }
    let r = run_scenario("tier1_serde_read_source")
        .await
        .expect("serde chain failed");
    assert_eq!(r.pages, 1, "serde (334 lines) MUST be single-page");
    assert!(r.final_has_more == false);
    assert!(r.final_next_token.is_none());
    assert_eq!(r.accumulated_sha, r.disk_sha);
}

/// ripgrep cli: 295 lines, MUST be single-page.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn h44_ripgrep_read_source_is_single_page() {
    if require_binary().is_none() {
        return;
    }
    let r = run_scenario("tier1_ripgrep_read_source")
        .await
        .expect("ripgrep chain failed");
    assert_eq!(r.pages, 1, "ripgrep (295 lines) MUST be single-page");
    assert_eq!(r.accumulated_sha, r.disk_sha);
}

/// clap_builder: 53 lines, MUST be single-page.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn h44_clap_read_source_is_single_page() {
    if require_binary().is_none() {
        return;
    }
    let r = run_scenario("tier1_clap_read_source")
        .await
        .expect("clap chain failed");
    assert_eq!(r.pages, 1, "clap (53 lines) MUST be single-page");
    assert_eq!(r.accumulated_sha, r.disk_sha);
}
