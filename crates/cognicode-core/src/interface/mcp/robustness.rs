//! B6.W1 — MCP server robustness primitives.
//!
//! Límites operacionales y diagnóstico en proceso del servidor stdio
//! MCP (`cognicode-mcp`). NO introduce un daemon: los límites viven
//! dentro del proceso del binario y los diagnósticos se leen del mismo
//! proceso (stdin/stdout siguen siendo JSON-RPC puro, sin un canal
//! auxiliar de observabilidad).
//!
//! Esta superficie es la verificación de B6.W1 contra el riesgo
//! R-B6-W1 ("introducir un daemon para resolver 'robustez' es un atajo
//! que duplica la superficie de fallo"). Si `ServerLimits` y
//! `ServerDiagnostics` viven aquí, en el mismo crate que la
//! implementación, y `default_limits()` se evalúa por unit test, el
//! daemon sería una duplicación sin valor.
//!
//! Two surfaces are exported:
//!   - `ServerLimits`: valores operacionales que el resto del crate
//!     puede consultar para decidir concurrencia, fanout, timeouts y
//!     paginación. La fuente de verdad es `default_limits()`.
//!   - `ServerDiagnostics`: snapshot en proceso del estado del
//!     servidor. Se alimenta de contadores atómicos actualizados por
//!     el dispatch (notas `note_request_started`, `note_request_finished`,
//!     `note_cancellation`, `note_timeout`) y de `/proc/self/status` para
//!     RSS. La función `collect_diagnostics()` es la única forma pública
//!     de leerlos, así el resto del crate no depende del layout interno.
//!
//! Bilingual doc comments (English / Spanish) per AGENTS.md.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// B6.W1 — Operational limits enforced by the stdio MCP server.
///
/// These are the values the rest of the crate queries to decide how to
/// schedule concurrency, cap recursion/fanout, time out a dispatch, and
/// hand back paginated continuations. The values themselves live in
/// `default_limits()` and are pinned by `tests::default_limits_are_sane`.
///
/// The fields mirror the JSON Schema we expose on the platform side
/// (one MCP server, one process). Keeping them here means a daemon
/// (R-B6-W1) would have to re-state them — which is precisely the
/// duplication we are refusing to introduce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerLimits {
    /// Maximum number of in-flight tool calls the server will hold open
    /// concurrently. Beyond this, new calls are rejected at the
    /// dispatch boundary (fail-closed).
    pub max_concurrent_requests: usize,
    /// Maximum depth of nested tool calls (graph walks, recursive
    /// composites). Counts the *outer* call as depth 1.
    pub max_request_depth: usize,
    /// Maximum bytes a single response may carry before the server
    /// truncates it and hands back a continuation token.
    pub max_response_bytes: usize,
    /// Maximum child calls one composite tool may fan out to (used by
    /// `graph_analyze` / `codebase_map` style tools).
    pub max_fanout: usize,
    /// Per-tool dispatch timeout in milliseconds. Today this is selected
    /// per-category (graph=60s, navigation=45s, search=500ms, default=30s)
    /// in `timeout_for_category`; this field is the upper bound the
    /// category lookup is allowed to choose.
    pub request_timeout_ms: u64,
    /// Grace period (ms) the server allows a cancelled tool to wind
    /// down cleanly before its worker is forcibly torn down.
    pub cancel_grace_ms: u64,
}

impl Default for ServerLimits {
    fn default() -> Self {
        Self {
            // stdio clients (Claude/OpenCode/Cursor) typically open 1-2
            // sessions; 32 leaves headroom for parallel agents.
            max_concurrent_requests: 32,
            // Graph analytics tops out at depth ~6 in practice
            // (analyze_impact → get_call_hierarchy → trace_path).
            max_request_depth: 8,
            // 4 MiB keeps one Mermaid render or one large search
            // response on a single frame; anything bigger is paginated.
            max_response_bytes: 4 * 1024 * 1024,
            // Composite tools fan out to <8 children today.
            max_fanout: 16,
            // Hard ceiling; per-category is shorter (500ms..60s).
            request_timeout_ms: 60_000,
            // 2s is enough for a tokio worker to drop its locks and exit.
            cancel_grace_ms: 2_000,
        }
    }
}

/// Returns the published operational limits.
///
/// The function exists (rather than a constant) so the test suite can
/// pin a single source of truth: if a value is wrong, the test fails
/// and the operator updates the function — not the constant and the test.
pub fn default_limits() -> ServerLimits {
    ServerLimits::default()
}

/// B6.W1 — Snapshot of process-local diagnostic state.
///
/// Read once via `collect_diagnostics()`. The struct is `Copy` so
/// callers can compare snapshots without a borrow fight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerDiagnostics {
    /// Process ID (`getpid()`).
    pub pid: u32,
    /// Currently open MCP sessions. 0 in stdio (one process == one
    /// session); exposed for parity with the HTTP server.
    pub open_sessions: usize,
    /// In-flight tool calls right now (counter goes up on
    /// `note_request_started` and down on `note_request_finished`).
    pub inflight_requests: usize,
    /// Cumulative tool calls served since process start.
    pub total_requests: u64,
    /// Cumulative cancellation notifications observed.
    pub total_cancellations: u64,
    /// Cumulative dispatch timeouts (tokio timeout fired).
    pub total_timeouts: u64,
    /// Peak RSS observed in bytes (read from `/proc/self/status`). 0 on
    /// non-Linux platforms — `collect_diagnostics` is honest about that.
    pub peak_memory_bytes: u64,
    /// Seconds since the first call to `collect_diagnostics()`. 0 on a
    /// fresh process within the first second.
    pub uptime_seconds: u64,
}

// --- process-local state ------------------------------------------------

static SERVER_START: OnceLock<Instant> = OnceLock::new();
static TOTAL_REQUESTS: AtomicU64 = AtomicU64::new(0);
static TOTAL_CANCELLATIONS: AtomicU64 = AtomicU64::new(0);
static TOTAL_TIMEOUTS: AtomicU64 = AtomicU64::new(0);
static INFLIGHT_REQUESTS: AtomicU64 = AtomicU64::new(0);
static PEAK_MEMORY_BYTES: AtomicU64 = AtomicU64::new(0);

fn start_instant() -> &'static Instant {
    SERVER_START.get_or_init(Instant::now)
}

/// Record the start of a tool dispatch. Increments `total_requests` and
/// `inflight_requests`. Called from the `call_tool_handler` boundary in
/// `rmcp_adapter` (out of scope for this WU; the wiring is a follow-up
/// once the unit tests pin the contract).
pub fn note_request_started() {
    TOTAL_REQUESTS.fetch_add(1, Ordering::Relaxed);
    INFLIGHT_REQUESTS.fetch_add(1, Ordering::Relaxed);
}

/// Record the end of a tool dispatch. Decrements `inflight_requests`.
/// Pairs with `note_request_started` — leaving it without the matching
/// finish would inflate `inflight_requests` indefinitely.
pub fn note_request_finished() {
    INFLIGHT_REQUESTS.fetch_sub(1, Ordering::Relaxed);
}

/// Record an inbound cancellation notification (`on_cancelled` in
/// `rmcp_adapter`). Independent of the dispatch outcome.
pub fn note_cancellation() {
    TOTAL_CANCELLATIONS.fetch_add(1, Ordering::Relaxed);
}

/// Record a dispatch timeout (tokio elapsed). The 500ms search budget
/// and the 60s graph budget both feed into this counter.
pub fn note_timeout() {
    TOTAL_TIMEOUTS.fetch_add(1, Ordering::Relaxed);
}

/// Refresh the peak-RSS counter from `/proc/self/status`.
///
/// Best-effort: if the file is missing or unparseable, the existing
/// peak is preserved. The peak is a *high-water mark*, not a sample.
fn refresh_peak_memory() {
    let Some(bytes) = read_vm_rss_bytes() else {
        return;
    };
    let mut current = PEAK_MEMORY_BYTES.load(Ordering::Relaxed);
    while bytes > current {
        match PEAK_MEMORY_BYTES.compare_exchange_weak(
            current,
            bytes,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(observed) => current = observed,
        }
    }
}

/// Read `VmRSS` from `/proc/self/status` in bytes. Returns `None` on
/// any failure (non-Linux, sandboxed, parse error). The caller treats
/// `None` as "no observation" and keeps the previous peak.
fn read_vm_rss_bytes() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            // Format: "VmRSS:	   12345 kB"
            let mut iter = rest.split_whitespace();
            let value = iter.next()?;
            let unit = iter.next().unwrap_or("kB");
            let kib: u64 = value.parse().ok()?;
            let bytes = match unit {
                "kB" | "KiB" => kib * 1024,
                "MB" | "MiB" => kib * 1024 * 1024,
                "B" => kib,
                _ => return None,
            };
            return Some(bytes);
        }
    }
    None
}

/// Collect a fresh snapshot of the process-local diagnostic state.
///
/// Side effect: refreshes the peak RSS counter so a long-running
/// server reports its true high-water mark even if no tool was called
/// in the interval. The counter refresh is `O(proc lines)` and is
/// cheaper than the alternative (sampling RSS from a separate thread).
pub fn collect_diagnostics() -> ServerDiagnostics {
    refresh_peak_memory();
    ServerDiagnostics {
        pid: std::process::id(),
        // stdio MCP has exactly one session per process; HTTP/SSE
        // reports its own. Both surfaces return the same struct.
        open_sessions: 0,
        inflight_requests: INFLIGHT_REQUESTS.load(Ordering::Relaxed) as usize,
        total_requests: TOTAL_REQUESTS.load(Ordering::Relaxed),
        total_cancellations: TOTAL_CANCELLATIONS.load(Ordering::Relaxed),
        total_timeouts: TOTAL_TIMEOUTS.load(Ordering::Relaxed),
        peak_memory_bytes: PEAK_MEMORY_BYTES.load(Ordering::Relaxed),
        uptime_seconds: start_instant().elapsed().as_secs(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The published limits are sane. The exact numbers are tuned, not
    /// sacred, but every field has to be positive and the byte/time
    /// fields have to be large enough that real workloads fit.
    #[test]
    fn default_limits_are_sane() {
        let l = default_limits();
        assert!(
            l.max_concurrent_requests >= 1,
            "max_concurrent_requests must be ≥ 1, got {}",
            l.max_concurrent_requests
        );
        assert!(
            l.max_request_depth >= 1,
            "max_request_depth must be ≥ 1, got {}",
            l.max_request_depth
        );
        assert!(
            l.max_response_bytes >= 1024 * 1024,
            "max_response_bytes must be ≥ 1 MiB, got {}",
            l.max_response_bytes
        );
        assert!(
            l.max_fanout >= 1,
            "max_fanout must be ≥ 1, got {}",
            l.max_fanout
        );
        assert!(
            l.request_timeout_ms >= 1_000,
            "request_timeout_ms must be ≥ 1s, got {}",
            l.request_timeout_ms
        );
        // The hard ceiling has to allow the longest per-category
        // budget (graph = 60s) to fit comfortably.
        assert!(
            l.request_timeout_ms >= 60_000,
            "request_timeout_ms must be ≥ 60s to fit the graph budget"
        );
        assert!(
            l.cancel_grace_ms > 0,
            "cancel_grace_ms must be > 0, got {}",
            l.cancel_grace_ms
        );
    }

    /// `collect_diagnostics` reports a non-zero uptime after the
    /// process has been alive for > 1 s. We sleep a full 1100 ms so the
    /// `as_secs()` rounding has cleared zero.
    #[test]
    fn collect_diagnostics_returns_non_zero_uptime_after_delay() {
        let before = collect_diagnostics().uptime_seconds;
        // Snapshot the start time once so the measurement is not a race
        // against `OnceLock::get_or_init(Instant::now)` happening for
        // the first time mid-test.
        let _ = start_instant();
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        let after = collect_diagnostics().uptime_seconds;
        assert!(
            after >= before,
            "uptime should be monotonically non-decreasing: before={before} after={after}"
        );
        // After a 1.1s sleep, the post-sleep snapshot must be ≥ 1.
        // (The first collect_diagnostics() also pinned start_instant,
        // so the elapsed time is at least the sleep duration plus a
        // few µs of test setup.)
        assert!(
            after >= 1,
            "uptime must be ≥ 1s after a 1.1s sleep, got {after}"
        );
        // The PID is always positive and matches `std::process::id()`.
        let d = collect_diagnostics();
        assert_eq!(d.pid, std::process::id());
    }

    /// `note_request_started` increments the total counter. We do NOT
    /// assert that the counter went up by exactly 1, because the unit
    /// tests in this crate run in parallel and other tests may have
    /// nudged the counter in between.
    #[test]
    fn total_requests_increments_on_a_synthetic_call() {
        let before = collect_diagnostics().total_requests;
        let before_inflight = collect_diagnostics().inflight_requests;
        note_request_started();
        let after_inflight = collect_diagnostics().inflight_requests;
        assert!(
            after_inflight > before_inflight,
            "inflight must grow on note_request_started: before={before_inflight} after={after_inflight}"
        );
        note_request_finished();
        let after = collect_diagnostics().total_requests;
        assert!(
            after > before,
            "total_requests must increment: before={before} after={after}"
        );
        let after_inflight = collect_diagnostics().inflight_requests;
        assert!(
            after_inflight <= before_inflight,
            "inflight must not grow past start: before={before_inflight} after={after_inflight}"
        );
    }

    /// The cancellation grace window is non-zero. The exact value is
    /// tuned (2s today), but a zero would mean the server kills
    /// in-flight tools immediately on `notifications/cancelled`, which
    /// is what R-B6-W1 is meant to prevent.
    #[test]
    fn cancel_grace_ms_is_nonzero() {
        let l = default_limits();
        assert!(l.cancel_grace_ms > 0, "cancel_grace_ms must be > 0");
        // And short enough that an agent does not hang forever waiting
        // for a hung tool to die.
        assert!(
            l.cancel_grace_ms <= 30_000,
            "cancel_grace_ms must be ≤ 30s, got {}",
            l.cancel_grace_ms
        );
    }
}
