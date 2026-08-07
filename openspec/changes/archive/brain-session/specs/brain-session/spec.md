# brain-session Specification (NEW)

## Purpose

Long-lived, conversational MCP surface for graph exploration. A "brain" is an in-memory session carrying context (focus node, ask history, TTL) across `tools/call` invocations, so an agent can open a brain, ask several related questions, and close it cleanly. Six new MCP tools (`brain_open`, `brain_attach`, `brain_ask`, `brain_focus`, `brain_status`, `brain_close`) join the 18-tool surface. Sessions are opt-in and additive — every one-shot tool keeps its current wire contract. Per-tool IO contracts live in `tools/brain_*.md`.

## Requirements

### Requirement: Session State Model

`BrainSessionState` MUST contain: `session_id: String` (UUIDv4), `workspace_id: String`, `created_at: i64` (epoch ms), `last_activity: i64` (epoch ms), `ttl: u64` (seconds, default 1800), `focus_node: Option<String>`, `history: Vec<HistoryEntry>` (capped FIFO, default 50). `HistoryEntry = { question, answer_summary, pattern_id: u8, ts: i64 }`. State MUST be private, exposed only via read-only accessors.

#### Scenario: Fresh state populated

- GIVEN `BrainSessionService::open(ws, ttl)` runs
- THEN `session_id` is a non-empty UUIDv4
- AND `created_at == last_activity`
- AND `focus_node` is `None`
- AND `history` is empty

#### Scenario: UUIDs unique per open

- GIVEN the same `workspace_id` is opened twice
- THEN the two `session_id` values differ

### Requirement: SessionRegistry and Concurrency

`SessionRegistry` MUST hold `Arc<Mutex<HashMap<SessionId, Arc<BrainSessionService>>>>`. API: `open(ws, ttl) -> SessionId`, `attach(id) -> Option<Arc<BrainSessionService>>`, `close(id) -> bool`, `evict_expired(now_ms) -> usize`. Locks MUST be held only for lookup/insert/remove/push — never across `await`. `Arc<BrainSessionService>` MUST be cloned out before any async dispatch.

#### Scenario: Attach returns a clone

- GIVEN a session exists
- WHEN `attach(id)` runs
- THEN the result is `Some(Arc<BrainSessionService>)`
- AND the registry `Mutex` is not held while methods on the `Arc` are invoked

#### Scenario: Unknown session returns None

- GIVEN no session with `id` exists
- WHEN `attach(id)` runs
- THEN it returns `None`

#### Scenario: Close removes and returns true

- GIVEN session with `id = S` exists
- WHEN `close(S)` runs
- THEN it returns `true`
- AND a subsequent `attach(S)` returns `None`

#### Scenario: Close unknown returns false

- GIVEN no session with `id = S`
- WHEN `close(S)` runs
- THEN it returns `false`

#### Scenario: Concurrent attach is safe

- GIVEN two threads call `attach(S)` simultaneously
- WHEN both return
- THEN both receive a valid `Arc<BrainSessionService>`
- AND no panic, deadlock, or data race occurs

### Requirement: Lazy TTL Expiry

Expired sessions MUST be evicted lazily on `open` and `attach` (no background task). On `open`, the registry MUST scan the map, remove entries where `now_ms - last_activity_ms >= ttl_seconds * 1000`, then insert the new session. On `attach`, `evict_expired` MUST run first, then the lookup. `last_activity` MUST be refreshed BEFORE business logic on `ask`/`focus`/`status`.

#### Scenario: Expired session evicted on open

- GIVEN `last_activity = 1000`, `ttl = 60` (60_000 ms), `now_ms = 70_000`
- WHEN `open(ws, ttl)` runs
- THEN the expired session is removed
- AND the new session is inserted

#### Scenario: Expired session rejected on attach

- GIVEN a session whose TTL has elapsed
- WHEN `attach(id)` runs
- THEN the session is evicted
- AND `attach` returns `None`

#### Scenario: last_activity refreshed on mutation

- GIVEN `last_activity = T0`
- WHEN any of `ask`/`focus`/`status` runs
- THEN `last_activity` is updated to current `now_ms`

#### Scenario: TTL = 0 disables expiry

- GIVEN a session opened with `ttl = 0`
- WHEN `evict_expired` runs repeatedly
- THEN the session is not evicted

### Requirement: Capped History (FIFO)

Every successful `brain_ask` MUST append a `HistoryEntry` and truncate oldest past the cap (default 50). Cap applied AFTER push. Failed asks (graph unavailable, no entity match) MUST NOT append.

#### Scenario: History grows to cap

- GIVEN fresh session, cap = 3
- WHEN three successful asks happen
- THEN `history.len() == 3`

#### Scenario: Cap evicts oldest

- GIVEN `history = [A,B,C]`, cap = 3
- WHEN a fourth successful ask appends `D`
- THEN `history == [B,C,D]`

#### Scenario: Failed ask does not append

- GIVEN `history = [A]`
- WHEN `brain_ask` fails
- THEN `history` remains `[A]`

#### Scenario: Default cap is 50

- GIVEN no explicit cap on open
- THEN the effective cap is `50`

### Requirement: Focus Node Management

Session holds `focus_node: Option<String>`. `brain_focus` sets/clears it. `brain_ask` injects it as a backtick-quoted token prepended to the question. If `focus_node = Some("AuthService")` and question = "what does it call?", the rewritten question MUST be `what does \`AuthService\` call?`. If `focus_node = None`, the question MUST be dispatched unchanged. Setting `node = ""` MUST be rejected. Setting `node = null` MUST clear the focus.

#### Scenario: Focus node prepended

- GIVEN `focus_node = Some("AuthService")`
- WHEN `brain_ask` is called with `"what does it call?"`
- THEN the dispatched question's first backtick token is `AuthService`

#### Scenario: No focus leaves question unchanged

- GIVEN `focus_node = None`
- WHEN `brain_ask` is called with `"what does \`foo\` call?"`
- THEN the dispatcher receives the question byte-for-byte

#### Scenario: Clearing focus is honored

- GIVEN `focus_node = Some("Foo")`
- WHEN `brain_focus(node = null)` runs
- THEN `focus_node` becomes `None`

#### Scenario: No duplicate injection

- GIVEN `focus_node = Some("AuthService")`
- WHEN `brain_ask` is called with `"what does \`AuthService\` depend on?"`
- THEN `AuthService` appears exactly once in the dispatched question

### Requirement: Tool Registration

Six new constants MUST join `TOOL_NAMES`: `brain_open`, `brain_attach`, `brain_ask`, `brain_focus`, `brain_status`, `brain_close`. `TOOL_NAMES.len()` grows from 18 to 24. `build_tool_schemas()` MUST emit a schema for each. Every `brain_*` response MUST carry `provenance.source = Some("brain-session")`.

#### Scenario: 6 new tools in tools/list

- GIVEN the MCP server is started
- WHEN a client calls `tools/list`
- THEN the response includes all 6 `brain_*` names
- AND `TOOL_NAMES.len() == 24`

#### Scenario: Existing 18 tools unchanged

- GIVEN the 18 prior tools are registered
- WHEN the 6 brain tools are added
- THEN the 18 prior names still appear in `TOOL_NAMES`
- AND their schemas are not modified

### Requirement: brain_open / attach / focus / status / close

Each tool MUST match the per-tool contract in `tools/brain_<name>.md` (input schema, output schema, error codes). The tools collectively MUST: open returns a fresh UUIDv4 session; attach rejects `session_not_found` and `session_expired`; focus rejects empty strings; status returns `history: []` for empty history (not `null`); close is idempotent and returns HTTP 200 with `closed: false` for unknown sessions (not an error envelope). All five refresh `last_activity` on success.

#### Scenario: Open returns valid session

- GIVEN `brain_open(workspace_id = "ws-1")` is called
- THEN `payload.session_id` is a non-empty UUIDv4
- AND `payload.workspace_id == "ws-1"`
- AND `payload.history_len == 0`

#### Scenario: Open with missing workspace_id rejected

- GIVEN `brain_open({})` is called
- THEN `error.code == "missing_required_arg"` naming `workspace_id`

#### Scenario: Opened session is attachable

- GIVEN `brain_open` returned `session_id = S`
- WHEN `brain_attach(S)` runs
- THEN the response is a valid envelope with `payload.session_id == S`

#### Scenario: Attach to unknown rejected

- GIVEN no session with `id = "bogus"`
- WHEN `brain_attach("bogus")` runs
- THEN `error.code == "session_not_found"`

#### Scenario: Attach to expired evicts

- GIVEN a session whose TTL elapsed
- WHEN `brain_attach(S)` runs
- THEN `error.code == "session_expired"`
- AND the session is removed from the registry

#### Scenario: Status returns history FIFO order

- GIVEN a session with 2 history entries
- WHEN `brain_status(S)` runs
- THEN `history[0]` is the oldest entry

#### Scenario: Empty history is empty array

- GIVEN a fresh session
- WHEN `brain_status(S)` runs
- THEN the JSON `history` field is `[]`
- AND `history_len == 0`

#### Scenario: Focus empty string rejected

- GIVEN `focus_node = None`
- WHEN `brain_focus(S, "")` runs
- THEN `error.code == "invalid_focus_node"`
- AND `focus_node` remains `None`

#### Scenario: Close removes session

- GIVEN an open session `S`
- WHEN `brain_close(S)` runs
- THEN `payload.closed == true`
- AND a subsequent `brain_attach(S)` returns `"session_not_found"`

#### Scenario: Close idempotent

- GIVEN `brain_close(S)` already returned `closed: true`
- WHEN `brain_close(S)` runs again
- THEN `payload.closed == false`
- AND the response is not an error envelope

#### Scenario: Close of unknown not an error

- GIVEN no session `S`
- WHEN `brain_close("bogus")` runs
- THEN `payload.closed == false`
- AND the response is not an error envelope

### Requirement: brain_ask

MUST accept required `session_id` and required `question` (non-empty). MUST look up the session, refresh `last_activity`, prepend the focus-node token (if any), dispatch via `AskRouter::classify` + `dispatch_ask`, append a `HistoryEntry` on success, and return `{ session_id, ask_envelope, focus_injected, enriched_question, pattern_id, history_len_after }`. The inner envelope's `provenance.source` MUST remain `"ask-router"`. Per-tool: `tools/brain_ask.md`.

#### Scenario: Ask appends history on success

- GIVEN empty history
- WHEN `brain_ask(S, "what does `foo` call?")` returns success
- THEN `history.len() == 1`
- AND `history[0].pattern_id` is in `1..=8`

#### Scenario: Ask injects focus node

- GIVEN `focus_node = Some("AuthService")`
- WHEN `brain_ask(S, "what does it call?")` runs
- THEN `enriched_question` starts with a backtick-quoted `AuthService` token

#### Scenario: Ask with empty question rejected

- GIVEN a valid `session_id`
- WHEN `brain_ask(S, "")` runs
- THEN `error.code == "missing_required_arg"` naming `question`

#### Scenario: Ask on unknown session rejected

- GIVEN no session `S`
- WHEN `brain_ask("bogus", "...")` runs
- THEN `error.code == "session_not_found"`
- AND no global state is created

#### Scenario: Ask on expired session rejected

- GIVEN a session whose TTL elapsed
- WHEN `brain_ask(S, "...")` runs
- THEN `error.code == "session_expired"`
- AND no history entry is appended

#### Scenario: Failed dispatch does not append

- GIVEN empty history
- WHEN `brain_ask` returns an error envelope from the ask-router
- THEN `history` remains empty
- AND `last_activity` is still refreshed

### Requirement: Concurrency Safety

Two concurrent `brain_ask` calls on the same session MUST both succeed (or both produce well-formed error envelopes) without deadlock, panic, or torn writes. The registry lock MUST be released before any `await` on the ask dispatcher. History push MUST be atomic w.r.t. other mutations on the same session.

#### Scenario: Two concurrent asks succeed

- GIVEN empty history
- WHEN two `brain_ask` calls run concurrently
- THEN both return valid envelopes
- AND final `history.len() == 2`

#### Scenario: Concurrent ask and focus do not deadlock

- GIVEN a session exists
- WHEN `brain_ask` and `brain_focus` run concurrently
- THEN both return within a bounded time

### Requirement: Empty History Semantics

A session with no history MUST expose `history = []` and `history_len = 0` in every status/focus payload. The ask router MUST NOT receive the session's history as input (history is session-level, not ask-router concern).

#### Scenario: Ask router does not see history

- GIVEN a session with 3 history entries
- WHEN `brain_ask` is called
- THEN `dispatch_ask` does not receive the history list
- AND the ask router's `McpResultEnvelope` is preserved verbatim under `payload.ask_envelope`

## Out of Scope

PostgreSQL persistence (Phase 3); `WorkspaceSession` reuse (rejected); background TTL sweeper; cross-session focus sharing; streaming/chunked history reads; UI/dashboard; auth/authz; modification of the 18 existing one-shot tools.

## TDD RED Gate

All scenarios above MUST have a failing test before implementation. Suite MUST: (1) mock `Arc<ExplorerService>` and `Option<Arc<CallGraph>>`; (2) drive every `brain_*` tool through the MCP dispatch layer (`call_tool_args` helper); (3) cover happy paths, expired-session, unknown-session, empty-history, and concurrent-access scenarios; (4) verify `TOOL_NAMES.len() == 24` and the 6 new constants are present; (5) verify `provenance.source == Some("brain-session")` for every `brain_*` response; (6) verify focus-node injection via an instrumented mock capturing what `AskRouter::classify` receives; (7) verify lazy TTL eviction by manipulating the injected clock. Gate fails if any scenario lacks a test, any test compiles/passes before production code exists, or the 18 prior tools' test suite regresses.
