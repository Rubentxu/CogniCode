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

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub use cognicode_core::infrastructure::process_memory::read_peak_memory_bytes;

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
    /// Per-tool dispatch timeout in milliseconds. The window of a tool
    /// is the one its category declares (graph=60s, navigation=45s,
    /// search=500ms, default=30s) in `timeout_for_category`, raised to
    /// the window of the graph for a tool that needs the graph; this
    /// field is the upper bound the window is allowed to choose.
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
            // Hard ceiling; the windows run from 500ms to the graph's 60s.
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

/// The window the build of the graph needs.
///
/// The build is the work of the `graph` category, and the window of
/// that category is this one: it is twice the thirty seconds that the
/// build of this repository measures, and it is the ceiling the server
/// publishes (`request_timeout_ms`).
pub const GRAPH_WINDOW: Duration = Duration::from_secs(60);

/// M3.2: The dispatch window of a category.
///
/// Categories with heavy graph algorithms (PageRank, IDF, communities)
/// get 60s; LSP-backed navigation gets 45s; a query over the symbol
/// index gets 500ms; a walk of the tree, like the content search, is
/// the work of the `file` category, which gets 30s.
pub fn timeout_for_category(category: &str) -> Duration {
    match category {
        "graph" => GRAPH_WINDOW,
        "navigation" => Duration::from_secs(45),
        "search" => Duration::from_millis(500),
        _ => Duration::from_secs(30),
    }
}

/// M3.2: The dispatch window of a tool.
///
/// The category says what kind of work the tool is, and the window has
/// to cover that work. A tool that needs the graph gets the window of
/// the graph: the build of the graph is part of its work, and a
/// deadline shorter than the build answers the caller with a timeout
/// while the graph is built anyway — the work is done and thrown away.
///
/// The requirement is the one the catalogue declares
/// (`runtime_metadata.requires_graph`), the same declaration the wire
/// publishes, so the window and the metadata cannot disagree about the
/// same tool.
///
/// A tool whose work carries a bound gets a window that covers the
/// longest that work may take: the work answers within that bound, and a
/// window shorter than it answers the caller with a timeout instead of
/// with the result of the work that did fit. The bound is declared by the
/// layer that does the work and reaches the server through the interface,
/// so the window and the work read the same number.
pub fn timeout_for_tool(tool_name: &str, category: &str) -> Duration {
    let window = timeout_for_category(category);
    let window = if tool_requires_graph(tool_name) {
        window.max(GRAPH_WINDOW)
    } else {
        window
    };
    match cognicode_core::interface::mcp::work_budget::for_tool(tool_name) {
        Some(worst_case) => window.max(worst_case + WORK_BUDGET_MARGIN),
        None => window,
    }
}

/// The margin the dispatch window leaves over the longest the work may
/// take.
///
/// The work answers within its own bound, so the window has to be longer
/// than that bound: the margin is the room the answer needs to travel
/// back. Without it the deadline of the dispatch fires at the same
/// moment the work ends, and the caller gets a timeout instead of the
/// result.
const WORK_BUDGET_MARGIN: Duration = Duration::from_secs(5);

/// Whether the catalogue declares that the tool needs the graph.
///
/// The oracle is the catalogue, the same one the wire publishes: a tool
/// that needs the graph and does not say so gets a window that cuts
/// its own work.
fn tool_requires_graph(tool_name: &str) -> bool {
    static MAP: OnceLock<HashMap<String, bool>> = OnceLock::new();
    let map = MAP.get_or_init(|| {
        cognicode_core::interface::catalog::load_tool_catalog_active()
            .into_iter()
            .map(|descriptor| {
                let needs = descriptor
                    .runtime_metadata
                    .get("requires_graph")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
                (descriptor.name, needs)
            })
            .collect()
    });
    map.get(tool_name).copied().unwrap_or(false)
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
    read_vm_size_bytes("VmRSS:")
}

/// Read a memory size from `/proc/self/status`, in bytes.
///
/// The field is named by the caller: `VmRSS:` for the resident set of the
/// moment. Returns `None` on any failure (non-Linux, sandboxed, parse error):
/// the caller treats `None` as "no observation".
fn read_vm_size_bytes(field: &str) -> Option<u64> {
    let text = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(field) {
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

    /// M3.2 — The window of each category is what the product gives it.
    #[test]
    fn the_window_of_each_category_is_the_one_the_product_gives_it() {
        // graph analytics is allowed the longest window (60s)
        assert_eq!(timeout_for_category("graph"), GRAPH_WINDOW);
        // LSP navigation is allowed 45s
        assert_eq!(timeout_for_category("navigation"), Duration::from_secs(45));
        // a query over the symbol index answers from an index that is
        // already built, so it is given the narrowest window
        assert_eq!(timeout_for_category("search"), Duration::from_millis(500));
        // file operations default to 30s: that is the window of a walk
        // of the tree, like the content search
        assert_eq!(timeout_for_category("file"), Duration::from_secs(30));
        // composite/quality/refactor/aix are also 30s
        assert_eq!(timeout_for_category("composite"), Duration::from_secs(30));
        assert_eq!(timeout_for_category("quality"), Duration::from_secs(30));
        assert_eq!(timeout_for_category("refactor"), Duration::from_secs(30));
        assert_eq!(timeout_for_category("aix"), Duration::from_secs(30));
    }

    /// An unknown category falls through to the default, so a window is
    /// never lost by a typo.
    #[test]
    fn an_unknown_category_falls_through_to_the_default_window() {
        assert_eq!(
            timeout_for_category("unknown_category"),
            Duration::from_secs(30)
        );
        assert_eq!(timeout_for_category(""), Duration::from_secs(30));
    }

    /// The window of every tool that needs the graph covers the build
    /// of the graph.
    ///
    /// The build is part of the work of a tool that needs the graph,
    /// and the window of that build is the one of the `graph` category.
    /// A window shorter than the build answers the caller with a
    /// timeout while the graph is built anyway: the work is done and
    /// thrown away.
    ///
    /// The requirement and the window are read from the same
    /// declaration, so this contract watches the two together: a tool
    /// that says it needs the graph and does not get the window of the
    /// build is the defect.
    #[test]
    fn the_window_of_every_tool_that_needs_the_graph_covers_the_build() {
        for descriptor in cognicode_core::interface::catalog::load_tool_catalog_active() {
            let needs = descriptor
                .runtime_metadata
                .get("requires_graph")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            if !needs {
                continue;
            }
            let window = timeout_for_tool(&descriptor.name, &descriptor.category);
            assert!(
                window >= GRAPH_WINDOW,
                "{} needs the graph and its category ({}) gives it {window:?}: the build of the \
                 graph does not fit in that window, so the answer would be replaced by a timeout \
                 while the graph is built anyway",
                descriptor.name,
                descriptor.category
            );
        }
    }

    /// The window of the tool that populates the semantic index covers
    /// the population.
    ///
    /// The population is a walk of the tree and an extraction of the
    /// symbols of every file of it: the same work as the build of the
    /// graph, and no more expensive. A window shorter than that work
    /// answers the caller with a timeout while the index is populated
    /// anyway — and the population cannot be aborted, so the work would
    /// be done and thrown away.
    ///
    /// The tool declares that it needs the graph, and the window that
    /// covers the build of the graph is the one that covers the
    /// population. This contract watches that link: a tool that stopped
    /// declaring that it needs the graph would drop to the window of its
    /// category —half a second for the `search` one— and every call would
    /// be cut while the index was populated.
    #[test]
    fn the_window_of_the_tool_that_populates_the_index_covers_the_population() {
        let window = timeout_for_tool("nl_to_symbol", "search");
        assert!(
            window >= GRAPH_WINDOW,
            "the population of the semantic index does not fit in {window:?}: the index is \
             populated anyway and the answer is thrown away"
        );
    }

    /// The window of a tool whose work carries a bound covers the
    /// longest that work may take.
    ///
    /// The verification compiles every candidate it retrieved, and that
    /// work takes longer than the window of the `search` category its
    /// tool belongs to. A window shorter than the work answers the
    /// caller with a timeout while the work is running, and the work
    /// that was done is thrown away.
    ///
    /// The bound is read from the layer that does the work, so the
    /// window and the work cannot disagree about the same call.
    #[test]
    fn the_window_of_a_tool_that_carries_work_covers_that_work() {
        let worst_case =
            cognicode_core::interface::mcp::work_budget::for_tool("retrieve_and_verify")
                .expect("the verification declares the longest its work may take");
        let window = timeout_for_tool("retrieve_and_verify", "search");
        assert!(
            window > worst_case,
            "the window has to leave room to answer after the work ends: {window:?} vs {worst_case:?}"
        );
        assert!(
            window <= Duration::from_millis(default_limits().request_timeout_ms),
            "the window cannot pass the ceiling the server publishes: {window:?}"
        );
    }

    /// No window passes the ceiling the server publishes.
    ///
    /// The ceiling is the upper bound the window is allowed to choose.
    /// A tool whose work carries a budget gets a window derived from
    /// that budget, and a budget that grows past the ceiling would take
    /// the window with it: the contract says it does not happen.
    #[test]
    fn no_window_passes_the_published_ceiling() {
        let ceiling = Duration::from_millis(default_limits().request_timeout_ms);
        for descriptor in cognicode_core::interface::catalog::load_tool_catalog_active() {
            let window = timeout_for_tool(&descriptor.name, &descriptor.category);
            assert!(
                window <= ceiling,
                "{}: the window {window:?} passes the ceiling {ceiling:?}",
                descriptor.name
            );
        }
    }

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
