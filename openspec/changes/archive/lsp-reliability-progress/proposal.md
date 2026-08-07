# Proposal: LSP Reliability and Progress Reporting

## Intent

Improve user experience when working with LSP servers (especially rust-analyzer) that require significant indexing time. Currently, users/agents have no feedback during indexing, LSP requests fail with generic errors, and there's no way to track progress or cancel long operations. This creates a poor experience where the system appears "stuck" without any indication of what's happening.

## Scope

### In Scope
- **LSP Server Health Check** - Method to check if server is initialized AND ready (not just spawned)
- **Server Progress Tracking** - Implement LSP `workDoneProgress` protocol to track indexing/analysis progress
- **Readiness Wait with Timeout** - Ability to wait for server readiness with configurable timeout and progress callbacks
- **Structured Error Messages** - Distinguish server states: "starting", "indexing", "ready", "request timeout", "server crashed"
- **Graceful Degradation** - Fallback to tree-sitter with clear message when server doesn't become ready in time
- **Cancellation Support** - Ability to cancel long-running LSP requests via CancellationToken

### Out of Scope
- Progress UI in terminals (only programmatic callbacks)
- Multi-server progress aggregation
- Persistent progress state across restarts
- LSP client-side progress reporting (only server-side)

## Approach

1. **Extend `LspProcess`** with readiness state tracking and progress notification handling
2. **Create `LspServerState` enum** with clear states: `NotStarted`, `Initializing`, `Indexing { progress: f32 }`, `Ready`, `Failed { reason }`
3. **Implement `window/workDoneProgress`** handling in `JsonRpcTransport` to receive progress notifications
4. **Add `wait_for_ready()` method** to `LspProcessManager` with timeout and optional progress callback
5. **Extend `LspProcessError`** with structured error variants for each failure mode
6. **Add cancellation token support** to request methods for long-running operations
7. **Implement fallback logic** in providers to use tree-sitter when LSP is unavailable

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/infrastructure/lsp/process.rs` | Modified | Add state tracking, progress handling, cancellation |
| `src/infrastructure/lsp/process_manager.rs` | Modified | Add `wait_for_ready()`, improve error messages |
| `src/infrastructure/lsp/json_rpc.rs` | Modified | Handle notifications separately from responses |
| `src/infrastructure/lsp/client.rs` | Modified | Add `LspServerState` enum, structured errors |
| `src/infrastructure/lsp/providers/` | Modified | Add fallback to tree-sitter when LSP unavailable |
| `src/application/services/lsp_proxy_service.rs` | Modified | Use new readiness checks |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Not all LSP servers support `workDoneProgress` | Medium | Graceful fallback - just report "indexing" without percentage |
| Progress notifications format varies | Low | Use `lsp-types` crate for standard types, handle both string and object progress |
| Thread safety with progress callbacks | Low | Use `Arc<Mutex>` for callback state, already using tokio sync |
| Performance overhead from progress tracking | Low | Progress notifications are async, minimal overhead |

## Rollback Plan

1. All changes are additive - no breaking API changes
2. New methods (`wait_for_ready`, `get_state`) can be unused without affecting existing code
3. Progress tracking is optional - if callbacks are `None`, no tracking occurs
4. If issues arise, disable progress handling with feature flag or config option
5. Revert individual files - changes are isolated to LSP infrastructure layer

## Dependencies

- `lsp-types` crate already included (for `WorkDoneProgress` types)
- `tokio-util` for `CancellationToken` (add to Cargo.toml if not present)

## Success Criteria

- [ ] `LspProcessManager::wait_for_ready()` returns server state within timeout
- [ ] Progress callback receives indexing percentage (0-100) when server supports it
- [ ] Error messages clearly distinguish: "starting", "indexing", "ready", "timeout", "crashed"
- [ ] Request cancellation terminates pending LSP request within 1 second
- [ ] Fallback to tree-sitter occurs automatically after configurable timeout (default 60s)
- [ ] All existing LSP tests pass without modification
- [ ] New integration test validates progress reporting with rust-analyzer
