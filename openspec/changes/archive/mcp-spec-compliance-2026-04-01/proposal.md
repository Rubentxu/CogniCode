# Proposal: MCP Specification Full Compliance (2025-03-26)

## Intent

CogniCode's MCP server (`cognicode-mcp`) currently implements a partial, custom JSON-RPC dialect that fails capability negotiation with standard MCP clients (Claude Desktop, Cursor, Windsurf, etc.). This change brings it to full compliance with the MCP specification so it works as a drop-in MCP server.

## Scope

### In Scope

1. **Protocol version negotiation** — parse client `protocolVersion`, respond with negotiated version
2. **Capability negotiation** — parse `clientInfo` + `clientCapabilities`, declare `serverCapabilities` (tools, logging)
3. **Lifecycle** — enforce `initialize` -> `notifications/initialized` -> ready state machine; reject requests before initialized
4. **`ping/pong`** — required by spec for liveness checks
5. **`notifications/initialized`** — accept from client after `initialize` response
6. **`notifications/cancelled`** — support cancellation of in-flight tool calls
7. **`logging/setLevel`** — allow clients to control log verbosity
8. **`notifications/message`** — send structured log messages to client via stderr-safe notifications
9. **Progress tokens** — extract `_meta.progressToken` from tool calls, emit `notifications/progress`
10. **CLI argument** — accept `--cwd <path>` or first positional arg for project root directory
11. **Fix `tools/call` params format** — current format uses nested `{name, arguments}` wrapper; spec requires `params` directly as `{name, arguments}`
12. **Fix `tools/call` response format** — ensure `content` array uses spec-compliant `{type, text}` objects with `isError` support
13. **Fix `tools/list` response format** — ensure array of `{name, description, inputSchema}` per spec

### Out of Scope

- Streamable HTTP transport (only stdio needed for current use case)
- `resources/*` endpoints (not needed for code analysis tools)
- `prompts/*` endpoints (not needed)
- `sampling/createMessage` (server-initiated LLM calls — not needed)
- `completions/complete` (autocomplete suggestions — not needed)
- Elicitation (new 2025-11-25 feature — not needed)
- Tasks (new 2025-11-25 feature — not needed)

## Capabilities

### New Capabilities

- `mcp-lifecycle`: Initialize handshake, capability negotiation, state machine (uninitialized -> initializing -> initialized -> ready), `notifications/initialized`, `ping/pong`
- `mcp-logging`: `logging/setLevel` request handler, `notifications/message` emission to client
- `mcp-progress`: Progress token extraction from `_meta`, `notifications/progress` emission for long-running tool calls
- `mcp-cancellation`: `notifications/cancelled` handling, cooperative cancellation via `CancellationToken`

### Modified Capabilities

- `mcp-tools`: Fix `tools/call` params format (flat `name`+`arguments`), fix response `content` array format with `isError` field, fix `tools/list` schema compliance

## Approach

1. **MCP protocol layer** (`src/interface/mcp/protocol.rs`) — replace ad-hoc JSON parsing with typed MCP protocol structs (`InitializeRequest`, `InitializeResponse`, `ToolsCallRequest`, etc.) matching the spec schema
2. **State machine** — add `ServerState` enum to `HandlerContext`; gate all tool calls behind `Ready` state
3. **Handler registry** — refactor `handle_tools_call` match arm into a `ToolRegistry` that maps tool names to handler functions with schema introspection
4. **Progress** — wrap long-running handlers (`build_graph`, `build_lightweight_index`, `build_call_subgraph`) with progress emission via `_meta.progressToken`
5. **CLI** — add `clap` args to `mcp_server.rs` binary for `--cwd`

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/interface/mcp/server.rs` | Modified | State machine, lifecycle enforcement, `ping`, `notifications/initialized` |
| `src/interface/mcp/protocol.rs` | Modified | New typed MCP request/response structs |
| `src/interface/mcp/schemas.rs` | Modified | Fix `tools/list` and `tools/call` I/O formats |
| `src/interface/mcp/handlers.rs` | Modified | Accept progress token, emit progress notifications |
| `src/interface/mcp/mod.rs` | Modified | Export new types |
| `src/bin/mcp_server.rs` | Modified | Add `--cwd` CLI argument |
| `src/interface/mcp/progress.rs` | New | Progress token extraction + notification emission |
| `src/interface/mcp/state.rs` | New | Server state machine |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Breaking existing clients that use current format | Low | Current format is non-standard; no known external clients |
| Progress overhead on fast tool calls | Low | Only emit progress for calls with `_meta.progressToken` present |
| State machine rejects valid requests during init race | Medium | Use `tokio::sync::RwLock` for state; ensure `notifications/initialized` arrives before tool calls |

## Rollback Plan

The current server.rs is self-contained. Revert by restoring `server.rs`, `schemas.rs`, `protocol.rs`, `handlers.rs`, and `mcp_server.rs` from git. Since no git repo exists, keep a backup of current files before starting.

## Dependencies

- Existing `tokio`, `serde`, `serde_json`, `clap` dependencies (already in Cargo.toml)
- No new crate dependencies needed

## Success Criteria

- [ ] `cognicode-mcp` passes MCP Inspector (https://modelcontextprotocol.io/inspector) validation
- [ ] Works with Claude Desktop MCP config (`"command": "cognicode-mcp", "args": ["--cwd", "/path/to/project"]`)
- [ ] `initialize` -> `notifications/initialized` -> `tools/list` -> `tools/call` flow works end-to-end
- [ ] `ping` returns empty result
- [ ] `logging/setLevel` changes log verbosity
- [ ] Long-running tools emit `notifications/progress` when `progressToken` provided
- [ ] `notifications/cancelled` aborts in-flight tool calls
- [ ] All 465 existing tests still pass
- [ ] New MCP protocol tests added (lifecycle, state machine, format compliance)
