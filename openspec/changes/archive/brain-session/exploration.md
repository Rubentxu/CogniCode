# Exploration: brain-session

## Current State

The `ExplorerMcpHandler` at `crates/cognicode-explorer/src/mcp.rs` is a **stateless rmcp handler** holding two `Arc` handles:
- `Arc<ExplorerService>` — the domain service (symbol repo, spotter, views, lenses, MoldQL, exploration paths)
- `Option<Arc<CallGraph>>` — optional in-memory call graph (loaded at binary startup from SQLite or PostgreSQL)

The 18-tool dispatch in `call_tool` clones both Arcs per request — zero shared mutable state. The ask-router (`cognicode_ask`, tool #18) wraps the 17 primitives behind a single NL entry point. `AskArgs.context: Option<serde_json::Value>` is **explicitly reserved for "future use (routing hints, conversation state)"** — the ready injection point for session_id.

A separate `WorkspaceSession` (`cognicode-core/src/application/workspace_session.rs`) exists but wraps analysis/file-ops/LSP concerns, NOT the explorer graph surface. Two separate MCP binaries exist: `cognicode-explorer/src/bin/mcp.rs` (the explorer) and `cognicode-mcp/src/main.rs` (diagram-aware). The brain_session belongs in the explorer binary.

The product model (`docs/explorer-graph/target-product-model.md`) defines the "brain" as a **queryable model joining one or more spaces** with its own confidence model, UI, and metadata row. The roadmap Phase 2 outcome explicitly lists: "A long-lived `brain_session` tool exists, so an agent can open a brain, attach to it, ask several questions, and close it cleanly." The `core-mcp-boundaries.md` assigns "Session lifecycle (open brain, attach, detach)" to `cognicode-mcp` (the MCP surface crate).

## Affected Areas

- `crates/cognicode-explorer/src/mcp.rs` — add 6 new tool constants, arg structs, dispatch arms, and tool schemas; handler holds session registry
- `crates/cognicode-explorer/src/session/` (new module) — BrainSessionService, session state, expiry logic
- `crates/cognicode-explorer/src/ask/dispatch.rs` — `dispatch_ask` becomes session-aware (accepts optional session context for focus/follow-up enrichment)
- `crates/cognicode-explorer/src/lib.rs` — `pub mod session;`
- `crates/cognicode-explorer/src/bin/mcp.rs` — no changes (handler construction unchanged; sessions are opt-in runtime state)
- `docs/explorer-graph/` — no code changes; the exploration documents the product model alignment

## Approaches

### 1. Stateful Handler Extension (Moderate)
Extend `ExplorerMcpHandler` with `Arc<Mutex<HashMap<SessionId, SessionState>>>`. The 6 session tools mutate this map inside `call_tool`. `dispatch()` stays `&self`-based.

- **Pros**: Minimal new types, keeps everything in one file, follows `CogniCodeHandler` precedent (which holds `Arc<HandlerContext>`)
- **Cons**: Mutex contention under concurrent session access; handler grows from 3165 LOC to ~3700 LOC; harder to unit-test session logic independently

### 2. Session Service Layer (Cleaner — RECOMMENDED)
Create `crates/cognicode-explorer/src/session/` with:
- `BrainSessionService` — owns `Arc<ExplorerService>`, `Option<Arc<CallGraph>>`, session state (history, focus, created_at, ttl)
- `SessionRegistry` — `HashMap<SessionId, BrainSessionService>` behind `Arc<Mutex<>>`
- Handler holds `SessionRegistry`; dispatch arms delegate to it

- **Pros**: Clean separation of concerns, testable independently, follows existing `ask/` module precedent, handler stays backward-compatible (18 one-shot tools untouched)
- **Cons**: More new files (~4), ~800 LOC new code, indirection layer between handler and session

### 3. Reuse `WorkspaceSession` (Risky)
Wrap `cognicode_core::WorkspaceSession` with ask history/focus state.

- **Pros**: Leverages existing session pattern
- **Cons**: `WorkspaceSession` has a different concern domain (file ops, analysis, LSP); tight coupling between explorer and core; the session's async API (`ensure_graph_built()`, `build_lightweight_index()`) doesn't match the explorer's sync-in-dispatch model. **REJECTED**.

## Recommendation

**Approach 2: Session Service Layer.** 

Rationale:
1. The existing ask-router architecture (`ask/mod.rs`, `patterns.rs`, `dispatch.rs`, `entity.rs`, `followups.rs`) already demonstrates the module pattern — domain logic in its own crate submodule, wired into mcp.rs. The session module follows the same precedent.
2. `AskArgs.context` is the injection point for `session_id` — no wire-level breakage. The ask-router's `dispatch_ask` signature extends from `(ClassifiedQuestion, &Arc<ExplorerService>, &Option<Arc<CallGraph>>)` to additionally accept `Option<&BrainSessionService>`.
3. The existing `ExplorerMcpHandler` stays backward-compatible: all 18 one-shot tools work identically whether a session registry exists or not.
4. Each session tool returns the standard `McpResultEnvelope<T>` — provenance is set to `source = "brain-session"`, confidence tracks session state freshness.
5. Six tools follow the existing "one tool per verb" pattern:
   - `brain_open` → creates session, returns `session_id` + workspace summary
   - `brain_attach` → reconnects to existing session by `session_id` (or `workspace_path`)
   - `brain_ask` → routes through `cognicode_ask` with session context (focus node, history)
   - `brain_focus` → sets/gets focus node in session
   - `brain_status` → returns session metadata (uptime, question count, focus, ttl)
   - `brain_close` → removes session from registry, cleans up

## Key Design Decisions

### Session Storage: In-Memory Only (Phase 2)
Sessions live in `Arc<Mutex<HashMap<SessionId, BrainSessionService>>>` — process lifetime, dies with server restart. Matches existing `ExplorerService.paths` pattern (`Arc<Mutex<HashMap<String, ExplorationPath>>>`). PostgreSQL persistence comes in Phase 3.

### Session State per Instance
```rust
struct BrainSessionState {
    session_id: String,           // UUID v4
    workspace_id: String,         // from open_workspace
    created_at: DateTime<Utc>,   
    last_activity: DateTime<Utc>,
    ttl: Duration,                // configurable, default 30 min
    focus_node: Option<String>,   // current MVP id
    history: Vec<AskRecord>,      // (question, classified, primary_result) — capped at N
    ask_count: u32,
}

struct AskRecord {
    question: String,
    category: QuestionCategory,
    confidence: f64,
    timestamp: DateTime<Utc>,
}
```

### Ask Integration
`brain_ask` wraps `cognicode_ask`:
1. Parse `AskArgs` → extract `session_id` from `context`
2. Look up session in registry
3. Prepend focus_node context to question (e.g., "given current focus on `X`, what calls it?")
4. Call `AskRouter::classify()` + `dispatch_ask()` with enriched entities
5. Append to session history
6. Return standard envelope with brain-session provenance

### Session Expiry: Lazy TTL
A `SessionRegistry::cleanup_expired()` is called on every `brain_open` and `brain_attach`. No background task needed for Phase 2. Sessions with `last_activity + ttl < now()` are dropped. The `brain_status` tool reports remaining TTL.

### Envelope Interaction
Every brain_session tool returns `McpResultEnvelope<T>` with:
- `provenance.source = "brain-session"`
- `provenance.confidence` reflects session state freshness (1.0 for active, 0.8 for reattached, 0.5 for expired-but-reattached)
- `suggested_follow_ups` include session-aware hints (e.g., "would you like to focus on this node?")

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | `BrainSessionService` create/attach/ask/close | `#[tokio::test]` with `build_test_service()` |
| Unit | `SessionRegistry` expiry, cleanup, max capacity | Pure fn tests, no service needed |
| Unit | `brain_ask` session-aware dispatch | Mock `dispatch_ask` to verify session context enrichment |
| Integration | Full 6-tool surface via `dispatch()` | Existing `call_tool_args()` + `dispatch()` pattern |
| Contract | TDD RED gate: 6+ failing tests before implementation | Tests reference undefined constants/types |
| Contract | 18 existing tools unchanged | Run existing test suite at every step |

## Risks

- **Mutex Contention**: Multiple concurrent `brain_ask` calls on the same session will contend on the Mutex. Mitigation: `dispatch()` clones the `Arc<BrainSessionService>` out of the Mutex before calling ask, so the lock is held only for lookup + history push (sub-millisecond).
- **Wire-Level Breakage**: Adding 6 new tools is additive — but agents that hardcode the 18-tool list may break. Mitigation: the roadmap explicitly says "ship the session tools as opt-in and keep the existing one-shot tools working."
- **Session History Bloat**: Unbounded history eats memory. Mitigation: cap at N records (configurable, default 50), FIFO eviction.
- **ask-router Context Pollution**: Prepending focus_node to the question string may confuse regex patterns. Mitigation: inject focus_node as an additional entity token (backtick-quoted), not as free-form text.

## Ready for Proposal

Yes — the exploration is complete. The next phase should produce a proposal that defines the 6-tool surface, the BrainSessionService API contract, the session state model, and the TDD RED gate test set.
