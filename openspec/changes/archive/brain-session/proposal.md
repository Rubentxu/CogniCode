# Proposal: Brain Session — Conversational Graph Exploration

## Intent

Enable multi-turn conversational exploration of the call graph through a persistent session lifecycle. The roadmap Phase 2 outcome is: "A long-lived `brain_session` tool exists, so an agent can open a brain, attach to it, ask several questions, and close it cleanly." Agents currently must re-resolve entities on every one-shot tool call; sessions carry context (focus node, ask history) across turns.

## Scope

### In Scope
- 6 new MCP tools: `brain_open`, `brain_attach`, `brain_ask`, `brain_focus`, `brain_status`, `brain_close`
- `crates/cognicode-explorer/src/session/` module with `BrainSessionService` + `SessionRegistry`
- `BrainSessionState`: session_id, workspace_id, created_at, last_activity, ttl, focus_node, history (capped FIFO, default 50)
- In-memory `Arc<Mutex<HashMap<SessionId, BrainSessionService>>>` registry (Phase 2)
- Lazy TTL expiry on `brain_open`/`brain_attach`
- `brain_ask` enrichment: prepend focus-node entity token to question, append to history
- TDD RED gate: 6+ failing tests before implementation

### Out of Scope
- PostgreSQL persistence (Phase 3), `WorkspaceSession` reuse (rejected), UI/dashboard, background TTL task

## Capabilities

### New Capabilities
- `brain-session`: Session lifecycle (open/attach/ask/focus/status/close), 6 MCP tools, in-memory registry, session state model, lazy TTL, capped history

### Modified Capabilities
- `ask-router`: `dispatch_ask` extended to accept optional `&BrainSessionService`; `AskArgs.context` becomes `session_id` injection point; session-aware follow-ups emitted

## Approach

**Session Service Layer** (exploration recommendation).

New module `crates/cognicode-explorer/src/session/`:
- `BrainSessionService` — owns `Arc<ExplorerService>`, optional `Arc<CallGraph>`, session state
- `SessionRegistry` — `Arc<Mutex<HashMap<SessionId, BrainSessionService>>>`

Handler holds `SessionRegistry`; dispatch arms delegate. Existing 18 one-shot tools unchanged — sessions opt-in. `brain_ask` wraps `cognicode_ask` internally (no MCP re-serialization). Focus node injected as backtick-quoted entity token, not free-form text. Mutex held only for lookup + history push; `Arc<BrainSessionService>` cloned out before ask dispatch. All tools return `McpResultEnvelope<T>` with `source = "brain-session"`.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/session/` | New | BrainSessionService, SessionRegistry, state |
| `crates/cognicode-explorer/src/mcp.rs` | Modified | 6 tool constants, arg structs, dispatch arms, schemas |
| `crates/cognicode-explorer/src/ask/dispatch.rs` | Modified | `dispatch_ask` gains optional `&BrainSessionService` param |
| `crates/cognicode-explorer/src/lib.rs` | Modified | `pub mod session;` |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Mutex contention on concurrent session access | Low | Lock held only for lookup + push (<1ms); `Arc` cloned out before ask |
| Memory bloat from unbounded history | Low | Capped at N records (default 50), FIFO eviction |
| Wire-level breakage for agents hardcoding 18-tool list | Low | Additive change; roadmap says "opt-in" |
| Focus-node injection confuses regex patterns | Medium | Injected as backtick-quoted entity token, not free-form |

## Rollback Plan

Remove `pub mod session;` from `lib.rs`, revert `mcp.rs` to pre-change state, delete `crates/cognicode-explorer/src/session/`. All session tools are additive — removing them restores the 18-tool surface with zero side effects.

## Dependencies

- `ask-router` — `brain_ask` wraps `cognicode_ask` internally

## Success Criteria

- [ ] 6 MCP tools registered in `tools/list` and callable via `tools/call`
- [ ] `brain_ask` enriches question with focus-node context and appends to history
- [ ] Lazy TTL evicts expired sessions on `brain_open`/`brain_attach`
- [ ] All 18 existing tools pass unchanged test suite
- [ ] 6+ RED tests fail before implementation, pass after
