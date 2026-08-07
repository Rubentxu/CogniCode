# Design: MCP Specification Full Compliance

## Technical Approach

Implement 5 MCP capabilities on top of the existing handler infrastructure. Replace the stateless request dispatcher with a typed state machine, handler registry, and structured notification system. All changes are additive; existing handler signatures remain unchanged except where required for cancellation.

**Capabilities:**
- `mcp-lifecycle`: `ServerState` enum + `RwLock` in `HandlerContext`; `initialize` parses client caps, `ping` works in any state, `notifications/initialized` transitions to Ready
- `mcp-logging`: `logging/setLevel` handler + dynamic tracing filter; `notifications/message` emitted to stdout
- `mcp-progress`: Extract `_meta.progressToken` from tool call params; emit `notifications/progress` for 6 long-running tools
- `mcp-cancellation`: `CancellationToken` passed via `Arc<...>` in context; `notifications/cancelled` aborts in-flight requests
- `mcp-tools`: Fix `format_tool_response` to return `isError: true` for tool errors (not JSON-RPC errors); `tools/list` schema compliance unchanged (already correct)

## Architecture Decisions

| Decision | Choice | Alternatives | Rationale |
|----------|--------|--------------|-----------|
| State machine location | New `state.rs` | Keep in `server.rs` | Single responsibility; server.rs is already 1061 lines |
| Cancellation propagation | `Arc<tokio_util::sync::CancellationToken>` in `HandlerContext` | Add param to every handler | Zero handler signature changes; handlers opt-in via `ctx.cancellation_token()` |
| Handler dispatch | `ToolRegistry` struct with `HashMap<&str, HandlerFn>` | 900-line `match` arm | Reduces cyclomatic complexity; each handler registered once |
| Progress token storage | `Arc<OnceCell<ProgressToken>>` in `HandlerContext` | Thread-local / request-local | Fits existing `Arc<...>` pattern in `HandlerContext` |
| Logging level control | Dynamic `tracing_subscriber::EnvFilter` via `set_level` | Pre-built filter | Only way to change level at runtime without rebuilding subscriber |

## Data Flow

```
stdin JSON line
    │
    ▼
handle_request(line, &mut ctx)
    │
    ├─► StateGuard (check ServerState)
    │       ├─ Uninitialized/Initializing ──► reject (-32002) unless ping/initialize
    │       └─ Ready ──► proceed
    │
    ├─► MethodRouter
    │       ├─ initialize ──► parse caps, set state=Initializing, return caps
    │       ├─ notifications/initialized ──► set state=Ready, no response
    │       ├─ ping ──► return {}
    │       ├─ logging/setLevel ──► update EnvFilter
    │       ├─ notifications/cancelled ──► set cancel flag
    │       ├─ tools/list ──► return registry schemas
    │       └─ tools/call ──► ToolRegistry.dispatch(name, args, ctx)
    │
    └─► stdout JSON line
            │
            ├─► Response OR
            └─► notifications/progress (穿插 during long ops)
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/interface/mcp/state.rs` | Create | `ServerState` enum + `ServerStateGuard` for state transitions |
| `src/interface/mcp/progress.rs` | Create | `ProgressTracker` for `_meta.progressToken` extraction + `notifications/progress` emission |
| `src/interface/mcp/mod.rs` | Modify | Export `state`, `progress` modules |
| `src/interface/mcp/server.rs` | Modify | Remove McpResponse impl block, replace match with `MethodRouter` + `ToolRegistry`, add `handle_ping`, `handle_logging_set_level`, `handle_cancelled` |
| `src/interface/mcp/handlers.rs` | Modify | Add `cancellation_token()` method to `HandlerContext`; long-running handlers check token between phases |
| `src/bin/mcp_server.rs` | Modify | Add `clap` args for `--cwd`, parse before `run_server` |

## Interfaces / Contracts

```rust
// src/interface/mcp/state.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState { Uninitialized, Initializing, Ready }

impl HandlerContext {
    pub async fn state(&self) -> tokio::sync::RwLockReadGuard<ServerState>;
    async fn transition_to(&mut self, new_state: ServerState) -> Result<(), McpError>;
}

// src/interface/mcp/progress.rs
pub struct ProgressTracker {
    token: Option<ProgressTokenValue>, // String or i64
}

impl ProgressTracker {
    pub fn from_params(params: &serde_json::Value) -> Self;
    pub async fn emit_start(&self, ctx: &mut HandlerContext, message: &str);
    pub async fn emit_progress(&self, ctx: &mut HandlerContext, current: u64, total: u64, message: &str);
    pub async fn emit_complete(&self, ctx: &mut HandlerContext);
}

// src/interface/mcp/handlers.rs
impl HandlerContext {
    pub fn cancellation_token(&self) -> Arc<tokio_util::sync::CancellationToken>;
}

// McpResponse::format_tool_response (modified error path)
Err(e) => McpResponse::success(
    serde_json::json!({"content": [{"type": "text", "text": e.to_string()}], "isError": true}),
    id
)
```

## Testing Strategy

| Layer | What | How |
|-------|------|-----|
| Unit | State transitions | Test `ServerState` transitions: Uninitialized→Initializing→Ready; invalid transitions rejected |
| Unit | `format_tool_response` error path | Verify `isError: true` in result for `HandlerError`, not JSON-RPC error |
| Unit | `ProgressTracker` token extraction | Unit test: `from_params` with string/int/None `_meta.progressToken` |
| Unit | `logging/setLevel` level mapping | Test MCP level → tracing `Level` mapping for all 8 levels |
| Integration | Lifecycle flow | `initialize` → `notifications/initialized` → `tools/list` → `ping` in any state |
| Integration | Cancellation | Fire long-running op, send `notifications/cancelled`, verify no response sent |
| Integration | Progress emission | Call `build_graph` with `_meta.progressToken`, verify `notifications/progress` on stdout |

## Open Questions

- [ ] **tokio-util dependency**: The proposal says no new deps, but `tokio_util::sync::CancellationToken` is the cleanest solution. Reject `tokio-util` and implement a custom `AtomicBool` wrapper instead?
- [ ] **Logging to stdout vs tracing**: `notifications/message` must go to stdout (per spec), but current code uses `tracing`. Should log notifications bypass `tracing` entirely and write directly to stdout, or should `tracing` be configured to output to stdout?
- [ ] **Progress for non-long-running tools**: Spec says fast tools SHOULD NOT emit progress even with token. Should we implement a no-op `ProgressTracker` that silently drops all emissions for non-long-running tools, or skip progress extraction entirely for non-long-running tool names?
