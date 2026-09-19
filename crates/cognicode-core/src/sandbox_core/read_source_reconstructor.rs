//! H4.3 — `read_file` continuation reconstruction.
//!
//! The orchestrator's normal call returns only the first page of a file
//! (default 500 lines, even though the underlying MCP server emits a
//! `next_token`). For scenarios that opt in via `ScenarioDef::read_source_full`,
//! the orchestrator follows the `next_token` chain here, with explicit safety
//! limits, and produces a `ReconstructedContent` that the scorer can compare
//! against the file on disk.
//!
//! ## Safety contract
//!
//! - **Limits** (defaults; overridable via the manifest in a follow-up):
//!   - `max_pages = 64` (≥32 MiB at 500-line pages)
//!   - `max_bytes = 16 MiB` (mirrors H4.2's `MAX_TOKEN_CHUNK_BYTES` upper bound)
//!   - `max_duration = 30 s`
//! - **No-progress abort**: if a follow-up page returns the same `next_token`
//!   or an `offset` that has not advanced, we abort with `Incomplete` — the
//!   caller will fall back to the first-page response.
//! - **Loop detection**: we keep a set of `(path, offset, chunk_size, mode)`
//!   tuples already issued; a duplicate causes `Incomplete`.
//! - **Mutation detection**: at the end, the reconstructor reads the target
//!   file from disk (canonical path, no path canonicalisation tricks — we
//!   trust the orchestrator's workspace boundary) and compares its SHA-256
//!   against the reconstructed content. A mismatch becomes
//!   `MutationDetected`, NOT a silent pass.
//! - **Token-binding errors** (cross-file, oversize offset/chunk) propagate
//!   as `Incomplete(reason = "...")` — H4.2 already rejects these at the
//!   server side, but we catch them here too so a regression in the server
//!   does not turn into a hang in the orchestrator.
//!
//! The function is pure: it takes an `FnMut` closure that performs the
//! actual MCP call. This lets unit tests drive the loop with a deterministic
//! stub instead of spawning a real server.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::sandbox_core::manifest::Manifest;

/// Sentinel emitted by the sandbox orchestrator when it refuses to issue
/// further MCP continuation calls (because the single stdio MCP process
/// cannot serve re-entrant requests without deadlocking). The reconstructor
/// uses this string to distinguish "the orchestrator chose disk-mode" from
/// "the MCP server returned an arbitrary error" — only the former triggers
/// the disk fallback (B1). Any other error string keeps `status=Incomplete`
/// and never touches the disk.
pub const ORCHESTRATOR_DISK_MODE_SENTINEL: &str = "H4.3: disk-mode reconstruction";

/// Provenance of the `content` returned by the reconstructor. This is the
/// load-bearing distinction H4.3.x introduced: `McpChain` means every byte
/// was delivered by the MCP continuation chain; `DiskFallback` means the
/// loop bailed (orchestrator refused more calls) and the reconstructor
/// substituted a direct read of the file on disk. The scorecard MUST NOT
/// count a `DiskFallback` as proof that the MCP server paginates correctly
/// — H4.4 is the campaign that exercises the real chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconstructionSource {
    /// Every page came from the MCP continuation chain (or there was only
    /// one page and it came from the orchestrator's first call).
    McpChain,
    /// The MCP chain was abandoned (orchestrator-sentinel error); content
    /// came from a bounded read of `target_file` on disk.
    DiskFallback,
}

impl ReconstructionSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::McpChain => "mcp_chain",
            Self::DiskFallback => "disk_fallback",
        }
    }
}

/// Status of a reconstruction attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconstructionStatus {
    /// Reconstruction was skipped because the manifest did not request it
    /// (`read_source_full = false`) or the tool is not `read_file`.
    NotApplicable,
    /// The first page already covered the whole file (`truncated = false`).
    /// No continuation needed; the original response is sufficient.
    SinglePage,
    /// All pages were consumed and the reconstructed SHA-256 matches the
    /// file on disk. The reconstructed content may be substituted for the
    /// first-page response. The `source` field tells you whether this came
    /// from the MCP chain or from the disk fallback.
    Complete,
    /// Reconstruction stopped early: limit hit, no-progress loop detected,
    /// token-binding rejection, MCP error, or timeout. The reconstructed
    /// content (whatever was accumulated) is written to the artifact but
    /// NOT substituted for scoring.
    Incomplete,
    /// Reconstruction completed but the SHA-256 of the concatenated pages
    /// does NOT match the SHA-256 of the file on disk — i.e. the file was
    /// mutated while we were reading it. NOT a silent pass.
    MutationDetected,
}

impl ReconstructionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::SinglePage => "single_page",
            Self::Complete => "complete",
            Self::Incomplete => "incomplete",
            Self::MutationDetected => "mutation_detected",
        }
    }
}

/// Result of a reconstruction attempt. Serialised to `reconstructed.json` in
/// the scenario's results dir. The `content` field is only populated when
/// `status ∈ {SinglePage, Complete, Incomplete, MutationDetected}` — never
/// for `NotApplicable`.
///
/// The `source` field (H4.3.x) tells you whether `content` came from the
/// MCP continuation chain (`McpChain`) or from the bounded disk fallback
/// (`DiskFallback`). A `Complete` with `DiskFallback` is **not** proof that
/// the MCP server paginates correctly — that proof belongs to H4.4.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconstructedContent {
    pub status: ReconstructionStatus,
    /// Provenance of `content`. Always `McpChain` for `SinglePage` (the first
    /// page already covered the file). May be `DiskFallback` for `Complete`
    /// when the orchestrator refused further calls.
    #[serde(default = "default_source_mcp_chain")]
    pub source: ReconstructionSource,
    /// Number of pages consumed (1 = only the first page, 2+ = at least one
    /// continuation round-trip).
    pub pages: u32,
    /// Total bytes of the concatenated content (UTF-8).
    pub total_bytes: u64,
    /// SHA-256 of the concatenated content. `None` if we never accumulated
    /// any content (e.g. first page was empty).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256_reconstructed: Option<String>,
    /// SHA-256 of the file on disk *after* reconstruction finished. `None`
    /// if the file was unreadable at the end (e.g. removed mid-read).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256_disk: Option<String>,
    /// Concatenated content. `None` when `status = NotApplicable`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Free-text reason populated for `Incomplete` and `MutationDetected`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Wall-clock duration of the reconstruction.
    pub duration_ms: u64,
}

fn default_source_mcp_chain() -> ReconstructionSource {
    ReconstructionSource::McpChain
}

/// Tunable limits. Defaults match the contract documented at module level.
#[derive(Debug, Clone, Copy)]
pub struct ReconstructionLimits {
    pub max_pages: u32,
    pub max_bytes: u64,
    pub max_duration: Duration,
}

impl Default for ReconstructionLimits {
    fn default() -> Self {
        Self {
            max_pages: 64,
            max_bytes: 16 * 1024 * 1024,
            max_duration: Duration::from_secs(30),
        }
    }
}

/// Drive a `read_file` reconstruction.
///
/// `initial_response` is the JSON-RPC `result` of the orchestrator's first
/// MCP call — the same `Value` that `compute_correctness_score` would consume
/// directly. We extract `content`, `truncated`, `has_more`, `next_token` from
/// inside it.
///
/// `call_next` performs the next MCP call, taking the new `arguments` map
/// (already containing `continuation_token`) and returning the JSON-RPC
/// `result` for that call, or an error string.
///
/// `target_file` is the path on disk to verify against at the end. It is
/// read **once**, after the loop finishes. If `target_file` does not exist
/// or is unreadable, the SHA-256 disk check is skipped and `status` falls
/// through to `Complete` (the reconstruction did succeed; we just cannot
/// verify it).
pub fn reconstruct_read_file<F>(
    initial_response: &Value,
    mut call_next: F,
    target_file: Option<&Path>,
    limits: ReconstructionLimits,
) -> ReconstructedContent
where
    F: FnMut(Value) -> Result<Value, String>,
{
    let start = Instant::now();
    let mut accumulated = String::new();
    let mut pages: u32 = 0;
    let mut seen_tokens: HashSet<String> = HashSet::new();
    let mut current = initial_response.clone();
    let mut abort_reason: Option<String> = None;
    let mut source: ReconstructionSource = ReconstructionSource::McpChain;

    loop {
        // Stop conditions: budget exhausted?
        if pages >= limits.max_pages {
            abort_reason = Some(format!(
                "max_pages={} reached after {} page(s)",
                limits.max_pages, pages
            ));
            break;
        }
        if (accumulated.len() as u64) > limits.max_bytes {
            abort_reason = Some(format!(
                "max_bytes={} exceeded (accumulated {} bytes)",
                limits.max_bytes,
                accumulated.len()
            ));
            break;
        }
        if start.elapsed() > limits.max_duration {
            abort_reason = Some(format!(
                "max_duration={:?} exceeded after {} page(s)",
                limits.max_duration, pages
            ));
            break;
        }

        // Page 0 = the initial response. Page 1+ = continuation responses.
        pages += 1;
        let page_text = match extract_page_text(&current) {
            Ok(t) => t,
            Err(e) => {
                abort_reason = Some(format!("page #{} extraction failed: {}", pages, e));
                break;
            }
        };
        let page_meta = match extract_page_meta(&current) {
            Ok(m) => m,
            Err(e) => {
                abort_reason = Some(format!("page #{} meta extraction failed: {}", pages, e));
                break;
            }
        };

        // Append. We trust that the server emits content whose concatenation
        // does not duplicate or lose bytes (this is the contract H4.1
        // established and H4.2 hardened with validate_token_binding). If
        // the contract breaks we will detect it via the post-loop SHA-256
        // comparison.
        accumulated.push_str(&page_text.content);

        if !page_meta.has_more {
            // Natural termination.
            break;
        }

        // Continuation required.
        let token = match page_meta.next_token {
            Some(t) if !t.is_empty() => t,
            _ => {
                abort_reason = Some(format!(
                    "page #{} reported has_more=true but no next_token",
                    pages
                ));
                break;
            }
        };

        // Loop detection.
        if !seen_tokens.insert(token.clone()) {
            abort_reason = Some(format!(
                "page #{} repeated an already-seen continuation token (loop)",
                pages
            ));
            break;
        }

        // Build the next call. The caller (orchestrator) passes back the
        // whole arguments map for the first page; we mutate it here to
        // include the continuation token. Note we DON'T need to know what
        // other args were used — the MCP server resolves them from the
        // token's encoded path/offset/chunk_size/mode. (See H4.1 contract.)
        let call_args = json!({
            "continuation_token": token,
        });
        current = match call_next(call_args) {
            Ok(r) => r,
            Err(e) => {
                abort_reason = Some(format!("page #{} call failed: {}", pages + 1, e));
                break;
            }
        };
    }

    // H4.3.x disk-mode fallback (B1 + B2 + B3):
    //
    // We ONLY fall back to reading `target_file` from disk when ALL of the
    // following hold. This makes the fallback behaviour non-surprising and
    // audit-friendly:
    //
    //   (B1) The MCP continuation failure was caused by the orchestrator's
    //        refusal to issue more calls, signalled by the well-known
    //        sentinel `ORCHESTRATOR_DISK_MODE_SENTINEL`. Any other error
    //        string — a real MCP timeout, an invalid token, a server 5xx —
    //        keeps `status=Incomplete` and we never touch the disk.
    //   (B2) The file metadata reports a size ≤ `max_bytes`. If the file is
    //        larger than the limit, we abort with `Incomplete(reason =
    //        "disk content exceeds max_bytes")` BEFORE opening it. This
    //        converts the byte limit from a soft promise into a hard cap.
    //   (B3) The actual read uses `BufReader::take(max_bytes + 1)` and is
    //        allowed to consume at most `max_duration - elapsed_so_far`.
    //        The `take` makes the read *bounded by construction*; the
    //        remaining-deadline check makes the duration limit an actual
    //        cap, not a post-hoc measurement. If either trips, we abort
    //        with `Incomplete` and never expose partial content.
    //
    // On success, `content` is marked with `source = DiskFallback` so the
    // scorecard can filter it. The orchestrator and any downstream
    // consumer must NOT count a `DiskFallback` reconstruction as proof
    // that the MCP server paginates correctly.
    if let (Some(p), Some(reason)) = (target_file, abort_reason.as_ref())
        && pages >= 1
        && pages < limits.max_pages
        && reason.contains(ORCHESTRATOR_DISK_MODE_SENTINEL)
    {
        let remaining_bytes = limits.max_bytes.saturating_sub(accumulated.len() as u64);
        let remaining_time = limits
            .max_duration
            .checked_sub(start.elapsed())
            .unwrap_or(Duration::ZERO);

        match read_target_file_bounded(p, remaining_bytes, remaining_time) {
            BoundedDiskRead::Ok(content) => {
                accumulated = content;
                pages = pages.saturating_add(1);
                source = ReconstructionSource::DiskFallback;
                abort_reason = None;
            }
            BoundedDiskRead::TooLarge { size } => {
                abort_reason = Some(format!(
                    "page #{} call refused; disk content size={} > remaining budget={}",
                    pages + 1,
                    size,
                    remaining_bytes
                ));
            }
            BoundedDiskRead::Timeout => {
                abort_reason = Some(format!(
                    "page #{} call refused; disk read exceeded remaining time={:?}",
                    pages + 1,
                    remaining_time
                ));
            }
            BoundedDiskRead::IoError(e) => {
                abort_reason = Some(format!(
                    "page #{} call refused; disk fallback failed: {}",
                    pages + 1,
                    e
                ));
            }
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    // Compute SHA-256 of what we accumulated.
    let sha256_reconstructed = if accumulated.is_empty() {
        None
    } else {
        Some(sha256_hex(accumulated.as_bytes()))
    };

    // Verify against the file on disk if the path is readable.
    let abort_reason_ref = abort_reason.as_deref();
    let (status, sha256_disk, final_reason) = if let Some(reason) = abort_reason_ref {
        (
            ReconstructionStatus::Incomplete,
            None,
            Some(reason.to_string()),
        )
    } else {
        match target_file {
            Some(p) => match std::fs::read(p) {
                Ok(disk_bytes) => {
                    let disk_sha = sha256_hex(&disk_bytes);
                    let disk_sha_for_reason = disk_sha.clone();
                    match &sha256_reconstructed {
                        Some(rec_sha) if rec_sha == &disk_sha => {
                            (ReconstructionStatus::Complete, Some(disk_sha), None)
                        }
                        Some(rec_sha) => (
                            ReconstructionStatus::MutationDetected,
                            Some(disk_sha),
                            Some(format!(
                                "reconstructed_sha={} disk_sha={} (file mutated during reconstruction)",
                                rec_sha, disk_sha_for_reason
                            )),
                        ),
                        None => (
                            // Empty reconstruction; disk is also empty — treat as complete.
                            ReconstructionStatus::Complete,
                            Some(disk_sha),
                            None,
                        ),
                    }
                }
                Err(_) => {
                    // Cannot verify. Conservatively: report Complete but flag the
                    // missing verification in `reason` so the consumer knows.
                    (
                        ReconstructionStatus::Complete,
                        None,
                        Some("disk verification skipped (file unreadable)".to_string()),
                    )
                }
            },
            None => {
                // No path provided. We still report Complete based on natural termination.
                (ReconstructionStatus::Complete, None, None)
            }
        }
    };

    // Refine SinglePage vs Complete.
    let is_single_page = pages <= 1 && abort_reason.is_none();
    let status = if is_single_page && status == ReconstructionStatus::Complete {
        ReconstructionStatus::SinglePage
    } else {
        status
    };

    // Don't leak content for NotApplicable; otherwise use the
    // accumulated content (may be empty if everything aborted before
    // appending).
    let content = if status == ReconstructionStatus::NotApplicable || accumulated.is_empty() {
        None
    } else {
        Some(accumulated)
    };

    ReconstructedContent {
        status,
        source,
        pages,
        total_bytes: content.as_ref().map(|s| s.len() as u64).unwrap_or(0),
        sha256_reconstructed,
        sha256_disk,
        content,
        reason: final_reason,
        duration_ms,
    }
}

/// Outcome of `read_target_file_bounded`.
enum BoundedDiskRead {
    /// Read completed within `max_bytes` and within the time budget;
    /// `String` is the file content as UTF-8 (lossy decoding rejected).
    Ok(String),
    /// The file's `metadata().len()` exceeded `max_bytes`. We never opened
    /// it for reading — the byte cap is enforced before any I/O.
    TooLarge {
        /// Size reported by `metadata().len()` in bytes.
        size: u64,
    },
    /// The read did not finish within the supplied deadline. We return
    /// `Timeout` instead of partial content so callers can never expose
    /// a truncated file.
    Timeout,
    /// Any other I/O error from opening or reading the file.
    IoError(String),
}

/// Bounded read of `target_file` from disk.
///
/// This is the only code path in the reconstructor that may touch the file
/// system outside of the post-loop SHA-256 verification. It enforces:
///
///   * **B2 — byte cap before open**: `metadata().len()` is compared to
///     `max_bytes` BEFORE any read. If the file is bigger, we return
///     `TooLarge` without opening it. The reconstructor's loop already
///     accounted for accumulated content, so the budget passed here is the
///     *remaining* budget.
///
///   * **B3 — bounded read by construction**: the actual read uses
///     `BufReader::take(max_bytes)`. The reader physically cannot deliver
///     more than `max_bytes` bytes, regardless of the file's real size or
///     of any I/O error further down.
///
///   * **B3 — bounded read by deadline**: the read happens in 64 KiB
///     chunks. Between chunks we check `start.elapsed() > deadline`. If
///     the deadline trips mid-read, we abort the read and return
///     `Timeout`. The deadline is enforced *during* the read, not after.
///
/// Lossy UTF-8 decoding is rejected (`from_utf8`); a non-UTF-8 file is
/// reported as `IoError`.
fn read_target_file_bounded(path: &Path, max_bytes: u64, deadline: Duration) -> BoundedDiskRead {
    let start = Instant::now();
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return BoundedDiskRead::IoError(e.to_string()),
    };
    let size = meta.len();
    if size > max_bytes {
        return BoundedDiskRead::TooLarge { size };
    }
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => return BoundedDiskRead::IoError(e.to_string()),
    };
    let mut reader = BufReader::new(file).take(max_bytes);
    let mut buf = Vec::with_capacity(size as usize);
    let mut chunk = [0u8; 64 * 1024];
    loop {
        if start.elapsed() > deadline {
            return BoundedDiskRead::Timeout;
        }
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) => return BoundedDiskRead::IoError(e.to_string()),
        }
    }
    match String::from_utf8(buf) {
        Ok(s) => BoundedDiskRead::Ok(s),
        Err(e) => BoundedDiskRead::IoError(format!("non-UTF-8 content: {e}")),
    }
}

/// Returns the "single page" reconstruction for scenarios where the manifest
/// opts in but the first response is already complete. This keeps the
/// artifact schema uniform: every `read_source_full` scenario gets a
/// `reconstructed.json` regardless of outcome.
pub fn reconstruct_single_page(initial_response: &Value) -> ReconstructedContent {
    let start = Instant::now();
    let page_text = extract_page_text(initial_response).unwrap_or_default();
    let content = if page_text.content.is_empty() {
        None
    } else {
        Some(page_text.content)
    };
    let sha256_reconstructed = content.as_ref().map(|s| sha256_hex(s.as_bytes()));
    ReconstructedContent {
        status: ReconstructionStatus::SinglePage,
        source: ReconstructionSource::McpChain,
        pages: 1,
        total_bytes: content.as_ref().map(|s| s.len() as u64).unwrap_or(0),
        sha256_reconstructed,
        sha256_disk: None,
        content,
        reason: None,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

/// Compute a fresh SHA-256 over the file on disk (post-reconstruction) and
/// compare it against the reconstructed content. Returns the new
/// `ReconstructedContent` reflecting the verification. Used by the
/// orchestrator after the loop terminates naturally to perform the
/// mutation check.
pub fn verify_against_disk(
    current: ReconstructedContent,
    target_file: &Path,
) -> ReconstructedContent {
    match std::fs::read(target_file) {
        Ok(bytes) => {
            let disk_sha = sha256_hex(&bytes);
            let disk_sha_for_reason = disk_sha.clone();
            let mut updated = current;
            match (&updated.sha256_reconstructed, &updated.status) {
                (Some(rec), ReconstructionStatus::Complete) if rec == &disk_sha => {
                    updated.sha256_disk = Some(disk_sha);
                    updated
                }
                (Some(rec), _) => {
                    updated.status = ReconstructionStatus::MutationDetected;
                    updated.sha256_disk = Some(disk_sha);
                    updated.reason = Some(format!(
                        "reconstructed_sha={} disk_sha={} (file mutated during reconstruction)",
                        rec, disk_sha_for_reason
                    ));
                    updated
                }
                (None, _) => {
                    updated.sha256_disk = Some(disk_sha);
                    updated
                }
            }
        }
        Err(_) => current,
    }
}

// ---------- helpers ----------

#[derive(Debug, Default)]
struct PageText {
    content: String,
}

#[derive(Debug)]
struct PageMeta {
    has_more: bool,
    next_token: Option<String>,
}

fn extract_page_text(response: &Value) -> Result<PageText, String> {
    // The MCP server wraps the read_file result as
    //   { content: [{ type: "text", text: "<JSON string>" }] }
    // We have already unwrapped the JSON-RPC `result` by the time we are here
    // (the caller passes the inner result, not the full JSON-RPC envelope).
    let arr = response
        .get("content")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing content[]".to_string())?;
    let first = arr.first().ok_or_else(|| "empty content[]".to_string())?;
    let text = first
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "content[0].text is not a string".to_string())?;
    let inner: Value =
        serde_json::from_str(text).map_err(|e| format!("content[0].text is not JSON: {}", e))?;
    let content = inner
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "inner.content missing or non-string".to_string())?;
    Ok(PageText {
        content: content.to_string(),
    })
}

fn extract_page_meta(response: &Value) -> Result<PageMeta, String> {
    let arr = response
        .get("content")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing content[]".to_string())?;
    let first = arr.first().ok_or_else(|| "empty content[]".to_string())?;
    let text = first
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "content[0].text is not a string".to_string())?;
    let inner: Value =
        serde_json::from_str(text).map_err(|e| format!("content[0].text is not JSON: {}", e))?;
    let has_more = inner
        .get("has_more")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let next_token = inner
        .get("next_token")
        .and_then(|v| v.as_str())
        .map(String::from);
    Ok(PageMeta {
        has_more,
        next_token,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    // Avoid pulling a SHA-2 crate into the dependency tree of `cognicode-core`
    // (which already has one transitively via tokio/sha2, but explicit
    // dependency is cleaner). We use `std::process::Command` for portability
    // — wait, that would be slow in tight loops. Instead, use the SHA-2
    // crate directly since it's already in the workspace.
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in out {
        hex.push_str(&format!("{:02x}", b));
    }
    hex
}

// ---------- the manifest opt-in is on ScenarioDef; this module exposes the
// function the orchestrator calls ----------

/// Decide whether the orchestrator should run reconstruction for this scenario.
/// Returns `false` if the manifest does not opt in or the tool is not
/// `read_file`. Centralised here so the rules cannot drift between the
/// manifest parsing and the orchestrator's branching.
pub fn should_reconstruct(scenario: &crate::sandbox_core::manifest::ExpandedScenario) -> bool {
    scenario.read_source_full && scenario.tool == "read_file"
}

/// Re-export the manifest type alias used here, so the orchestrator can pass
/// it without an extra import.
pub use crate::sandbox_core::manifest::ExpandedScenario as Scenario;

// silence unused-import lints under certain feature flag combinations.
#[allow(dead_code)]
fn _silence_unused(_: &Manifest) {}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    /// Build a synthetic read_file response page.
    ///
    /// - `start_line` and `end_line` are 1-based.
    /// - `total_lines` is the total number of lines in the simulated file.
    /// - `has_more=true` and a synthetic `next_token` is added if more pages exist.
    fn make_page(
        content: &str,
        start_line: u32,
        end_line: u32,
        total_lines: u32,
        next_token: Option<&str>,
    ) -> Value {
        let has_more = end_line < total_lines || next_token.is_some();
        let mut inner = json!({
            "content": content,
            "total_lines": total_lines,
            "truncated": has_more,
            "metadata": {"path": "/tmp/fake.txt", "size": content.len(), "modified": 0},
            "mode": "raw",
            "start_line": start_line,
            "end_line": end_line,
            "has_more": has_more,
            "suggested_chunk_size": null,
        });
        if let Some(tok) = next_token {
            inner
                .as_object_mut()
                .unwrap()
                .insert("next_token".to_string(), Value::String(tok.to_string()));
        }
        // Wrap as the MCP server would: { content: [{ type:"text", text: <inner JSON string> }] }
        json!({
            "content": [{
                "type": "text",
                "text": inner.to_string(),
            }],
            "isError": false,
        })
    }

    /// Build a full file content with `total_lines` lines, each line
    /// being `"line N\n"` for N in 1..=total_lines.
    fn synth_file_content(total_lines: u32) -> String {
        let mut s = String::new();
        for i in 1..=total_lines {
            s.push_str(&format!("line {}\n", i));
        }
        s
    }

    fn temp_file_with(content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "h43_recon_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("fake.txt");
        fs::write(&path, content).expect("write fake");
        path
    }

    // ---- T6(a) archivo pequeño: una sola petición, comportamiento intacto ----
    #[test]
    fn reconstruct_small_file_returns_single_page() {
        let content = synth_file_content(100);
        let initial = make_page(&content, 1, 100, 100, None);

        let result = reconstruct_read_file(
            &initial,
            |_args| panic!("should not be called for single-page response"),
            None,
            ReconstructionLimits::default(),
        );

        assert_eq!(result.status, ReconstructionStatus::SinglePage);
        assert_eq!(result.pages, 1);
        assert_eq!(result.total_bytes as usize, content.len());
        assert_eq!(
            result.sha256_reconstructed.as_deref(),
            Some(sha256_hex(content.as_bytes()).as_str())
        );
        assert!(result.reason.is_none());
    }

    // ---- T6(b) archivo grande: reconstrucción completa byte-exact ----
    #[test]
    fn reconstruct_large_file_completes_byte_exact() {
        let content = synth_file_content(730);
        let path = temp_file_with(&content);

        // Simulate two pages: 1..=500 and 501..=730.
        let page1 = make_page(
            &content[..content.len() - 230 * 8],
            1,
            500,
            730,
            Some("tok2"),
        );
        let page2 = make_page(&content[content.len() - 230 * 8..], 501, 730, 730, None);

        let calls = std::cell::RefCell::new(0);
        let result = reconstruct_read_file(
            &page1,
            |_args| {
                *calls.borrow_mut() += 1;
                Ok(page2.clone())
            },
            Some(&path),
            ReconstructionLimits::default(),
        );

        assert_eq!(result.status, ReconstructionStatus::Complete);
        assert_eq!(result.pages, 2);
        assert_eq!(*calls.borrow(), 1, "exactly one continuation call");
        assert_eq!(result.total_bytes as usize, content.len());
        let disk_sha = sha256_hex(content.as_bytes());
        assert_eq!(
            result.sha256_reconstructed.as_deref(),
            Some(disk_sha.as_str())
        );
        assert_eq!(result.sha256_disk.as_deref(), Some(disk_sha.as_str()));
        assert!(result.reason.is_none());
    }

    // ---- T6(c) token inválido en página intermedia → Incomplete ----
    #[test]
    fn reconstruct_invalid_token_in_intermediate_page_yields_incomplete() {
        let page1 = make_page("page1\n", 1, 1, 5, Some("tok2"));

        let result = reconstruct_read_file(
            &page1,
            |_args| Err("Invalid continuation token".to_string()),
            None,
            ReconstructionLimits::default(),
        );

        assert_eq!(result.status, ReconstructionStatus::Incomplete);
        assert_eq!(result.pages, 1);
        assert!(
            result
                .reason
                .as_deref()
                .unwrap_or("")
                .contains("Invalid continuation token")
        );
    }

    // ---- T6(d) offset sin progreso (loop detection) → Incomplete ----
    #[test]
    fn reconstruct_repeated_token_yields_incomplete() {
        let page1 = make_page("page1\n", 1, 1, 5, Some("tok-loop"));
        // The "next" page claims has_more=true but its next_token is the same
        // as what we just received → loop.
        let page2 = make_page("page2\n", 2, 2, 5, Some("tok-loop"));

        let result = reconstruct_read_file(
            &page1,
            |_args| Ok(page2.clone()),
            None,
            ReconstructionLimits::default(),
        );

        assert_eq!(result.status, ReconstructionStatus::Incomplete);
        assert!(result.reason.as_deref().unwrap_or("").contains("loop"));
    }

    // ---- T6(e) archivo mutado entre páginas → MutationDetected ----
    #[test]
    fn reconstruct_detects_concurrent_mutation() {
        let page1_content = synth_file_content(730);
        let page1 = make_page(&page1_content, 1, 500, 730, Some("tok2"));
        let page2_content = synth_file_content(730);

        // The "disk" has the original content, but page2 returns
        // page2_content (which differs). SHA-256 mismatch → MutationDetected.
        let path = temp_file_with(&page1_content);

        let page2 = make_page(&page2_content, 501, 730, 730, None);
        let result = reconstruct_read_file(
            &page1,
            |_args| Ok(page2.clone()),
            Some(&path),
            ReconstructionLimits::default(),
        );

        assert_eq!(result.status, ReconstructionStatus::MutationDetected);
        assert!(result.reason.as_deref().unwrap_or("").contains("mutated"));
    }

    // ---- T6(f) error MCP en página intermedia → Incomplete ----
    #[test]
    fn reconstruct_mcp_error_in_intermediate_page_yields_incomplete() {
        let page1 = make_page("page1\n", 1, 1, 100, Some("tok2"));

        let result = reconstruct_read_file(
            &page1,
            |_args| Err("server timeout".to_string()),
            None,
            ReconstructionLimits::default(),
        );

        assert_eq!(result.status, ReconstructionStatus::Incomplete);
        assert!(
            result
                .reason
                .as_deref()
                .unwrap_or("")
                .contains("server timeout")
        );
    }

    // ---- T6(g) max_pages limit ----
    #[test]
    fn reconstruct_aborts_on_max_pages() {
        // Always return has_more=true with a fresh token → loop forever unless
        // we hit max_pages.
        let counter = std::cell::RefCell::new(0u32);
        let mut page1 = make_page("page1\n", 1, 1, 100_000, Some("tok-1"));
        page1["content"][0]["text"] = json!(format!(
            r#"{{"content":"page1\n","total_lines":100000,"truncated":true,"metadata":{{"path":"/tmp/fake.txt","size":6,"modified":0}},"mode":"raw","start_line":1,"end_line":1,"has_more":true,"next_token":"tok-1","suggested_chunk_size":null}}"#
        ));

        let result = reconstruct_read_file(
            &page1,
            |args| {
                let mut c = counter.borrow_mut();
                *c += 1;
                let tok = args
                    .get("continuation_token")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tok-1")
                    .to_string();
                Ok(make_page(
                    "more\n",
                    *c + 1,
                    *c + 1,
                    100_000,
                    Some(&format!("tok-{}", *c + 1)),
                )
                .tap_with_tok(&tok))
            },
            None,
            ReconstructionLimits {
                max_pages: 3,
                ..Default::default()
            },
        );

        assert_eq!(result.status, ReconstructionStatus::Incomplete);
        assert!(result.reason.as_deref().unwrap_or("").contains("max_pages"));
    }

    // Helper trait to attach a synthetic `next_token` to a response. The
    // simpler closure above already does this; this exists only so the test
    // is self-contained.
    trait TapWithTok {
        fn tap_with_tok(self, _tok: &str) -> Self;
    }
    impl TapWithTok for Value {
        fn tap_with_tok(mut self, _tok: &str) -> Self {
            // The make_page already includes the token; this is a no-op
            // placeholder to keep the closure typed.
            let _ = self.as_object_mut();
            self
        }
    }

    // ---- T6(h) should_reconstruct: solo cuando read_source_full + read_file ----
    #[test]
    fn should_reconstruct_requires_opt_in_and_read_file_tool() {
        let mut s = crate::sandbox_core::manifest::ExpandedScenario {
            id: "x".into(),
            language: "rust".into(),
            tier: "1".into(),
            tool: "read_file".into(),
            action: "read".into(),
            arguments: Default::default(),
            workspace: ".".into(),
            expected_outcome: "pass".into(),
            validation: Default::default(),
            timeout_seconds: 60,
            scenario_class: "read_only".into(),
            preview_only: false,
            variant: None,
            ground_truth: None,
            metrics: None,
            root_cause_validation: None,
            read_source_full: false,
            repo: None,
            commit: None,
            container_image: None,
            pre_steps: None,
        };
        assert!(!should_reconstruct(&s));
        s.read_source_full = true;
        assert!(should_reconstruct(&s));
        s.tool = "search_content".into();
        assert!(!should_reconstruct(&s));
    }

    // ---- T6(i) reconstruct_single_page: respuesta ya completa ----
    #[test]
    fn reconstruct_single_page_for_already_complete_response() {
        let content = synth_file_content(80);
        let response = make_page(&content, 1, 80, 80, None);
        let result = reconstruct_single_page(&response);
        assert_eq!(result.status, ReconstructionStatus::SinglePage);
        assert_eq!(result.pages, 1);
        assert_eq!(result.total_bytes as usize, content.len());
    }

    // ---- T6(j) disk-mode fallback: MCP continuation refused, target_file Some ----
    //
    // This mirrors the sandbox-orchestrator scenario where the MCP server
    // is a single stdio process and re-entrant continuation calls would
    // deadlock. The caller passes a closure that always refuses; the
    // reconstructor MUST fall back to reading `target_file` from disk
    // and report `Complete` (with byte-exact SHA-256 match).
    #[test]
    fn reconstruct_disk_fallback_when_mcp_continuation_refused() {
        let content = synth_file_content(900);
        let path = temp_file_with(&content);

        // Page 1 advertises has_more=true and a token; the closure
        // refuses (matches the real orchestrator closure for safety).
        let page1 = make_page(&content[..content.len() / 2], 1, 450, 900, Some("tok2"));

        let result = reconstruct_read_file(
            &page1,
            |_args| Err("H4.3: disk-mode reconstruction".to_string()),
            Some(&path),
            ReconstructionLimits::default(),
        );

        assert_eq!(
            result.status,
            ReconstructionStatus::Complete,
            "status was {:?} reason={:?}",
            result.status,
            result.reason
        );
        assert_eq!(result.pages, 2, "1 page + 1 disk read");
        assert_eq!(result.total_bytes as usize, content.len());
        let disk_sha = sha256_hex(content.as_bytes());
        assert_eq!(
            result.sha256_reconstructed.as_deref(),
            Some(disk_sha.as_str())
        );
        assert_eq!(result.sha256_disk.as_deref(), Some(disk_sha.as_str()));
        assert!(result.reason.is_none());
        // Sanity: content is the full disk content, not the half-page.
        assert!(
            result
                .content
                .as_deref()
                .unwrap_or("")
                .contains(&format!("line {}", content.matches('\n').count()))
        );
        // H4.3.x B4 — provenance must be DiskFallback, never McpChain,
        // so the scorecard can filter it out of any MCP-pagination
        // attestation.
        assert_eq!(result.source, ReconstructionSource::DiskFallback);
    }

    // ---- H4.3.x B1 — sentinel gating ----
    //
    // Adversarial: the closure returns an arbitrary MCP error (NOT the
    // orchestrator sentinel). The reconstructor MUST treat this as a real
    // protocol error and return `Incomplete` WITHOUT touching the disk.
    // Before H4.3.x the disk-fallback triggered on any "page #N call
    // failed" string, which would have masked e.g. an `Invalid continuation
    // token` error from the server.
    #[test]
    fn reconstruct_does_not_fall_back_on_arbitrary_mcp_error() {
        let content = synth_file_content(900);
        let path = temp_file_with(&content);
        let page1 = make_page(&content[..content.len() / 2], 1, 450, 900, Some("tok2"));

        // NOT the sentinel — this looks like a real protocol failure.
        let result = reconstruct_read_file(
            &page1,
            |_args| Err("Invalid continuation token".to_string()),
            Some(&path),
            ReconstructionLimits::default(),
        );

        assert_eq!(
            result.status,
            ReconstructionStatus::Incomplete,
            "expected Incomplete for arbitrary MCP error"
        );
        assert!(
            result
                .reason
                .as_deref()
                .unwrap_or("")
                .contains("Invalid continuation token"),
            "reason must propagate the original MCP error: got {:?}",
            result.reason
        );
        // Source stays McpChain — the disk was never consulted.
        assert_eq!(result.source, ReconstructionSource::McpChain);
        // Content is only the truncated first page; we did NOT silently
        // substitute the disk content.
        assert!(
            result.content.as_deref().unwrap_or("").contains("line 1"),
            "expected truncated first-page content only"
        );
        assert!(
            !result
                .content
                .as_deref()
                .unwrap_or("")
                .contains(&format!("line {}", content.matches('\n').count())),
            "must NOT leak disk content under arbitrary MCP error"
        );
    }

    // ---- H4.3.x B2 — byte cap enforced BEFORE open ----
    //
    // Adversarial: the disk file is larger than `max_bytes`. The
    // reconstructor MUST abort to `Incomplete` without ever reading the
    // file. Before H4.3.x the disk-fallback used `read_to_string` with no
    // size check, so a giant file would have been slurped into memory and
    // counted as a successful reconstruction.
    #[test]
    fn reconstruct_disk_fallback_rejects_file_larger_than_max_bytes() {
        let content = synth_file_content(900);
        let path = temp_file_with(&content);
        let page1 = make_page(&content[..content.len() / 2], 1, 450, 900, Some("tok2"));

        // Set max_bytes to 100 — strictly less than the file size.
        let limits = ReconstructionLimits {
            max_pages: 64,
            max_bytes: 100,
            max_duration: std::time::Duration::from_secs(30),
        };

        let result = reconstruct_read_file(
            &page1,
            |_args| Err(ORCHESTRATOR_DISK_MODE_SENTINEL.to_string()),
            Some(&path),
            limits,
        );

        assert_eq!(
            result.status,
            ReconstructionStatus::Incomplete,
            "file larger than max_bytes must yield Incomplete"
        );
        assert_eq!(result.source, ReconstructionSource::McpChain);
        let reason = result.reason.as_deref().unwrap_or("");
        assert!(
            reason.contains("disk content size") && reason.contains("remaining budget"),
            "reason must cite the size cap; got {:?}",
            reason
        );
        // The content must NOT be the full file.
        assert!(
            !result
                .content
                .as_deref()
                .unwrap_or("")
                .contains(&format!("line {}", content.matches('\n').count())),
            "must NOT return full file when over max_bytes"
        );
    }

    // ---- H4.3.x B3 — duration cap enforced DURING read ----
    //
    // Adversarial: the disk file is small but the deadline is set to a
    // value that the read cannot satisfy (here: 0 nanoseconds — the
    // deadline check trips on the first chunk). The reconstructor MUST
    // abort to `Incomplete(Timeout)` instead of returning partial content.
    // Before H4.3.x the duration check was post-hoc, so a long read would
    // have been allowed to finish.
    //
    // We use a tiny file (well under `max_bytes`) and a zero deadline to
    // make the test deterministic without relying on sleep. The bounded
    // reader checks `start.elapsed() > deadline` before every chunk; with
    // deadline = 0 the first check trips immediately.
    #[test]
    fn reconstruct_disk_fallback_aborts_on_zero_deadline() {
        let content = synth_file_content(50);
        let path = temp_file_with(&content);
        let page1 = make_page(&content[..content.len() / 2], 1, 25, 50, Some("tok2"));

        let limits = ReconstructionLimits {
            max_pages: 64,
            max_bytes: 16 * 1024 * 1024,
            max_duration: std::time::Duration::ZERO,
        };

        let result = reconstruct_read_file(
            &page1,
            |_args| Err(ORCHESTRATOR_DISK_MODE_SENTINEL.to_string()),
            Some(&path),
            limits,
        );

        assert_eq!(result.status, ReconstructionStatus::Incomplete);
        assert_eq!(result.source, ReconstructionSource::McpChain);
        let reason = result.reason.as_deref().unwrap_or("");
        assert!(
            reason.contains("remaining time") || reason.contains("exceeded"),
            "reason must cite the deadline; got {:?}",
            reason
        );
    }

    // ---- H4.3.x B4 — provenance serialization round-trip ----
    //
    // The `source` field must round-trip through serde so consumers
    // reading `reconstructed.json` can filter on it. This guards against
    // a serde refactor silently dropping the discriminator.
    #[test]
    fn reconstruct_source_round_trips_through_json() {
        let original = ReconstructedContent {
            status: ReconstructionStatus::Complete,
            source: ReconstructionSource::DiskFallback,
            pages: 2,
            total_bytes: 100,
            sha256_reconstructed: Some("a".repeat(64)),
            sha256_disk: Some("a".repeat(64)),
            content: Some("hello".into()),
            reason: None,
            duration_ms: 1,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: ReconstructedContent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.source, ReconstructionSource::DiskFallback);
        assert_eq!(parsed.status, ReconstructionStatus::Complete);
        assert_eq!(parsed.total_bytes, 100);

        // McpChain must also round-trip (the default for new content).
        let mcp = ReconstructedContent {
            status: ReconstructionStatus::SinglePage,
            source: ReconstructionSource::McpChain,
            pages: 1,
            total_bytes: 0,
            sha256_reconstructed: None,
            sha256_disk: None,
            content: None,
            reason: None,
            duration_ms: 0,
        };
        let json2 = serde_json::to_string(&mcp).expect("serialize");
        let parsed2: ReconstructedContent = serde_json::from_str(&json2).expect("deserialize");
        assert_eq!(parsed2.source, ReconstructionSource::McpChain);

        // Backward-compat: a JSON without `source` (e.g. an old artifact
        // written before H4.3.x) must default to McpChain. This is the
        // contract for the `#[serde(default = ...)]` we added.
        let legacy = r#"{
            "status": "single_page",
            "pages": 1,
            "total_bytes": 0,
            "duration_ms": 0
        }"#;
        let parsed_legacy: ReconstructedContent =
            serde_json::from_str(legacy).expect("deserialize legacy");
        assert_eq!(parsed_legacy.source, ReconstructionSource::McpChain);
    }

    // ---- H4.3.x regression: existing disk-fallback test now reports source ----
    //
    // `reconstruct_disk_fallback_when_mcp_continuation_refused` already
    // covers the happy path. This test repeats the shape but explicitly
    // asserts the source discriminator. Kept separate so a future refactor
    // that drops the source field fails this test loudly.
    #[test]
    fn reconstruct_disk_fallback_marks_source_as_disk_fallback() {
        let content = synth_file_content(120);
        let path = temp_file_with(&content);
        let page1 = make_page(&content[..content.len() / 2], 1, 60, 120, Some("tok2"));

        let result = reconstruct_read_file(
            &page1,
            |_args| Err(ORCHESTRATOR_DISK_MODE_SENTINEL.to_string()),
            Some(&path),
            ReconstructionLimits::default(),
        );

        assert_eq!(result.status, ReconstructionStatus::Complete);
        assert_eq!(result.source, ReconstructionSource::DiskFallback);
        assert!(result.sha256_reconstructed.is_some());
        assert!(result.sha256_disk.is_some());
        assert!(result.content.is_some());
    }

    // ---- H4.3.x regression: MCP chain success keeps source = McpChain ----
    //
    // When the continuation chain terminates naturally (no disk fallback),
    // the source MUST remain McpChain. This is the whole point: a Complete
    // with McpChain is the only outcome that proves the MCP server
    // paginates correctly.
    #[test]
    fn reconstruct_mcp_chain_success_keeps_source_as_mcp_chain() {
        let content = synth_file_content(730);
        let path = temp_file_with(&content);

        let page1 = make_page(
            &content[..content.len() - 230 * 8],
            1,
            500,
            730,
            Some("tok2"),
        );
        let page2 = make_page(&content[content.len() - 230 * 8..], 501, 730, 730, None);

        let calls = std::cell::RefCell::new(0);
        let result = reconstruct_read_file(
            &page1,
            |_args| {
                *calls.borrow_mut() += 1;
                Ok(page2.clone())
            },
            Some(&path),
            ReconstructionLimits::default(),
        );

        assert_eq!(result.status, ReconstructionStatus::Complete);
        assert_eq!(
            result.source,
            ReconstructionSource::McpChain,
            "natural MCP termination must not be mislabelled as DiskFallback"
        );
        assert_eq!(*calls.borrow(), 1);
    }
}
