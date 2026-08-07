# Proposal: LSP Reliability & Progress Reporting

## Intent

The current LSP integration fails silently when rust-analyzer/pyright are still indexing large projects. Users and AI agents get no feedback, making the tool appear broken. This change adds structured progress reporting, health checks, and graceful fallback so the system is reliable and transparent.

## Problem Statement

- rust-analyzer takes 30+ seconds to index large Rust projects
- No feedback during indexing — appears "stuck"
- Generic errors instead of "still indexing"
- No way to check if server is ready before sending requests
- AI agents can't distinguish "slow" from "broken"
- No cancellation for hung requests

## Scope

### In Scope
- **Server Health Check** — `is_ready()` method to check if server is initialized + indexed
- **Structured Status** — `ServerStatus` enum: `Starting | Indexing(progress%) | Ready | Busy | Crashed`
- **Wait-for-Ready** — `wait_for_ready(timeout, callback)` to block until server is ready
- **Progress Callbacks** — Report indexing progress via callback during long operations
- **Graceful Fallback** — If server not ready within timeout, use tree-sitter with clear message
- **Cancellation Support** — Ability to cancel long-running requests
- **Structured Errors** — `LspProcessError` with specific variants for each failure mode

### Out of Scope
- New LSP features (rename, completion)
- Multi-threaded process pools
- On-disk caching of index state

## Approach

### State Machine for Server Status

```
ServerStatus:
  Starting    → Initial process spawned, not yet initialized
  Indexing    → Initialized, server is building index (0-100%)
  Ready       → Server ready to accept requests
  Busy        → Server handling a request
  Crashed     → Server process died
```

### Key APIs

```rust
// LspProcessManager
pub async fn wait_for_ready(
    &self,
    language: Language,
    timeout_secs: u64,
    progress_callback: Option<Box<dyn Fn(ProgressUpdate) + Send>>,
) -> Result<ServerStatus, LspProcessError>

pub fn is_ready(&self, language: Language) -> bool

// Progress reporting
pub struct ProgressUpdate {
    pub message: String,
    pub percentage: Option<f32>,
    pub status: ServerStatus,
}

// Errors
pub enum LspProcessError {
    ServerNotReady { status: ServerStatus, waited_secs: u64 },
    ServerCrashed { reason: String, crash_count: u32 },
    RequestTimeout { method: String, waited_secs: u64 },
    Cancelled { method: String },
    // ... existing variants
}
```

### Fallback Strategy

1. Send request to LSP server
2. If server not ready → `wait_for_ready(timeout=30s)` with progress callback
3. If still not ready after timeout → log warning, use tree-sitter fallback, return result with `fallback_reason` field
4. If server crashes during request → record crash, retry once, then fallback

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/infrastructure/lsp/process.rs` | Modified | Add `ServerStatus`, `wait_for_ready()`, cancellation |
| `src/infrastructure/lsp/process_manager.rs` | Modified | Add health check, progress aggregation |
| `src/infrastructure/lsp/providers/composite.rs` | Modified | Use `wait_for_ready` before LSP calls, log fallback reason |
| `src/infrastructure/lsp/error.rs` | New | Unified `LspProcessError` with structured variants |
| `src/interface/cli/commands.rs` | Modified | Show progress during navigate commands |

## Dependencies

- tokio timeouts (already in deps)
- No new external crates

## Rollback Plan

Revert changes to `process.rs`, `process_manager.rs`, `composite.rs`. Delete `error.rs`. All changes are isolated to LSP infrastructure.

## Success Criteria

- [ ] `ServerStatus` enum tracks all states correctly
- [ ] `wait_for_ready` returns within timeout with progress callbacks
- [ ] Tree-sitter fallback used with clear message when timeout exceeded
- [ ] CLI shows progress during LSP operations
- [ ] `cargo test lsp` passes
- [ ] All 4 integration tests pass (including rust-analyzer hover)
