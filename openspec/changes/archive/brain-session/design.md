# Design: Brain Session — Conversational Graph Exploration

## Technical Approach

Create a `session/` submodule under `cognicode-explorer` with three types: `BrainSessionState` (data), `BrainSessionService` (logic + state), and `SessionRegistry` (Arc<Mutex<HashMap>> store). The handler gains a `SessionRegistry` field. Six new dispatch arms delegate to `SessionRegistry` methods. `brain_ask` prepends the focus node as a backtick-quoted token, then calls the existing `dispatch_ask` — the ask-router is unchanged except that `dispatch_ask` gains an optional `session` param for future follow-up emission (unused in Phase 2).

## Architecture Decisions

### Decision: Session Service Layer

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Stateful handler extension | Minimal new types, but handler grows to ~3700 LOC | Rejected |
| **Session Service Layer** | 4 new files, ~800 LOC, testable independently | **Chosen** |
| Reuse WorkspaceSession | Different concern domain, tight coupling | Rejected |

**Rationale**: Follows the existing `ask/` module precedent — domain logic isolated in its own submodule, wired into `mcp.rs` dispatch. Each session tool returns `McpResultEnvelope<T>` via the existing `ok_envelope_inner`.

### Decision: Lock Protocol — lookup → clone Arc → release → await

**Choice**: `Arc<Mutex<HashMap<SessionId, Arc<BrainSessionService>>>>`
**Alternatives**: `RwLock` (rejected — read/write asymmetry not justified for Phase 2 scale), `DashMap` (rejected — adds dependency, overkill for ~10 concurrent sessions).
**Rationale**: Spec mandates lock held ONLY for lookup/insert/remove/push — never across `.await`. The `Arc<BrainSessionService>` is cloned out under the lock, then the lock is dropped before any async work. Matches `ExplorerService.paths` pattern already in the codebase.

### Decision: Focus Injection — by brain_ask dispatch arm, NOT dispatch_ask

**Choice**: `brain_ask` prepends `` `focus_node` `` to the question string before calling `AskRouter::classify`.
**Alternatives**: Inject inside `dispatch_ask` (rejected — spec explicitly forbids; dispatcher's job is unchanged).
**Rationale**: Spec §Focus injection states: "brain_ask prepends focus_node as backtick-quoted token to question; dispatcher sees enriched question." The inner envelope's `provenance.source` stays `"ask-router"`.

### Decision: Lazy TTL Eviction — on open and attach only

**Choice**: `SessionRegistry::evict_expired()` called at the top of `brain_open` and `brain_attach`.
**Alternatives**: Background tokio task (rejected — out of scope for Phase 2).
**Rationale**: Spec §TTL: "No background task." `ttl = 0` disables expiry.

## Data Flow

```
Agent                    ExplorerMcpHandler            SessionRegistry          BrainSessionService       dispatch_ask
  │                            │                            │                         │                       │
  ├─ tools/call brain_open ──→ ├─ dispatch() match arm ───→ ├─ evict_expired()       │                       │
  │                            │                            ├─ insert(UUID, Arc<Svc>) │                       │
  │                            │←─ envelope_ok(session_id) ─┤                         │                       │
  │                            │                            │                         │                       │
  ├─ tools/call brain_ask ───→ ├─ dispatch() match arm ───→ ├─ lock → clone Arc ────→ │                       │
  │                            │                            ├─ release lock           │                       │
  │                            │                            │                         ├─ prepend focus_node ─→ │
  │                            │                            │                         │←─ McpResultEnvelope ──┤
  │                            │                            ├─ lock → push history ──→ │                       │
  │                            │                            ├─ release lock           │                       │
  │                            │←─ envelope_ok(result) ─────┤                         │                       │
```

## File Changes

| File | Action | Lines (est.) | Description |
|------|--------|-------------|-------------|
| `crates/cognicode-explorer/src/session/mod.rs` | Create | ~15 | `pub mod state; pub mod service; pub mod registry;` + re-exports |
| `crates/cognicode-explorer/src/session/state.rs` | Create | ~55 | `BrainSessionState`, `HistoryEntry`, serde derives, constants |
| `crates/cognicode-explorer/src/session/service.rs` | Create | ~180 | `BrainSessionService` — construction, `ask_with_session`, `set_focus`, `get_status`, history push logic |
| `crates/cognicode-explorer/src/session/registry.rs` | Create | ~120 | `SessionRegistry` — `Arc<Mutex<HashMap>>`, open/attach/close/evict/get |
| `crates/cognicode-explorer/src/lib.rs` | Modify | +1 | Add `pub mod session;` after `pub mod ask;` |
| `crates/cognicode-explorer/src/mcp.rs` | Modify | +~350 | 6 tool constants, 6 arg structs, 6 dispatch arms, 6 tool schemas, handler gains `registry` field, `TOOL_NAMES` 18→24 |
| `crates/cognicode-explorer/src/ask/dispatch.rs` | Modify | +8 | `dispatch_ask` gains `session: Option<&BrainSessionService>` (unused in Phase 2, reserved for follow-up emission) |

**Total**: ~4 new files (~370 LOC), 3 modified files (~360 LOC added). Net ~730 LOC.

## Interfaces / Contracts

### BrainSessionState (session/state.rs)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

pub const DEFAULT_HISTORY_CAP: usize = 50;
pub const DEFAULT_TTL_SECS: u64 = 1800;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainSessionState {
    pub session_id: String,           // UUIDv4
    pub workspace_id: String,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub ttl_secs: u64,                // 0 = no expiry
    pub focus_node: Option<String>,
    pub history: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub question: String,
    pub answer_summary: String,       // truncated to 200 chars
    pub pattern_id: u8,               // QuestionCategory discriminant
    pub ts: DateTime<Utc>,
}
```

### BrainSessionService (session/service.rs)

```rust
pub struct BrainSessionService {
    state: Mutex<BrainSessionState>,  // interior mutability for history/focus
    service: Arc<ExplorerService>,
    graph: Option<Arc<CallGraph>>,
}

impl BrainSessionService {
    pub fn new(session_id: String, workspace_id: String,
               ttl_secs: u64, service: Arc<ExplorerService>,
               graph: Option<Arc<CallGraph>>) -> Self;
    pub fn set_focus(&self, node: Option<String>);
    pub fn focus_node(&self) -> Option<String>;
    pub fn state_snapshot(&self) -> BrainSessionState;
    pub async fn ask_with_session(&self, question: &str)
        -> McpResultEnvelope<serde_json::Value>;
}
```

### SessionRegistry (session/registry.rs)

```rust
pub struct SessionRegistry {
    sessions: Arc<Mutex<HashMap<String, Arc<BrainSessionService>>>>,
}

impl SessionRegistry {
    pub fn new() -> Self;
    pub fn open(&self, workspace_id: String, ttl_secs: u64,
                service: Arc<ExplorerService>,
                graph: Option<Arc<CallGraph>>) -> String;
    pub fn attach(&self, session_id: &str)
        -> Result<Arc<BrainSessionService>, SessionError>;
    pub fn get(&self, session_id: &str)
        -> Result<Arc<BrainSessionService>, SessionError>;
    pub fn close(&self, session_id: &str) -> bool; // idempotent
    pub fn evict_expired(&self);
}
```

### ExplorerMcpHandler change (mcp.rs)

```rust
pub struct ExplorerMcpHandler {
    service: Arc<ExplorerService>,
    graph: Option<Arc<CallGraph>>,
    registry: SessionRegistry,  // NEW — always present, zero-cost when unused
}
```

### 6 New Tool Constants (mcp.rs)

```rust
pub const TOOL_BRAIN_OPEN: &str = "brain_open";
pub const TOOL_BRAIN_ATTACH: &str = "brain_attach";
pub const TOOL_BRAIN_ASK: &str = "brain_ask";
pub const TOOL_BRAIN_FOCUS: &str = "brain_focus";
pub const TOOL_BRAIN_STATUS: &str = "brain_status";
pub const TOOL_BRAIN_CLOSE: &str = "brain_close";
```

### 6 New Arg Structs (mcp.rs)

```rust
struct BrainOpenArgs { workspace_id: Option<String>, ttl: Option<u64> }
struct BrainAttachArgs { session_id: Option<String> }
struct BrainAskArgs { session_id: Option<String>, question: Option<String> }
struct BrainFocusArgs { session_id: Option<String>, focus_node: Option<String> }
struct BrainStatusArgs { session_id: Option<String> }
struct BrainCloseArgs { session_id: Option<String> }
```

### dispatch_ask signature change (ask/dispatch.rs)

```rust
// Before:
pub async fn dispatch_ask(
    classified: ClassifiedQuestion,
    service: &Arc<ExplorerService>,
    graph: &Option<Arc<CallGraph>>,
) -> McpResultEnvelope<Value>;

// After:
pub async fn dispatch_ask(
    classified: ClassifiedQuestion,
    service: &Arc<ExplorerService>,
    graph: &Option<Arc<CallGraph>>,
    _session: Option<&BrainSessionService>,  // NEW, unused in Phase 2
) -> McpResultEnvelope<Value>;
```

All existing call sites pass `None` — zero behavioral change.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `BrainSessionState` construction, serde roundtrip | Plain `#[test]`, no service needed |
| Unit | `SessionRegistry` open/attach/close/evict | `#[test]` with mock service (tempdir + NoopRepo) |
| Unit | `SessionRegistry` lock protocol — lock not held across await | `#[tokio::test]` verifying concurrent access |
| Unit | `BrainSessionService::ask_with_session` — focus prepend | Mock-based, assert enriched question string |
| Unit | History cap 50 FIFO — push 55 entries, assert first 5 gone | Pure state test |
| Unit | TTL eviction — expired sessions removed on open/attach | Time-manipulated state test |
| Unit | `brain_close` idempotent — unknown id returns `closed: false` | Registry test |
| Integration | 6 tool dispatch via `dispatch()` | Existing `call_tool_args()` pattern |
| Contract | TOOL_NAMES.len() == 24 | Regression guard (replaces current `==18` test) |
| Contract | 18 existing tools unchanged | Run full existing test suite |
| TDD RED | 6+ tests referencing undefined `session::` types/constants | Must fail before implementation |

### TDD RED Gate Sequence

1. Add `pub mod session;` to `lib.rs` → compile error (module missing)
2. Create `session/mod.rs` with empty re-exports → compile error (types missing)
3. Write tests referencing `SessionRegistry`, `BrainSessionService`, `TOOL_BRAIN_*` → all fail
4. Add 6 tool constants to `mcp.rs` → `TOOL_NAMES` assertion (len==24) fails
5. Add `registry` field to handler → constructor tests fail
6. Implement types and dispatch arms → tests turn GREEN one by one

## Migration / Rollback

No migration required. All changes are additive:
- 6 new tools are opt-in — existing 18-tool consumers are unaffected.
- `dispatch_ask` gains an optional param with default `None` — existing call sites pass `None`.
- Rollback: remove `pub mod session;` from `lib.rs`, revert `mcp.rs`, delete `session/` directory.

## Open Questions

None — all decisions resolved by spec contracts.
