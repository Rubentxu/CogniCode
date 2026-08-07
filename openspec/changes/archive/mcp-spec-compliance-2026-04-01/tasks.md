# Tasks: MCP Specification Full Compliance

## Phase 1: Foundation

- [x] 1.1 [src/interface/mcp/state.rs] Create `ServerState` enum (Uninitialized, Initializing, Ready) with `RwLock<ServerState>` wrapper
- [x] 1.2 [src/interface/mcp/state.rs] Implement state transitions: `try_transition()` validates valid transitions, returns error on invalid
- [x] 1.3 [src/interface/mcp/progress.rs] Create `ProgressTracker` struct: extract `_meta.progressToken` from params, emit `notifications/progress` to stdout
- [x] 1.4 [src/interface/mcp/progress.rs] Implement `emit_start()`, `emit_progress()`, `emit_complete()` methods
- [x] 1.5 [src/interface/mcp/mod.rs] Export new `state` and `progress` modules

## Phase 2: Core Server Refactor

- [x] 2.1 [src/interface/mcp/server.rs] Create `ToolRegistry` struct: `HashMap<&'static str, ToolHandlerFn>` with `register()` and `dispatch()` methods
- [x] 2.2 [src/interface/mcp/server.rs] Move all 27 tool handlers from match arms into `ToolRegistry::register_tools()`
- [x] 2.3 [src/interface/mcp/server.rs] Refactor `initialize`: parse `clientInfo`, `protocolVersion`, `capabilities` from params; store in HandlerContext; return server caps with `tools: {}`, `logging: {}`
- [x] 2.4 [src/interface/mcp/server.rs] Add `notifications/initialized` handler: transition state to Ready, no response (notification)
- [x] 2.5 [src/interface/mcp/server.rs] Add `ping` handler: return `{}` in any state (even before init)
- [x] 2.6 [src/interface/mcp/server.rs] Add state guard: reject all requests except `initialize`/`ping` with error -32002 before Ready state
- [x] 2.7 [src/interface/mcp/server.rs] Fix `format_tool_response()`: tool errors return `{content: [{type:"text",text:e}], isError:true}` not JSON-RPC error

## Phase 3: Advanced Features

- [x] 3.1 [src/interface/mcp/server.rs] Add `logging/setLevel` handler: parse `level` param, rebuild `EnvFilter`, return `{}`
- [x] 3.2 [src/interface/mcp/server.rs] Add `notifications/message` emission: write JSON to stdout for log messages at configured level
- [x] 3.3 [src/interface/mcp/server.rs] Wire `ProgressTracker` into `tools/call`: extract token, create tracker, pass to handler
- [x] 3.4 [src/interface/mcp/handlers.rs] Add `cancellation_token()` to `HandlerContext`: return `Arc<AtomicBool>` (no new deps)
- [x] 3.5 [src/interface/mcp/handlers.rs] Add cancellation checks in `handle_build_graph`, `handle_build_lightweight_index`, `handle_build_call_subgraph` between phases
- [x] 3.6 [src/interface/mcp/server.rs] Add `notifications/cancelled` handler: set `AtomicBool` on matching request id

## Phase 4: CLI

- [x] 4.1 [src/bin/mcp_server.rs] Add `--cwd <path>` arg via clap, validate path exists on startup, pass to `run_server()`
- [x] 4.2 [src/interface/mcp/server.rs] Accept `project_root: PathBuf` param in `run_server()` instead of hardcoding `"."`

## Phase 5: Testing

- [x] 5.1 [src/interface/mcp/state.rs] Unit tests: state transitions (valid + invalid), double-init rejection
- [x] 5.2 [src/interface/mcp/progress.rs] Unit tests: token extraction (string/int/None), notification format
- [x] 5.3 [src/interface/mcp/server.rs] Unit tests: ToolRegistry dispatch, ping, initialize caps format, format_tool_response isError
- [x] 5.4 [src/interface/mcp/server.rs] Integration test: initialize -> notifications/initialized -> tools/list -> tools/call -> ping sequence
- [x] 5.5 [src/interface/mcp/server.rs] Integration test: tools/call before initialized returns -32002
- [x] 5.6 Run `cargo test --lib` and verify all 465+ tests pass within ~35s
- [x] 5.7 Run `cargo clippy` to verify no new warnings
