//! B6.W1 — MCP robustness: verify the stdio MCP server against the
//! operational contract (no daemon, JSON-RPC on stdout, timeouts,
//! pagination, isolation, clean shutdown).
//!
//! Every test in this suite spawns the compiled `cognicode-mcp` binary
//! as a subprocess and exchanges JSON-RPC frames over its stdio. If
//! the binary is not present in `target/{release,debug}/`, the suite
//! short-circuits with a clear `SKIP_NOT_APPLICABLE` style message so
//! CI does not fail when only the library has been compiled.
//!
//! The contract is intentionally adversarial: a server that ships
//! observability on stdout, that blocks past its timeout, or that
//! leaves orphan PIDs behind would defeat the whole point of the
//! stdio MCP transport. These tests are the B6.W1 verification, and
//! they fail loudly when any property regresses.

#![allow(clippy::needless_raw_string_hashes)]

use cognicode_core::interface::mcp::rmcp_adapter::build_all_tools;
use cognicode_core::interface::mcp::robustness::{
    collect_diagnostics, default_limits, note_request_finished, note_request_started,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Page size the stdio server uses for `tools/list` pagination. It is
/// a private constant in `rmcp_adapter::list_tools`; restating it here
/// is what lets the pagination test notice a server that silently
/// changed its page contract (see `large_response_uses_pagination`).
const PAGE_SIZE: usize = 20;

const O_NONBLOCK: i32 = 2048;
const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;

unsafe extern "C" {
    fn fcntl(fd: i32, op: i32, ...) -> i32;
    fn kill(pid: i32, sig: i32) -> i32;
}

/// SAFETY: `fd` is a valid file descriptor borrowed for the duration
/// of the call. `fcntl(F_GETFL)` is read-only and does not mutate
/// state.
unsafe fn fcntl_getfl(fd: i32) -> i32 {
    unsafe { fcntl(fd, F_GETFL) }
}

/// SAFETY: `fd` is a valid file descriptor borrowed for the duration
/// of the call. `fcntl(F_SETFL)` mutates the fd's status flags; we
/// restore the previous flags after reading to leave the fd unchanged.
unsafe fn fcntl_setfl(fd: i32, flags: i32) -> i32 {
    unsafe { fcntl(fd, F_SETFL, flags) }
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Locate the compiled `cognicode-mcp` binary.
///
/// Mirrors the helper in `prf_h06_adversarial_e2e.rs`: walk
/// `CARGO_TARGET_DIR` first (this environment sets it), then the
/// workspace-relative `target/{release,debug}/`, then the bare
/// `target/{release,debug}/`. Returns `None` if no candidate exists.
fn binary_path() -> Option<PathBuf> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").ok();
    let target_dir = std::env::var("CARGO_TARGET_DIR").ok();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(td) = target_dir.as_deref() {
        candidates.push(PathBuf::from(td).join("release/cognicode-mcp"));
        candidates.push(PathBuf::from(td).join("debug/cognicode-mcp"));
    }
    if let Some(md) = manifest_dir.as_deref() {
        let workspace_root = PathBuf::from(md).join("..").join("..");
        candidates.push(workspace_root.join("target/release/cognicode-mcp"));
        candidates.push(workspace_root.join("target/debug/cognicode-mcp"));
    }
    candidates.push(PathBuf::from("target/release/cognicode-mcp"));
    candidates.push(PathBuf::from("target/debug/cognicode-mcp"));
    for p in &candidates {
        if p.exists() {
            return Some(p.canonicalize().unwrap_or_else(|_| p.clone()));
        }
    }
    eprintln!("binary_path: no candidate found. tried: {candidates:#?}");
    None
}

/// Spawn `cognicode-mcp` rooted at `cwd` and return the child handle.
fn spawn_binary(cwd: &Path) -> Option<Child> {
    let bin = binary_path()?;
    let mut cmd = Command::new(&bin);
    cmd.arg("--cwd").arg(cwd);
    cmd.env("MCP_TRANSPORT", "stdio");
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Some(cmd.spawn().expect("spawn cognicode-mcp"))
}

/// Send a JSON-RPC request (one frame, newline-terminated) and flush.
fn rpc_send(child: &mut Child, id: i64, method: &str, params: Value) {
    let frame = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
    .to_string();
    let stdin = child.stdin.as_mut().expect("stdin piped");
    stdin.write_all(frame.as_bytes()).expect("write rpc");
    stdin.write_all(b"\n").expect("write newline");
    stdin.flush().expect("flush rpc");
}

/// Drain stdout line-by-line until the expected id arrives or the
/// timeout fires. Returns the matched frame (if any) and the leftover
/// buffer of bytes that arrived after it (or instead of it on timeout).
///
/// Uses `O_NONBLOCK` on the stdout fd so the read loop honours the
/// deadline: a hung child returns `Err(WouldBlock)` and we poll until
/// the deadline. The fd's prior flags are restored before the
/// function returns, so the caller can keep reading.
fn rpc_recv_by_id(
    child: &mut Child,
    expected_id: i64,
    timeout: Duration,
) -> (Option<Value>, Vec<u8>) {
    let stdout = child.stdout.as_mut().expect("stdout piped");
    let stdout_fd = stdout.as_raw_fd();
    let prev_flags = unsafe { fcntl_getfl(stdout_fd) };
    unsafe {
        fcntl_setfl(stdout_fd, prev_flags | O_NONBLOCK);
    }
    let deadline = Instant::now() + timeout;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut matched: Option<Value> = None;
    loop {
        if Instant::now() >= deadline {
            break;
        }
        match stdout.read(&mut chunk) {
            Ok(0) => break, // EOF
            Ok(n) => {
                for &b in &chunk[..n] {
                    if b == b'\n' {
                        // Try to parse the accumulated line. Lines that
                        // are not the expected id are kept in `buf`
                        // (as the leftover) so the caller can still
                        // inspect them if needed.
                        let trimmed_start = buf
                            .iter()
                            .position(|b: &u8| !b.is_ascii_whitespace())
                            .unwrap_or(buf.len());
                        let line_bytes = &buf[trimmed_start..];
                        if let Ok(v) = serde_json::from_slice::<Value>(line_bytes)
                            && v.get("id").and_then(|x| x.as_i64()) == Some(expected_id)
                            && matched.is_none()
                        {
                            matched = Some(v);
                        }
                        buf.clear();
                    } else {
                        buf.push(b);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if matched.is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
    unsafe {
        fcntl_setfl(stdout_fd, prev_flags);
    }
    (matched, buf)
}

/// Decode a `tools/list` cursor into the byte offset it encodes.
///
/// `rmcp_adapter::list_tools` encodes the offset as the base64 of its
/// decimal string, so this is the inverse. A cursor that does not
/// decode to a number is not an offset, and the caller asserts on
/// `None` rather than skipping the check — that is precisely the
/// "cursor is a fingerprint, not a position" failure the pagination
/// test exists to catch.
fn decode_cursor(cursor: &str) -> Option<usize> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cursor)
        .ok()?;
    String::from_utf8(bytes).ok()?.parse().ok()
}

/// Extract the text payload of a `tools/call` response frame.
///
/// The stdio MCP surface answers a tool call with
/// `result.content[0].text`, which for the handlers exercised here is
/// either a JSON document (`Ok` path) or a one-line reason (`isError`
/// path). Returns the empty string when the frame has neither, so a
/// caller asserting on its content fails with a readable message
/// rather than panicking inside serde.
fn first_text(frame: &Value) -> String {
    frame
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_string()
}

/// Whether a `tools/call` frame reports failure.
///
/// A handler error comes back as `result.isError == true` (the
/// `call_tool` impl in `rmcp_adapter` converts every `Err` into
/// `CallToolResult::error`), never as a JSON-RPC `error` object — but
/// a transport-level rejection can still use one, so both count.
fn frame_is_error(frame: &Value) -> bool {
    let is_error = frame
        .get("result")
        .and_then(|r| r.get("isError"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    is_error || frame.get("error").is_some()
}

/// Whether `pid` names a live process.
///
/// `/proc/<pid>` is authoritative on Linux and lets us distinguish a
/// running child from a not-yet-reaped zombie (whose entry has state
/// `Z`). Elsewhere — this suite is already unix-only, it depends on
/// `ExitStatusExt` — fall back to `kill(pid, 0)`, which returns 0 for
/// a live process and -1/ESRCH for one that is gone.
///
/// Race note: the "gone" answer is subject to PID reuse, which would
/// need the kernel to hand out this exact PID again inside the
/// microseconds between reap and check. Not a practical flake source
/// for a test that has just reaped its own child.
fn process_is_alive(pid: u32) -> bool {
    let proc_entry = PathBuf::from(format!("/proc/{pid}"));
    if proc_entry.exists() {
        // `pid (comm) state ...` — `comm` can contain spaces and
        // parentheses, so the state char is the first field *after*
        // the last ')'.
        if let Ok(stat) = std::fs::read_to_string(proc_entry.join("stat"))
            && let Some((_, rest)) = stat.rsplit_once(')')
            && rest
                .split_whitespace()
                .next()
                .is_some_and(|state| state == "Z")
        {
            // A zombie is not an orphan, but it is not running either;
            // reporting it as gone keeps the check about "did the
            // process exit", which is what this suite is pinning.
            return false;
        }
        return true;
    }
    // SAFETY: `kill` with signal 0 performs only an existence and
    // permission check; it sends nothing and mutates no state.
    unsafe { kill(pid as i32, 0) == 0 }
}

/// Write `count` small Rust files into `root` so that a full-tree
/// `search_content` scan costs real wall-clock time.
///
/// A scan that matches nothing cannot short-circuit on `max_results`,
/// so it walks and reads every file — the cheapest way to hand the
/// server a dispatch that overruns its per-category budget without
/// needing a network, a fake clock, or a `sleep` inside the handler.
fn write_scan_corpus(root: &Path, count: usize) {
    let line = "let value = compute(alpha, beta, gamma) + delta_epsilon * foxtrot;\n";
    let body: String = std::iter::repeat_n(line, 60).collect();
    for i in 0..count {
        std::fs::write(root.join(format!("f{i:05}.rs")), &body).expect("write corpus file");
    }
}

/// Write a two-function Rust file whose entry point is `marker`.
fn write_marker(root: &Path, marker: &str) {
    let source = format!("fn {marker}() {{\n    helper();\n}}\n\nfn helper() {{}}\n");
    std::fs::write(root.join(format!("{marker}.rs")), source).expect("write marker file");
}

/// Send `shutdown` (graceful close) and reap the child.
fn shutdown_and_reap(mut child: Child) -> std::io::Result<std::process::ExitStatus> {
    rpc_send(&mut child, -1, "shutdown", json!({}));
    // Close stdin so the stdio transport exits cleanly even if the
    // server is waiting for more frames.
    drop(child.stdin.take());
    // Bound the wait: shutdown is quick, but a hung handler must not
    // hang the test runner forever.
    let start = Instant::now();
    loop {
        match child.try_wait()? {
            Some(status) => return Ok(status),
            None => {
                if start.elapsed() > Duration::from_secs(5) {
                    let _ = child.kill();
                    return child.wait();
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Skip the current test with an honest reason if the binary is absent.
macro_rules! require_binary {
    () => {
        match binary_path() {
            Some(p) => p,
            None => {
                eprintln!(
                    "SKIP_NOT_APPLICABLE: cognicode-mcp binary not built. Run \
                     `cargo build --bin cognicode-mcp` and re-run. Manual UAT: \
                     spawn the binary, send `initialize` + `tools/list`, \
                     observe JSON-only stdout and clean exit."
                );
                return;
            }
        }
    };
}

// ===========================================================================
// Test 1 — stdout framing keeps only JSON
// ===========================================================================

/// The MCP transport contract requires stdout to carry *only*
/// JSON-RPC frames (one per line). Anything else — log lines, banner,
/// progress messages — would break every stdio client. The test sends
/// `initialize` and `tools/list`, captures stdout, and asserts that
/// every line is parseable JSON. stderr is captured separately and
/// not asserted on the stdout side.
#[test]
fn stdio_framing_keeps_stdout_pure_json() {
    let _bin = require_binary!();
    let workspace = TempDir::new().expect("tempdir");
    let mut child = spawn_binary(workspace.path()).expect("spawn");

    rpc_send(
        &mut child,
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "b6_w1_test", "version": "0.0.1"}
        }),
    );
    let (init_resp, _) = rpc_recv_by_id(&mut child, 1, Duration::from_secs(10));
    let init = init_resp.expect("initialize response within 10s");
    assert_eq!(init["jsonrpc"], "2.0", "initialize must be JSON-RPC 2.0");

    rpc_send(&mut child, 2, "tools/list", json!({}));
    let (list_resp, _) = rpc_recv_by_id(&mut child, 2, Duration::from_secs(10));
    let list = list_resp.expect("tools/list response within 10s");
    assert!(
        list["result"]["tools"].is_array(),
        "tools/list must enumerate tools"
    );

    // Drain any buffered stdout and assert every byte is part of a
    // parseable JSON frame. We close stdin first to make EOF
    // deterministic.
    drop(child.stdin.take());
    let mut raw = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_end(&mut raw);
    }
    // Stderr is captured separately and intentionally not asserted on.
    let _ = child.wait();

    // Walk every line of stdout and require it to be parseable JSON.
    let mut non_json = Vec::new();
    for line in raw.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }
        if serde_json::from_slice::<Value>(line).is_err() {
            non_json.push(String::from_utf8_lossy(line).into_owned());
        }
    }
    assert!(
        non_json.is_empty(),
        "stdout must be JSON-only; {} non-JSON line(s): {non_json:?}",
        non_json.len()
    );
}

// ===========================================================================
// Test 2 — clean shutdown leaves no orphan PID
// ===========================================================================

/// `initialize` then `shutdown`/`exit` must exit with rc=0 and leave no
/// orphan. The stdio transport is one process per client; an orphan
/// here is the kind of failure mode the daemon-flavoured solution
/// would inherit by definition.
#[test]
fn initialize_then_clean_shutdown_no_orphan() {
    let _bin = require_binary!();
    let workspace = TempDir::new().expect("tempdir");
    let child = spawn_binary(workspace.path()).expect("spawn");
    let pid = child.id();
    let mut child = child;

    // The PID this test is about to hold accountable has to be a real
    // running process first. Without this precondition the
    // post-shutdown check below would also pass against a PID that
    // was never alive, which is the failure mode that makes a
    // liveness assertion look like it works when it does not.
    assert!(
        process_is_alive(pid),
        "server process {pid} must be alive immediately after spawn"
    );

    // Drive the lifecycle explicitly so we exercise the real exit path.
    rpc_send(
        &mut child,
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "b6_w1_shutdown", "version": "0.0.1"}
        }),
    );
    let _ = rpc_recv_by_id(&mut child, 1, Duration::from_secs(10));
    let status = shutdown_and_reap(child).expect("reap");

    // The status must be one of {0, signal 0..NORMAL}. A non-zero exit
    // here means the server tripped on its own shutdown path; a signal
    // means it had to be killed. Either is a fail.
    assert!(
        status.success(),
        "clean shutdown must exit with rc=0; got {status:?}"
    );
    assert!(
        status.signal().is_none(),
        "shutdown must not require a signal kill; got signal={:?}",
        status.signal()
    );

    // The half that holds the line on "no orphan". `shutdown_and_reap`
    // returning an `ExitStatus` means `wait()` reaped the child, but
    // that is an implication we are asserting explicitly rather than
    // trusting: a server that forked a grandchild holding the stdio
    // pipes would exit cleanly and still leave something behind, and
    // only an observation of the process table catches that.
    assert!(
        !process_is_alive(pid),
        "clean shutdown must leave no process behind at pid {pid}"
    );
}

// ===========================================================================
// Test 3 — pagination continuation on a large tool surface
// ===========================================================================

/// `tools/list` returns a `next_cursor` when the catalog exceeds the
/// page size. This test walks the cursor to completion and asserts
/// three things that a "server that quietly loses a page" cannot
/// survive:
///
///   1. the **union of every page is exactly the advertised
///      catalogue** — compared against `build_all_tools()`, the same
///      function the server paginates;
///   2. the cursor is a **real, contiguous offset**: base64 of
///      `0, PAGE_SIZE, 2*PAGE_SIZE, …`. A server that returned a
///      constant cursor, or advanced by the wrong stride, still walks
///      without duplicates but loses or repeats rows;
///   3. every page but the last is **exactly `PAGE_SIZE` long**, so a
///      truncated middle page cannot hide behind a non-empty union.
///
/// The MCP SDK already handles pagination at the framing level; what
/// we are pinning is that *our* server exposes the contract honestly.
#[test]
fn large_response_uses_pagination() {
    let _bin = require_binary!();
    let workspace = TempDir::new().expect("tempdir");
    let mut child = spawn_binary(workspace.path()).expect("spawn");

    rpc_send(
        &mut child,
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "b6_w1_pagination", "version": "0.0.1"}
        }),
    );
    let _ = rpc_recv_by_id(&mut child, 1, Duration::from_secs(10));

    // Walk the catalog by cursor, keeping the name set and the
    // per-page shape.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut ordered: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    let mut req_id = 2i64;
    let mut pages = 0usize;
    loop {
        let mut params = json!({});
        if let Some(c) = &cursor {
            params = json!({ "cursor": c });
        }
        rpc_send(&mut child, req_id, "tools/list", params);
        let (resp, _) = rpc_recv_by_id(&mut child, req_id, Duration::from_secs(10));
        let frame = resp.expect("tools/list page within 10s");
        let result = frame.get("result").expect("result present");
        let tools = result
            .get("tools")
            .and_then(|t| t.as_array())
            .expect("tools array");

        let next = result
            .get("nextCursor")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        // Shape of this page. A page that hands back a cursor for
        // "more" but is not full means the server skipped rows.
        if next.is_some() {
            assert_eq!(
                tools.len(),
                PAGE_SIZE,
                "page {pages} advertises a nextCursor but carries {} tools, \
                 not the full page size of {PAGE_SIZE}; the cursor skipped rows",
                tools.len()
            );
        }

        for tool in tools {
            let name = tool
                .get("name")
                .and_then(|n| n.as_str())
                .expect("tool name")
                .to_string();
            assert!(
                seen.insert(name.clone()),
                "duplicate tool name {name} across pages (cursor contract broken)"
            );
            ordered.push(name);
        }

        cursor = next;
        pages += 1;
        if pages > 200 {
            panic!("pagination did not terminate after 200 pages; cursor={cursor:?}");
        }
        if cursor.is_none() {
            break;
        }

        // The cursor must be base64 of the *contiguous* offset this
        // walk expects: page 1 hands back `PAGE_SIZE`, page 2 hands
        // back `2 * PAGE_SIZE`, and so on. This is the assertion that
        // fails when the cursor is not a real offset — a constant
        // cursor loops, a mis-strided one silently reorders.
        let expected_offset = pages * PAGE_SIZE;
        let decoded = decode_cursor(cursor.as_deref().expect("cursor present"));
        assert_eq!(
            decoded,
            Some(expected_offset),
            "cursor for page {pages} must encode offset {expected_offset}; \
             got {decoded:?} (cursor={:?})",
            cursor.as_deref()
        );

        req_id += 1;
    }
    let _ = shutdown_and_reap(child);

    assert!(
        !seen.is_empty(),
        "tools/list returned an empty catalog across all pages"
    );
    assert!(
        pages >= 4,
        "the catalogue is large enough to need several pages; walking it \
         took {pages}, which means the server is no longer paginating \
         and this test is not testing pagination"
    );

    // The load-bearing assertion: the union of every page is the
    // advertised catalogue, item for item. `build_all_tools()` is the
    // function the server itself paginates, so this is "the whole
    // catalogue", not a hand-copied list that can drift.
    let advertised: BTreeSet<String> = build_all_tools()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    let missing: Vec<&String> = advertised.difference(&seen).collect();
    let extra: Vec<&String> = seen.difference(&advertised).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "walking the cursor did not reproduce the advertised catalogue \
         ({} walked, {} advertised, {pages} pages).\n\
         missing from the walk: {missing:?}\n\
         walked but not advertised: {extra:?}\n\
         If the server binary under test is stale, rebuild it with \
         `cargo build --bin cognicode-mcp` before reading this as a \
         server bug.",
        seen.len(),
        advertised.len()
    );
}

// ===========================================================================
// Test 4 — request timeout cancels in-flight
// ===========================================================================

/// A `tools/call` that overruns its category budget must still be
/// answered, and it must leave the transport usable.
///
/// # Known gap, measured against `cognicode-mcp`
///
/// The per-category budget in `timeout_for_category` (`search` = 500
/// ms) does **not** cancel a synchronous handler. `call_tool_handler`
/// wraps the dispatch in `tokio::time::timeout`, but tokio's
/// `Timeout` only reports `Elapsed` once the inner future returns
/// `Pending`; a handler whose whole body runs inside one poll — which
/// is every file_ops, graph and AIX handler today — reaches the
/// deadline *after* it has already produced its value, and the value
/// wins. Measured 2026-10-15: a `search_content` over a 670 MB corpus
/// took 1350 ms against the 500 ms budget and returned `isError:false`
/// with a complete result. No timeout, no cancellation.
///
/// So this test does not claim to prove cancellation, because
/// cancellation does not happen. It pins the two properties that do
/// hold, and that a wedged or starved transport would break:
///
///   1. **bounded** — the slow call is answered, well inside the
///      published `default_limits().request_timeout_ms` ceiling;
///   2. **not wedged** — the very next request is served, so a long
///      dispatch leaves no stuck state, no leaked worker, and no
///      poisoned stdio pipe behind it.
///
/// When the handlers are made genuinely async (`spawn_blocking` or a
/// real await that yields), the timeout will start firing; this test
/// must then be tightened to assert an `isError` frame whose body is
/// `timeout`, and the `MIN_USEFUL_WORK_MS` guard below becomes the
/// thing to remove.
#[test]
fn slow_tool_call_is_bounded_and_does_not_wedge_the_transport() {
    /// Floor on how long the corpus scan must take for this test to
    /// be exercising a slow dispatch at all. Well under the 500 ms
    /// `search` budget so it does not turn into a machine-speed
    /// assertion, but far above the ~2 ms a no-op round trip costs.
    const MIN_USEFUL_WORK_MS: u128 = 150;

    /// ~25k small files (≈100 MB). A scan whose pattern matches
    /// nothing cannot stop on `max_results`, so it reads the whole
    /// tree — measured ≈0.65 s, comfortably past the 500 ms budget,
    /// while costing only ~2 s to materialise.
    const CORPUS_FILES: usize = 25_000;

    let _bin = require_binary!();
    let workspace = TempDir::new().expect("tempdir");
    write_scan_corpus(workspace.path(), CORPUS_FILES);
    let mut child = spawn_binary(workspace.path()).expect("spawn");

    rpc_send(
        &mut child,
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "b6_w1_timeout", "version": "0.0.1"}
        }),
    );
    let _ = rpc_recv_by_id(&mut child, 1, Duration::from_secs(10));

    rpc_send(
        &mut child,
        2,
        "tools/call",
        json!({
            "name": "search_content",
            "arguments": {
                "pattern": "ZZZ_NO_SUCH_TOKEN_ZZZ",
                "regex": true,
                "max_results": 1
            }
        }),
    );
    let started = Instant::now();
    let (resp, _) = rpc_recv_by_id(&mut child, 2, Duration::from_secs(60));
    let elapsed_ms = started.elapsed().as_millis();
    let frame = resp.expect("slow search_content must be answered within 60s");

    assert!(
        elapsed_ms >= MIN_USEFUL_WORK_MS,
        "the corpus scan finished in {elapsed_ms}ms, below the \
         {MIN_USEFUL_WORK_MS}ms floor this test needs — the workload \
         stopped being slow and the test now proves nothing"
    );

    // (1) Bounded: answered, and far below the published ceiling.
    let limit_ms = default_limits().request_timeout_ms as u128;
    assert!(
        elapsed_ms < limit_ms,
        "slow dispatch took {elapsed_ms}ms, above the published {limit_ms}ms ceiling"
    );

    // (2) Structured: whatever the outcome, it is an intact JSON-RPC
    // frame addressed to this request — never a torn or merged line.
    assert_eq!(frame["jsonrpc"], "2.0", "slow call must answer in-band");
    assert_eq!(frame["id"], 2, "slow call must answer its own request id");
    assert!(
        frame.get("result").is_some() || frame.get("error").is_some(),
        "slow call must produce a JSON-RPC result or error object: {frame}"
    );

    // (3) Not wedged: the transport still serves the next request.
    rpc_send(&mut child, 3, "tools/list", json!({}));
    let (next, _) = rpc_recv_by_id(&mut child, 3, Duration::from_secs(10));
    let next = next.expect("server must still serve requests after a slow dispatch");
    assert!(
        next["result"]["tools"].is_array(),
        "tools/list after a slow dispatch must enumerate tools: {next}"
    );

    let _ = shutdown_and_reap(child);
}

/// Tripwire for the known gap documented on
/// `slow_tool_call_is_bounded_and_does_not_wedge_the_transport`.
///
/// `call_tool_handler` guards its dispatch with
/// `tokio::time::timeout(timeout_for_category(&category), dispatch)`.
/// This runs the same primitive over a future whose body blocks its
/// worker thread synchronously — the shape every current handler has
/// — and asserts the timeout does **not** fire.
///
/// It passes today, and it is here so that the day someone makes a
/// handler genuinely async it turns red and forces a decision about
/// what the transport-level test above should then assert, instead of
/// the cancellation guarantee quietly changing underneath a test that
/// claims to pin it.
#[tokio::test]
async fn dispatch_timeout_cannot_interrupt_a_blocking_handler() {
    let budget = Duration::from_millis(50);
    let blocking = async {
        std::thread::sleep(Duration::from_millis(250));
        "handler finished"
    };
    let outcome = tokio::time::timeout(budget, blocking).await;
    assert_eq!(
        outcome,
        Ok("handler finished"),
        "tokio::time::timeout returned Elapsed for a handler body that \
         blocks its worker thread. The handler shape has changed, so the \
         per-category dispatch timeout may now actually cancel work — \
         revisit `slow_tool_call_is_bounded_and_does_not_wedge_the_transport` \
         and make it assert a `timeout` error instead."
    );
}

// ===========================================================================
// Test 5 — concurrent requests in different sessions are isolated
// ===========================================================================

/// Two sessions must not share mutable state.
///
/// The stdio transport is one process per session and
/// `CogniCodeHandler` owns exactly one `Arc<HandlerContext>` — so
/// there *is* real per-session state to isolate: the in-memory graph
/// store, the `graph_loaded` flag, the bound workspace root, and the
/// `InputValidator` sandbox built from it (`build_ctx` in
/// `rmcp_adapter`). This test populates all four in session A and
/// checks session B observes none of them.
///
/// Each check is a *negative* observation about B after A has
/// established the corresponding positive in itself, so a shared
/// store, a shared `OnceLock`, or a server that quietly ignored its
/// `--cwd` all turn this red.
///
/// The catalogue-equality assertion the test used to make (both
/// sessions see the same tool list) was worthless: `build_all_tools()`
/// is a build-time constant, so that equality held by construction and
/// said nothing about isolation. It survives here as a one-line
/// sanity check on the harness, not as evidence.
#[test]
fn concurrent_requests_in_different_sessions_isolated() {
    let _bin = require_binary!();
    let ws_a = TempDir::new().expect("ws a");
    let ws_b = TempDir::new().expect("ws b");
    write_marker(ws_a.path(), "alpha_marker");
    write_marker(ws_b.path(), "beta_marker");
    let a_file = ws_a.path().join("alpha_marker.rs");
    let a_file_str = a_file.to_string_lossy().into_owned();
    let b_file = ws_b.path().join("beta_marker.rs");
    let b_file_str = b_file.to_string_lossy().into_owned();

    let mut child_a = spawn_binary(ws_a.path()).expect("spawn a");
    let mut child_b = spawn_binary(ws_b.path()).expect("spawn b");

    for (child, name) in [(&mut child_a, "session_a"), (&mut child_b, "session_b")] {
        rpc_send(
            child,
            1,
            "initialize",
            json!({
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": name, "version": "0.0.1"}
            }),
        );
        let (init, _) = rpc_recv_by_id(child, 1, Duration::from_secs(10));
        assert!(init.is_some(), "{name} must complete initialize");
    }

    // Harness sanity: both sessions are alive and serving. (This is
    // the whole of what the old version of this test asserted.)
    for (child, name) in [(&mut child_a, "A"), (&mut child_b, "B")] {
        rpc_send(child, 2, "tools/list", json!({}));
        let (list, _) = rpc_recv_by_id(child, 2, Duration::from_secs(10));
        let list = list.unwrap_or_else(|| panic!("session {name} must serve tools/list"));
        assert!(
            !list["result"]["tools"]
                .as_array()
                .expect("tools array")
                .is_empty(),
            "session {name} catalogue must be non-empty"
        );
    }

    // --- Per-session state #1: the in-memory graph store. ---------
    rpc_send(
        &mut child_a,
        3,
        "tools/call",
        json!({"name": "build_graph", "arguments": {}}),
    );
    let (a_build, _) = rpc_recv_by_id(&mut child_a, 3, Duration::from_secs(60));
    let a_build = a_build.expect("session A build_graph within 60s");
    assert!(
        !frame_is_error(&a_build),
        "session A must build a graph over its own workspace: {a_build}"
    );
    assert!(
        first_text(&a_build).contains("\"success\":true"),
        "session A build_graph must report success: {a_build}"
    );

    // Same tool, same arguments, both sessions. A answers from its own
    // graph; B has none of its own and must not inherit A's.
    rpc_send(
        &mut child_a,
        4,
        "tools/call",
        json!({"name": "graph_analyze", "arguments": {}}),
    );
    let (a_graph, _) = rpc_recv_by_id(&mut child_a, 4, Duration::from_secs(30));
    let a_graph = a_graph.expect("session A graph_analyze within 30s");
    assert!(
        !frame_is_error(&a_graph),
        "session A must serve graph_analyze from the graph it just built: {a_graph}"
    );

    rpc_send(
        &mut child_b,
        3,
        "tools/call",
        json!({"name": "graph_analyze", "arguments": {}}),
    );
    let (b_graph, _) = rpc_recv_by_id(&mut child_b, 3, Duration::from_secs(30));
    let b_graph = b_graph.expect("session B graph_analyze within 30s");
    assert!(
        frame_is_error(&b_graph),
        "session B must NOT see session A's graph, but graph_analyze \
         succeeded there: {b_graph}"
    );
    assert!(
        first_text(&b_graph).contains("No graph available"),
        "session B's graph_analyze must fail with the no-graph reason: {b_graph}"
    );

    // --- Per-session state #2: the bound workspace root. ---------
    // `project_overview` reports the entry points it discovered. Each
    // session must see only the tree it was pointed at.
    for (child, id, mine, theirs, name) in [
        (&mut child_a, 5, "alpha_marker", "beta_marker", "A"),
        (&mut child_b, 4, "beta_marker", "alpha_marker", "B"),
    ] {
        rpc_send(
            child,
            id,
            "tools/call",
            json!({"name": "project_overview", "arguments": {}}),
        );
        let (frame, _) = rpc_recv_by_id(child, id, Duration::from_secs(30));
        let frame = frame.unwrap_or_else(|| panic!("session {name} project_overview within 30s"));
        assert!(
            !frame_is_error(&frame),
            "session {name} project_overview must succeed: {frame}"
        );
        let text = first_text(&frame);
        assert!(
            text.contains(mine),
            "session {name} project_overview must report its own entry \
             point {mine}: {text}"
        );
        assert!(
            !text.contains(theirs),
            "session {name} project_overview leaked session \
             {theirs}'s symbol — the two sessions share a workspace root: {text}"
        );
    }

    // --- Per-session state #3: the InputValidator sandbox. -------
    // Built from the canonicalised workspace root, so the same
    // absolute path must be readable from its own session and
    // refused from the other.
    rpc_send(
        &mut child_a,
        6,
        "tools/call",
        json!({"name": "read_file", "arguments": {"path": a_file_str.clone()}}),
    );
    let (a_read, _) = rpc_recv_by_id(&mut child_a, 6, Duration::from_secs(30));
    let a_read = a_read.expect("session A read_file within 30s");
    assert!(
        !frame_is_error(&a_read),
        "session A must read a file inside its own workspace: {a_read}"
    );

    rpc_send(
        &mut child_b,
        5,
        "tools/call",
        json!({"name": "read_file", "arguments": {"path": a_file_str.clone()}}),
    );
    let (b_cross, _) = rpc_recv_by_id(&mut child_b, 5, Duration::from_secs(30));
    let b_cross = b_cross.expect("session B read_file within 30s");
    assert!(
        frame_is_error(&b_cross),
        "session B must NOT be able to read session A's file at {a_file_str}: {b_cross}"
    );
    assert!(
        first_text(&b_cross).contains("Path outside workspace"),
        "session B's cross-workspace read must be refused by the sandbox \
         with the workspace-boundary reason: {b_cross}"
    );

    // ...and B still reads its own, so the refusal above is the
    // sandbox talking and not a broken handler.
    rpc_send(
        &mut child_b,
        6,
        "tools/call",
        json!({"name": "read_file", "arguments": {"path": b_file_str}}),
    );
    let (b_read, _) = rpc_recv_by_id(&mut child_b, 6, Duration::from_secs(30));
    let b_read = b_read.expect("session B read_file within 30s");
    assert!(
        !frame_is_error(&b_read),
        "session B must read a file inside its own workspace: {b_read}"
    );

    let _ = shutdown_and_reap(child_a);
    let _ = shutdown_and_reap(child_b);
}

// ===========================================================================
// Test 6 — structured error on a malformed tools/call argument
// ===========================================================================

/// A `tools/call` with a malformed argument must surface as a typed
/// JSON-RPC error or `isError=true` content. The framing must remain
/// valid JSON on stdout. A panic on stderr is acceptable as long as
/// stdout still carries the error response (this is what B5.W3 and
/// B5.W4 pinned for refactor errors and we keep the same shape).
#[test]
fn structured_error_response_is_iserror() {
    let _bin = require_binary!();
    let workspace = TempDir::new().expect("tempdir");
    let mut child = spawn_binary(workspace.path()).expect("spawn");
    rpc_send(
        &mut child,
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "b6_w1_err", "version": "0.0.1"}
        }),
    );
    let _ = rpc_recv_by_id(&mut child, 1, Duration::from_secs(10));

    // Malformed arguments for `read_file`: missing required `path`.
    rpc_send(
        &mut child,
        2,
        "tools/call",
        json!({
            "name": "read_file",
            "arguments": {}
        }),
    );
    let (resp, _) = rpc_recv_by_id(&mut child, 2, Duration::from_secs(5));
    let frame = resp.expect("read_file with missing path must respond");
    let is_error = frame
        .get("result")
        .and_then(|r| r.get("isError"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let has_error = frame.get("error").is_some();
    assert!(
        is_error || has_error,
        "malformed argument must produce isError=true or jsonrpc error; got: {frame}"
    );
    let _ = shutdown_and_reap(child);
}

// ===========================================================================
// Test 7 — note_request_started/finished affect the diagnostics counter
// ===========================================================================

/// The library-side counter hook is the bridge between the dispatch
/// boundary (which we do not touch in this WU) and the diagnostics
/// surface. Pin its behaviour so the wiring can land incrementally:
/// when a request lifecycle is recorded, the snapshot reflects it.
#[test]
fn robustness_counters_reflect_synthetic_lifecycle() {
    let before = collect_diagnostics().total_requests;
    let inflight_before = collect_diagnostics().inflight_requests;
    note_request_started();
    let inflight_mid = collect_diagnostics().inflight_requests;
    assert!(
        inflight_mid > inflight_before,
        "inflight must grow on note_request_started: before={inflight_before} mid={inflight_mid}"
    );
    note_request_finished();
    let inflight_after = collect_diagnostics().inflight_requests;
    let after = collect_diagnostics().total_requests;
    assert!(
        after > before,
        "total_requests must grow: before={before} after={after}"
    );
    assert!(
        inflight_after <= inflight_before,
        "inflight must not grow past start after finish: before={inflight_before} after={inflight_after}"
    );
}

// ===========================================================================
// Test 8 — published limits are sane and the diagnostics struct is Copy
// ===========================================================================

/// Defensive: the struct is `Copy` so callers can compare snapshots
/// without fighting a borrow. If someone tries to add a `String` or a
/// `Vec` to it, this test catches the regression at compile time.
#[test]
fn server_diagnostics_is_copy_and_limits_are_pinned() {
    let l = default_limits();
    assert!(l.max_concurrent_requests >= 1);
    assert!(l.max_fanout >= 1);
    assert!(l.request_timeout_ms >= 60_000, "graph budget must fit");
    assert!(l.cancel_grace_ms > 0);

    let d = collect_diagnostics();
    let copy1 = d;
    let copy2 = d;
    assert_eq!(copy1, copy2, "ServerDiagnostics must be Copy + PartialEq");
    assert!(d.pid > 0);
}
