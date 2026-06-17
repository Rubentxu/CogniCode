# Kernel Exploration: File Watcher Integration

## Current State

### Watcher module exists but is dead code

`crates/cognicode-core/src/application/ingest/watcher.rs` (122 lines) defines three
public items:

- `pub fn start_watcher(root: PathBuf) -> mpsc::UnboundedReceiver<Vec<PathBuf>>`
  — spawns a background `std::thread` with `notify::recommended_watcher`. Filters
  by `EventKind::{Modify, Create}` and by extension via `is_watchable`.
- `pub async fn debounce_changes(rx, window_ms) -> mpsc::Receiver<Vec<PathBuf>>`
  — coalesces events within a window using `tokio::time::interval`. Dedups with
  `sort() + dedup()`.
- `fn is_watchable(path: &Path) -> bool` — allow-list of ~30 extensions covering
  code, IaC, config, docs.

The module is registered in `mod.rs:17` (`pub mod watcher;`) but **never imported
or called from any other file**. There is no `use crate::application::ingest::watcher`
anywhere in the workspace.

### Critical bugs in the existing watcher

- `start_watcher` ends in `loop { std::thread::sleep(Duration::from_secs(60)); }`
  (line 67-70). This is a placeholder — the watcher never shuts down cleanly.
  The receiver side has no way to drop the watcher.
- `start_watcher` clones `tx` into the closure but drops the original `tx` after
  creating the thread (line 24), so the channel works via the closure's clone. OK,
  but the outer `tx` clone on line 27 is dead.
- `EventKind::Modify(_)` on macOS kqueue fires once per write; the debouncer in
  `debounce_changes` is a custom implementation that re-creates the timer per
  receive (not optimal — should reset on each event).
- No tests anywhere in `watcher.rs` (no `#[test]`, no `#[cfg(test)] mod tests`).

### Dependency state

- `notify = "7"` is in workspace `Cargo.toml:82` and re-exported to
  `cognicode-core/Cargo.toml:81`. Already built and in `Cargo.lock`.
- `notify-debouncer-full` is **NOT** a dependency. The roadmap (line 273) says
  use it, but the team shipped with raw `notify` and a hand-rolled debouncer.
- `cognicode-mcp` does NOT depend on `notify` directly; only `cognicode-core`
  pulls it in.

### Existing pipeline to feed changes into

- `run_scan(repo, cache, workspace_id, root, on_progress) -> ScanResult` in
  `crates/cognicode-core/src/application/ingest/service.rs:35` is the canonical
  re-scan entry point. Stages: Scan → Extract → PgUpsert → Resolve → Cluster →
  Analyze → Report → Refresh. Uses `pg_advisory_lock(hashtext(workspace_id))`
  for concurrency (ADR-023).
- `IngestController::start_scan(&self, workspace_id)` in
  `crates/cognicode-core/src/application/ingest/controller.rs:213` spawns a
  tokio task that calls `run_scan`. Returns `ScanAccepted { job_id, ... }`.
- `IngestController` is already wired into `cognicode-runtime::Runtime::into_api_state`
  (`crates/cognicode-runtime/src/lib.rs:103-109`) and exposed via
  `cognicode-explorer/src/api.rs` at `POST /api/workspaces/:id/scan`.

### MCP server has no background-task spawn point today

`crates/cognicode-mcp/src/server.rs` (285 lines) builds the HandlerContext
synchronously, then `axum::serve(listener, app).await` blocks the main task.
There is no `tokio::spawn` anywhere in the file. The cwd is captured in
`Args::cwd` (default `.`) and propagated to `HandlerContext::with_working_dir`.

The MCP `handle_build_graph` (`crates/cognicode-core/src/interface/mcp/handlers/mod.rs:976`)
does NOT use the new pipeline — it calls the legacy
`analysis_service.build_project_graph(directory)` path, which uses PetGraphStore
(not PG) and never refreshes the GraphCache via `apply_events`. So Mode B
(`--postgres`) is half-implemented in `crates/cognicode-mcp/src/main.rs:91`.

### GraphCache broadcast mechanism is fully wired

`crates/cognicode-core/src/infrastructure/graph/graph_cache.rs`:
- `ArcSwap<Mutex<VersionedGraphCache>>` with default retention 2 (ADR-035).
- `broadcast::Sender<GraphEvent>` with capacity 16 (line 47).
- Sends `GraphEvent::GraphReplaced` on `set`, `GraphEvent::GraphModified` on
  `apply_events`, `GraphEvent::GraphCleared` on `clear`.
- Subscribers via `cache.subscribe() -> broadcast::Receiver<GraphEvent>`.

Any caller that triggers `run_scan` will get the broadcast for free because
the Refresh stage calls `refresh_from_pg` which goes through `apply_events`.

### PG NOTIFY/LISTEN is designed but NOT wired

ADR-022 (`docs/adr/ADR-022-pg-trigger-notify-incremental-refresh.md`) designs
a `notify_graph_change()` trigger that fires `pg_notify('graph_updated', ...)`
on `graph_nodes` INSERT/UPDATE/DELETE, plus a `LISTEN graph_updated` listener
loop. **No implementation exists** in the workspace today — the watcher does
not need PG NOTIFY to function (run_scan already broadcasts via apply_events).

## Context Quality

- **Level:** C2
- **Evidence Present:**
  - `crates/cognicode-core/src/application/ingest/watcher.rs` (partial impl)
  - `docs/ingest-pipeline-roadmap.md` §File watcher lines 260-285 (planned approach)
  - ADR-022 (incremental refresh + PG trigger design)
  - ADR-023 (advisory lock pattern)
  - ADR-017 (full pipeline architecture)
  - `IngestController` + `run_scan` (the call surface)
  - GraphCache `broadcast::Sender<GraphEvent>` (the publish side)
  - Runtime composition root (`crates/cognicode-runtime/src/lib.rs`)
  - MCP server startup pattern (`crates/cognicode-mcp/src/server.rs`)
- **Missing Context:**
  - No multi-workspace watcher design
  - No decision on `notify-debouncer-full` vs raw `notify`
  - No test for end-to-end file touch → cache update
  - No graceful shutdown semantics in the existing watcher
- **Recommended Effort:** verify

## Knowledge Coverage

| Class | Status | Evidence | Gap Impact |
|------|--------|----------|------------|
| Roadmap/Backlog | present | `docs/ingest-pipeline-roadmap.md` lines 260-285 + §Pending Work line 401 | Plan is clear |
| Work Items | present | `crates/cognicode-core/src/application/ingest/watcher.rs` | Implementation exists but unwired; not dead-on-arrival |
| Architecture/ADRs | present | ADR-017 (pipeline), ADR-022 (incremental), ADR-023 (advisory lock), ADR-025 (MCP dual mode) | Architecture is settled; only integration is missing |
| Ownership | missing | No OWNERS, no author tag on watcher.rs; runtime vs MCP integration site is undecided | Blocking — who decides the integration site? |
| Learnings | missing | No postmortems; no notes on why raw `notify` over `notify-debouncer-full` | May rediscover tradeoffs (timer reset, shutdown, kqueue quirks) |

## Problem Taxonomy

| Axis | Applies | Evidence |
|------|---------|----------|
| Domain modeling | No | Watcher shape (events → batch → debounce → scan) is correct |
| Boundary/seam | Yes | Watcher must serialize against `pg_advisory_lock` taken by explicit scans; cannot run two scans of same workspace concurrently |
| Coupling/connascence | Yes | Watcher thread → tokio task boundary; debounce window must align with scan lock + extraction batching |
| API contract | Yes | New public API needed: `IngestController::start_watcher(workspace_id) -> WatcherHandle` |
| Refactor/legacy | Yes | `handle_build_graph` uses legacy `analysis_service.build_project_graph` (Mode A path); watcher must NOT use that path — must call `run_scan` |
| Event/CQRS | Yes | Watcher is a Command (trigger scan); `GraphCache::subscribe` is the Query side (SSE) |
| Testing | Yes | Zero tests in watcher.rs; no end-to-end test |
| Security/operations | Yes | Multi-tenant: watcher must respect workspace boundaries; bounded debounce; graceful shutdown; rate-limit rescan storms |

## Domain Language And Invariants

- **Domain Language (resolved):**
  - **File Watcher** = background task that observes file system mutations and triggers incremental scans (CONTEXT.md §Open Questions)
  - **Debounce** = coalescing rapid file events into a single batch within a time window (500ms per roadmap)
  - **ScanManifest** = PG table tracking `{file_path, content_hash, mtime, status}` — the change-detection manifest used by `scan_for_changes`
  - **PG NOTIFY/LISTEN** = trigger-based notification (ADR-022) — designed, NOT wired
  - **GraphEvent** = `SymbolAdded | SymbolRemoved | SymbolModified | DependencyAdded | DependencyRemoved | GraphReplaced | GraphCleared | GraphModified` — already broadcasts on `run_scan`
- **Invariants:**
  - `run_scan` acquires `pg_advisory_lock(hashtext(workspace_id))` → watcher-triggered scans serialize with explicit ones
  - `refresh_from_pg` calls `apply_events` → broadcast fires `GraphEvent::GraphModified` automatically
  - Watcher must NEVER block the thread; must use tokio task + bounded channel
- **Unresolved ambiguities:**
  - Should watcher drive `run_scan` (full pipeline including cluster/analyze) or a slim `run_scan_paths(changed_paths)` (incremental-only fast path)?
  - One WatcherHandle per workspace, or a global watcher with multi-root support?
  - Mode A (standalone, no PG) — does watcher trigger legacy `analysis_service.build_project_graph`?

## Knowledge Gaps

- **`notify-debouncer-full` is planned but not in deps** — roadmap calls for it; raw `notify` is what we have. Either add the dep, or keep raw `notify` and harden the manual debounce.
- **Watcher is dead code** — module exists, types are public, zero callers.
- **No shutdown semantics** — `start_watcher` runs forever with no `Drop` or `stop()`.
- **No integration test** — nothing verifies file touch → debounce → scan → cache update end-to-end.
- **Multi-workspace design unclear** — runtime can serve N workspaces; how many watchers?
- **PG NOTIFY listener not implemented** — ADR-022 designs it but no `LISTEN graph_updated` anywhere. Not blocking for v1 watcher because `run_scan` already broadcasts via `apply_events`.
- **MCP Mode B incomplete** — `handle_build_graph` uses legacy path; `--postgres` in `crates/cognicode-mcp/src/main.rs:91` is half-implemented.

## Affected Areas

- `crates/cognicode-core/src/application/ingest/watcher.rs` — needs shutdown, tests, debounce reset semantics, integration signature
- `crates/cognicode-core/src/application/ingest/controller.rs` — needs `start_watcher(workspace_id)` returning `WatcherHandle` (or new module)
- `crates/cognicode-core/src/application/ingest/service.rs` — possibly extract `run_scan_paths` (incremental fast path)
- `crates/cognicode-runtime/src/lib.rs` — composition root; needs to spawn watchers and own handles for graceful shutdown
- `crates/cognicode-mcp/src/server.rs` — has no background task today; needs `tokio::spawn` (Mode B only) OR the watcher lives in `cognicode-explorer` HTTP API only
- `crates/cognicode-explorer/src/api.rs` — already wires `IngestController`; could expose watcher lifecycle endpoints
- `Cargo.toml` (workspace) — needs `notify-debouncer-full = "0.4"` if we adopt it
- `CONTEXT.md` line 327 — Open Questions "File watcher integration" should be checked after implementation
- Possibly new ADR (ADR-029 or extension of ADR-022) for watcher lifecycle + debounce policy

## Options

| Option | Pros | Cons | Effort |
|--------|------|------|--------|
| **A.** Add `notify-debouncer-full` 0.4; extend `IngestController::start_watcher` returning `WatcherHandle`; spawn from `cognicode-runtime::bootstrap` per registered workspace; on events call `run_scan` (or slim `run_scan_paths`) | Battle-tested debounce; clean per-workspace lifecycle; reuses run_scan; trivial to test; advisory lock prevents races | Requires runtime to know which workspaces to watch; small new dep | M (1-2 days) |
| **B.** Keep raw `notify`; fix watcher.rs (Drop impl, timer reset, tests); wire from runtime as in A | No new dep; matches what is already in Cargo.lock; less to learn | Reinventing debounce semantics; more edge cases to cover manually | M (1-2 days) |
| **C.** Spawn watcher from MCP server.rs (only Mode B / `--postgres`) | Minimal composition-root change; localized to MCP path | Only works when MCP binary is the entrypoint; doesn't help the Explorer HTTP API binary | S-M (1 day) |
| **D.** Defer wiring; only fix watcher.rs (shutdown + tests + `notify-debouncer-full`) | Lowest risk; least churn | Feature remains dead code; doesn't close the loop; bumps version without value | S (4-6 hours) |

## Entropy Envelope

- **Method:** heuristic + CogniCode-style
- **Coupling risk:** medium
  - Watcher → `IngestController` → `run_scan` → `refresh_from_pg` → `GraphCache::apply_events()` → broadcast
  - Connascence of timing: watcher debounce window (500ms) vs scan lock duration (50-2000ms) vs extract batching (10 files per txn)
  - OCP risk: adding a new event source (network FS, manual API, GH webhook) needs a new ingestion path; `IngestController::start_scan` already absorbs this — good
- **Notes:**
  - Watcher thread ↔ tokio task boundary is the biggest source of entropy (sync `notify` callback → async mpsc)
  - PG trigger NOTIFY/LISTEN (ADR-022) is orthogonal to the watcher; not blocking for v1
  - Multi-tenant correctness depends on workspace resolver correctness — already an `Arc<dyn WorkspaceResolver>` in `IngestController`

## Recommendation

**Option A + B hybrid** (use `notify-debouncer-full`, but only if it adds clear value over hand-rolled):

1. **Decide debounce strategy.** Ask the user: adopt `notify-debouncer-full` 0.4 (roadmap-aligned, battle-tested) OR keep raw `notify` (zero new deps, harder to maintain). Default to A.
2. **Extend `IngestController`** with `pub async fn start_watcher(&self, workspace_id: &str) -> Result<WatcherHandle, String>`:
   - Resolves workspace_id → root path via `WorkspaceResolver`
   - Spawns a tokio task owning the `Debouncer` (or `Watcher`)
   - On debounced events, collects paths, then calls a new `run_scan_paths(repo, cache, ws_id, root, paths)` helper — slim incremental-only path (skips cluster/analyze/report if path count < threshold)
   - Returns `WatcherHandle { stop_tx: oneshot::Sender<()>, join: JoinHandle<()> }`
3. **Wire from `cognicode-runtime::bootstrap`**: after constructing IngestController, register each workspace and start a watcher per workspace. Store `Vec<WatcherHandle>` in Runtime for graceful shutdown (new method `Runtime::shutdown` or Drop impl).
4. **Add tests:**
   - Unit: debouncer flattens 50 events within 50ms into 1 batch; ignore non-code extensions
   - Integration: write file in tempdir → assert `run_scan` invoked within 1s → assert `GraphCache::subscribe()` receives `GraphEvent::GraphModified`
5. **Defer PG NOTIFY/LISTEN** — run_scan already broadcasts via `apply_events`; mark as separate kernel change (ADR-022 follow-up).

**Effort: M (1-2 days)** with Option A. Half a day less with Option B (no new dep).

## Ready For Proposal

**Yes — with one blocking question.**

The watcher has partial implementation but is dead code. The integration path is
clear (`IngestController` + `cognicode-runtime::bootstrap`). Recommend proceeding
to `sddk-propose` to draft spec/design/tasks for:

- Spec: per-workspace watcher lifecycle, debounce window (500ms default), incremental
  scan fast-path, graceful shutdown, multi-workspace semantics
- Design: `WatcherHandle` struct, `run_scan_paths` helper, runtime registration
- Tasks: (1) add `notify-debouncer-full` (or fix raw `notify` debounce), (2) extend
  IngestController, (3) runtime wiring + Drop, (4) integration test, (5) docs

**Open question for user before proposal:**

> The roadmap (`docs/ingest-pipeline-roadmap.md:273`) says use `notify-debouncer-full`,
> but the workspace only has raw `notify` as a dep and the team shipped a hand-rolled
> debouncer. Do you prefer **(A)** adopt `notify-debouncer-full` 0.4 (battle-tested,
> one new dep) or **(B)** keep raw `notify` and harden the existing debouncer in
> `watcher.rs` (no new dep, more code to maintain)?
