# Design: LSP Reliability & Progress Reporting

## Technical Approach

Add structured status tracking and progress reporting to the LSP layer. The existing `LspProcess` manages a single LSP process; we extend it with a `ServerStatus` state machine and `wait_for_ready()` that polls for readiness with progress callbacks. `LspProcessManager` becomes the orchestrator that aggregates status and provides health checks. `CompositeProvider` uses `wait_for_ready` before LSP calls, falling back to tree-sitter with clear messaging.

## Architecture Decisions

### Decision: ServerStatus as a simple enum, not a state machine trait

**Choice**: `ServerStatus` enum with explicit variants: `Starting`, `Indexing { progress: f32 }`, `Ready`, `Busy`, `Crashed { reason: String }`

**Alternatives considered**: State machine trait with transition validation, separate `HealthStatus` and `ReadinessStatus` enums

**Rationale**: Simpler to implement and test. Transitions are validated at call sites (e.g., only `Indexing` → `Ready`). LSP servers self-report progress via `$/progress` notifications; we map these to `Indexing` state.

### Decision: ProgressUpdate as a struct with an Option callback

**Choice**:
```rust
pub struct ProgressUpdate {
    pub message: String,
    pub percentage: Option<f32>,
    pub status: ServerStatus,
}

pub trait ProgressCallback: Send + Fn(ProgressUpdate) {}
```

**Alternatives considered**: Channel-based push to a `tokio::sync::mpsc::Receiver`, `tracing` spans with metrics

**Rationale**: Callback is simpler for the CLI use case (print to stdout). Channel adds complexity and requires lifecycle management. The callback can be `None` for fire-and-forget calls.

### Decision: wait_for_ready uses tokio::select with cancellation

**Choice**: Cooperative cancellation via `tokio::time::timeout` wrapped around a poll loop that checks server status every 500ms.

**Alternatives considered**: `tokio::sync::broadcast` channel for server-initiated status updates, polling `is_ready()` with `tokio::time::interval`

**Rationale**: Current `LspProcess` doesn't emit events — it would need significant refactoring to support push-based progress. Poll-based is pragmatic: we check `initialized` flag + LSP handshake completeness. On each poll we invoke the callback if provided.

### Decision: LspProcessError expanded with structured variants

**Choice**: Extend existing `LspProcessError` enum (in `error.rs` module):

```rust
pub enum LspProcessError {
    // ... existing variants ...
    ServerNotReady { status: ServerStatus, waited_secs: u64 },
    ServerCrashed { reason: String, crash_count: u32 },
    RequestTimeout { method: String, waited_secs: u64 },
    Cancelled { method: String },
}
```

**Alternatives considered**: New `LspError` type separate from `LspProcessError`, using `anyhow::Error`

**Rationale**: Existing code uses `LspProcessError` with `thiserror`. Extending it maintains compatibility. `anyhow` loses type specificity needed for error recovery logic.

## Data Flow

```
CLI Command (execute_navigate)
    │
    ▼
CompositeProvider::{get_definition, hover, find_references}
    │
    ├─[LSP path]─→ LspIntelligenceProvider
    │                    │
    │                    ▼
    │               LspProcessManager::wait_for_ready(timeout, callback)
    │                    │
    │                    ├─→ LspProcess::is_initialized() + capability check
    │                    │         │
    │                    │         └─[not ready]──→ poll every 500ms, invoke callback
    │                    │
    │                    ├─→ LSP $/progress notifications (future: parsed from transport)
    │                    │
    │                    └─[timeout or ready]──→ return ServerStatus
    │
    └─[fallback path]──→ TreesitterFallbackProvider
                              │
                              ▼
                         Result with fallback_reason
```

**Error flow**:
```
LspProcess request fails
    │
    ▼
LspProcessError::{Timeout, ServerCrashed, Transport}
    │
    ▼
LspProcessManager::request() catches crash, calls record_crash()
    │
    ▼
CompositeProvider catches error, logs warning, falls back to tree-sitter
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/infrastructure/lsp/error.rs` | Create | Unified `LspProcessError` with structured variants |
| `src/infrastructure/lsp/process.rs` | Modify | Add `ServerStatus` enum, `wait_for_ready()`, cancellation support |
| `src/infrastructure/lsp/process_manager.rs` | Modify | Add `is_ready()`, health check, progress aggregation |
| `src/infrastructure/lsp/providers/composite.rs` | Modify | Use `wait_for_ready` before LSP calls, pass fallback_reason |
| `src/interface/cli/commands.rs` | Modify | Progress display in `execute_navigate` |

## Interfaces / Contracts

### ServerStatus enum (process.rs)
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ServerStatus {
    Starting,
    Indexing { progress: f32 },  // 0.0 - 100.0
    Ready,
    Busy,
    Crashed { reason: String },
}
```

### ProgressUpdate struct (process.rs)
```rust
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub message: String,
    pub percentage: Option<f32>,
    pub status: ServerStatus,
}

pub trait ProgressCallback: Send + Fn(ProgressUpdate) + 'static {}
```

### wait_for_ready signature (LspProcessManager)
```rust
pub async fn wait_for_ready(
    &self,
    language: Language,
    timeout_secs: u64,
    progress_callback: Option<Box<dyn ProgressCallback>>,
) -> Result<ServerStatus, LspProcessError>
```

### Extended LspProcessError (error.rs)
```rust
pub enum LspProcessError {
    // ... existing (SpawnFailed, NotInitialized, Timeout, Transport, KillFailed, Io) ...
    ServerNotReady { status: ServerStatus, waited_secs: u64 },
    ServerCrashed { reason: String, crash_count: u32 },
    RequestTimeout { method: String, waited_secs: u64 },
    Cancelled { method: String },
}
```

### FallbackResult wrapper (composite.rs)
```rust
pub struct FallbackResult<T> {
    pub value: T,
    pub fallback_reason: Option<String>,
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `ServerStatus` transitions, `wait_for_ready` timeout/cancellation | Mock `LspProcess` in `process.rs` tests |
| Unit | `CrashRecord` and crash limit logic | Existing tests in `process_manager.rs` |
| Unit | `FallbackResult` with `fallback_reason` | Unit test in `composite.rs` |
| Integration | `wait_for_ready` with real (or mock) LSP transport | `tokio::test` with mock transport |
| Integration | CLI navigate commands with progress display | CLI integration test with `assert!(output.contains("indexing"))` |

## Migration / Rollout

No migration required. All changes are additive:
- New `error.rs` module is imported alongside existing `process.rs`
- `ServerStatus` is internal to `LspProcess`; no API surface change
- `CompositeProvider` behavior unchanged (fallback still works)
- CLI progress display is opt-in via callback

## Open Questions

- [ ] Should `$/progress` notifications from LSP be parsed in `JsonRpcTransport` and propagated via channel to `wait_for_ready`? Currently we only check `is_initialized` flag.
- [ ] Should there be a maximum crash count per session (reset on restart) vs. the current sliding window approach?
