# Tasks: Brain Session — Conversational Graph Exploration

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~730 (new) + ~60 (test additions) ≈ 790 |
| 400-line budget risk | High |
| Chained PRs recommended | No (size:exception accepted) |
| Suggested split | Single PR with work-unit commits (see work-unit-commits) |
| Delivery strategy | exception-ok |
| Chain strategy | size-exception |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: High

> **Size note**: Maintainer has accepted the `size:exception` label. Total scope is ~730 LOC net across 4 new files and 3 modified files. The change is decomposed into **work-unit commits** (per the `work-unit-commits` skill) rather than chained PRs, keeping each commit ≤400 changed lines and each commit independently reviewable. Rollback remains single-action: revert the merge commit.

### Suggested Work Units (commits, not PRs)

| Unit | Goal | Approx LOC | Notes |
|------|------|------------|-------|
| WU-1 | Types & state model (`session/state.rs` + `mod.rs`) | ~70 | Foundation; no behaviour |
| WU-2 | `SessionRegistry` (open/attach/get/close/evict) | ~140 | Pure data structure, lock protocol |
| WU-3 | `BrainSessionService` (focus, history, ask_with_session) | ~180 | Depends on WU-2 |
| WU-4 | `dispatch_ask` signature extension | ~10 | All 18 existing call sites pass `None` |
| WU-5 | `mcp.rs` wiring: 6 constants + handler `registry` field + TOOL_NAMES | ~120 | Compile + tool list test flips 18→24 |
| WU-6 | `mcp.rs` 6 dispatch arms + 6 arg structs + 6 schemas | ~240 | Integration |
| WU-7 | `lib.rs` re-export + integration tests + regression | ~30 | Final wiring |

Each work unit is one commit. Apply phase MUST commit in this order; tests live alongside their unit (TDD RED→GREEN pairs are not split across commits).

---

## Phase 1: Foundation — Types & State (TDD RED→GREEN)

> Goal: Define `BrainSessionState`, `HistoryEntry`, and module skeleton. Zero behaviour, pure data + serde.

- [ ] 1.1 **RED**: Add `pub mod session;` to `crates/cognicode-explorer/src/lib.rs` after `pub mod ask;` and create empty `crates/cognicode-explorer/src/session/mod.rs` with `pub mod state; pub mod service; pub mod registry;`. **Expected**: compile error — submodules missing. Then create `session/state.rs` with only the type declarations (no `impl`) and write failing test:
  - Test: `brain_session_state_serializes_with_uuid_and_history()` in `session/state.rs` `#[cfg(test)] mod tests` — roundtrip a constructed state via `serde_json::to_string`/`from_str`, assert `history` is `[]` (not `null`) and `session_id` roundtrips.
  - **Validation**: `cargo test -p cognicode-explorer session::state` fails (compile or assertion).
- [ ] 1.2 **GREEN**: Implement `BrainSessionState` and `HistoryEntry` per design.md §`BrainSessionState`. Add `DEFAULT_HISTORY_CAP: usize = 50`, `DEFAULT_TTL_SECS: u64 = 1800`, `pub const fn new(...)` constructor that sets `created_at = last_activity = Utc::now()`. Derive `Debug, Clone, Serialize, Deserialize` on both structs. `history` field MUST use `#[serde(default)]` so empty vector serializes as `[]`, not omitted.
  - **Validation**: `cargo test -p cognicode-explorer session::state` passes; `cargo build -p cognicode-explorer` clean.
  - **LOC est.**: ~55 in `state.rs` (types + 1 test + module doc).

## Phase 2: Registry — `SessionRegistry` (TDD RED→GREEN)

> Goal: `Arc<Mutex<HashMap<SessionId, Arc<BrainSessionService>>>>` with open/attach/get/close/evict. Lock held only for the duration of the map operation — never across `await`.

- [ ] 2.1 **RED**: Create `session/registry.rs` with `pub struct SessionRegistry`, `pub enum SessionError { NotFound, Expired }` (thiserror), and a stub `impl SessionRegistry { pub fn new() -> Self { ... } }`. Write failing tests in `#[cfg(test)] mod tests`:
  - `registry_open_returns_uuid_and_workspace_id()`
  - `registry_attach_unknown_id_returns_session_not_found()`
  - `registry_close_unknown_id_returns_false_idempotent()`
  - `registry_close_known_id_returns_true_and_removes_session()`
  - `registry_lock_not_held_across_await()` — `#[tokio::test(flavor = "multi_thread")]` spawns 4 tasks that each `attach` then `tokio::time::sleep(10ms).await`; asserts all 4 complete (would deadlock if lock held across await).
  - **Validation**: `cargo test -p cognicode-explorer session::registry` fails (methods missing).
- [ ] 2.2 **GREEN**: Implement `SessionRegistry` per design.md §`SessionRegistry`. Methods:
  - `open(workspace_id, ttl_secs, service: Arc<ExplorerService>, graph: Option<Arc<CallGraph>>) -> String` — generates UUIDv4 via `uuid::Uuid::new_v4().to_string()`, inserts `Arc<BrainSessionService>` (stub service struct OK for now), returns id.
  - `attach(session_id) -> Result<Arc<BrainSessionService>, SessionError>` — calls `evict_expired()` first; if absent returns `Err(NotFound)`; clones `Arc` and refreshes `last_activity` on the service.
  - `get(session_id) -> Result<Arc<BrainSessionService>, SessionError>` — like attach but does NOT refresh `last_activity`.
  - `close(session_id) -> bool` — returns `false` if absent (idempotent), `true` and removes if present.
  - `evict_expired()` — iterates entries, removes those where `now - last_activity > ttl_secs` AND `ttl_secs > 0`. **`ttl = 0` disables expiry** (skip check).
  - All methods acquire the mutex, do the map op, drop the guard, return. The `Arc` clone is what crosses async boundaries.
  - **Validation**: `cargo test -p cognicode-explorer session::registry` passes all 5 tests. `cargo build -p cognicode-explorer` clean.
  - **LOC est.**: ~120 in `registry.rs`.

## Phase 3: Service — `BrainSessionService` (TDD RED→GREEN)

> Goal: Per-session logic — focus getter/setter, history push with FIFO cap, `ask_with_session` that prepends focus token. Depends on registry existing for `open()` to compile, but is independently testable.

- [ ] 3.1 **RED**: Create `session/service.rs` with `pub struct BrainSessionService { state: Mutex<BrainSessionState>, service: Arc<ExplorerService>, graph: Option<Arc<CallGraph>> }` and an empty impl. Write failing tests:
  - `service_set_focus_stores_value()` — `set_focus(Some("Foo::bar"))` then `focus_node() == Some("Foo::bar".into())`.
  - `service_set_focus_none_clears()` — `set_focus(None)` clears.
  - `service_history_caps_at_50_fifo()` — push 55 entries directly to inner state; assert `state.history.len() == 50` and the FIRST pushed entry is gone, the 55th is the last.
  - `service_ask_with_session_prepends_focus_node()` — `#[tokio::test]` with mock service, focus = `Some("Foo::bar")`, call `ask_with_session("what does it call?")`, assert the inner `dispatch_ask` was called with question `` `Foo::bar` what does it call? ``. (Use a test double or assert via the returned envelope's provenance payload if mocking dispatch_ask is impractical — the binding assertion is that the answer_summary in history reflects the enriched question.)
  - `service_failed_ask_does_not_append_to_history()` — call `ask_with_session` with a question that triggers an error envelope; assert `state.history.len() == 0`.
  - **Validation**: `cargo test -p cognicode-explorer session::service` fails.
- [ ] 3.2 **GREEN**: Implement `BrainSessionService` per design.md §`BrainSessionService`:
  - `new(session_id, workspace_id, ttl_secs, service, graph)` — constructs inner `BrainSessionState` via `state::BrainSessionState::new()`.
  - `set_focus(node)` — locks state, replaces `focus_node`.
  - `focus_node()` — locks state, returns `state.focus_node.clone()`.
  - `state_snapshot()` — locks state, returns `state.clone()` (for `brain_status`).
  - `ask_with_session(question) -> McpResultEnvelope<Value>` — locks state, reads `focus_node`, drops lock, builds `format!("`{focus}` {question}")` if Some else `question.to_string()`, calls `crate::ask::dispatch::dispatch_ask(classified, &self.service, &self.graph, None).await`. If returned envelope `provenance.is_error()` returns `true`, do NOT push history. Otherwise push `HistoryEntry { question, answer_summary: <truncate 200 chars from payload>, pattern_id: classified.category as u8, ts: Utc::now() }`, then truncate to `DEFAULT_HISTORY_CAP`.
  - **Note**: `ask_with_session` MUST classify the question before calling `dispatch_ask`. Use `AskRouter::classify(&enriched_question, &self.service)` — import from `crate::ask::router`. If classification returns an error envelope, propagate without history push.
  - **Validation**: `cargo test -p cognicode-explorer session::service` passes. `cargo build -p cognicode-explorer` clean.
  - **LOC est.**: ~180 in `service.rs`.

## Phase 4: ask-router delta — `dispatch_ask` signature (TDD RED→GREEN)

> Goal: Extend `dispatch_ask` signature with `Option<&BrainSessionService>` param, unused in Phase 2. Zero behavioural change for existing 18-tool call sites.

- [ ] 4.1 **RED**: Modify signature in `crates/cognicode-explorer/src/ask/dispatch.rs` line 34 to add `_session: Option<&BrainSessionService>` and propagate through the **5 existing call sites** of `dispatch_ask` (one in `mcp.rs:807`, plus any other call sites in `ask/` module). Compile MUST fail until the param is added everywhere. Update the 3 existing tests (`dispatch_ask_missing_question_returns_validation_error`, `dispatch_ask_non_graph_question_succeeds_with_provenance`, `dispatch_ask_graph_question_without_graph_returns_unavailable_envelope`) to pass `None`.
  - **Validation**: `cargo build -p cognicode-explorer` fails (signature mismatch). `cargo test -p cognicode-explorer ask::dispatch` fails.
- [ ] 4.2 **GREEN**: Apply the signature change to all 5 call sites; pass `None` everywhere except where the new `brain_ask` arm (Phase 6) will pass `Some`. The body of `dispatch_ask` ignores `_session` (prefixed with underscore). Run full explorer test suite to confirm zero regressions.
  - **Validation**: `cargo test -p cognicode-explorer` passes (all 18-tool tests still green). `grep -rn "dispatch_ask(" crates/cognicode-explorer/src/` shows all call sites have 4 args.
  - **LOC est.**: +8 in `dispatch.rs`, +0 (no change) at 4 call sites, +0 at the new `brain_ask` site (added in Phase 6).

## Phase 5: MCP wiring — constants, handler field, TOOL_NAMES (TDD RED→GREEN)

> Goal: Add 6 tool constants, the `registry` field on `ExplorerMcpHandler`, and grow `TOOL_NAMES` from 18 → 24. The schema tests for 18 must flip to 24.

- [ ] 5.1 **RED**: In `crates/cognicode-explorer/src/mcp.rs`:
  1. Add 6 constants after `TOOL_ASK` (line 145): `TOOL_BRAIN_OPEN`, `TOOL_BRAIN_ATTACH`, `TOOL_BRAIN_ASK`, `TOOL_BRAIN_FOCUS`, `TOOL_BRAIN_STATUS`, `TOOL_BRAIN_CLOSE`.
  2. Extend `TOOL_NAMES` array with these 6 in this exact order.
  3. Add `registry: crate::session::SessionRegistry` field to `ExplorerMcpHandler` struct (line 320).
  4. Update both constructors `new()` and `with_graph()` to initialize `registry: SessionRegistry::new()`.
  5. Update the regression test at line 3163 from `assert_eq!(TOOL_NAMES.len(), 18);` to `assert_eq!(TOOL_NAMES.len(), 24);`.
  6. Write 6 stub tests in the existing `#[cfg(test)]` block (around line 1134) named `brain_open_dispatches`, `brain_attach_dispatches`, `brain_ask_dispatches`, `brain_focus_dispatches`, `brain_status_dispatches`, `brain_close_dispatches` — each calls `handler.dispatch(ToolCall { name: TOOL_BRAIN_*, arguments: json!({}) })` and expects a `missing_required_arg` envelope (NOT a compile error).
  - **Validation**: `cargo test -p cognicode-explorer mcp` fails. `cargo build -p cognicode-explorer` fails (dispatch arms missing).
- [ ] 5.2 **GREEN**: Wire the 6 stub dispatch arms in the `dispatch()` match (after the `TOOL_ASK` arm at line ~807). Each arm is a 2-line stub returning `err("TODO: implement in Phase 6")`. The 6 stub tests now pass the dispatch (no compile error) but the assertion will need updating in Phase 6.
  - **Validation**: `cargo test -p cognicode-explorer mcp::tool_names_has_twenty_four_entries` passes. `cargo build -p cognicode-explorer` clean.
  - **LOC est.**: ~120 in `mcp.rs` (constants + array + struct field + constructors + 6 stub arms + tests).

## Phase 6: MCP wiring — full dispatch arms + arg structs + schemas (TDD RED→GREEN)

> Goal: Implement the 6 dispatch arms end-to-end. Each tool validates args, calls the appropriate `SessionRegistry` method, returns `McpResultEnvelope<T>` with `source = "brain-session"`.

- [ ] 6.1 **RED**: Define the 6 arg structs in `mcp.rs` (after existing arg structs, around line 1130):
  ```rust
  #[derive(Deserialize)] struct BrainOpenArgs { workspace_id: Option<String>, ttl: Option<u64> }
  #[derive(Deserialize)] struct BrainAttachArgs { session_id: Option<String> }
  #[derive(Deserialize)] struct BrainAskArgs { session_id: Option<String>, question: Option<String> }
  #[derive(Deserialize)] struct BrainFocusArgs { session_id: Option<String>, focus_node: Option<String> }
  #[derive(Deserialize)] struct BrainStatusArgs { session_id: Option<String> }
  #[derive(Deserialize)] struct BrainCloseArgs { session_id: Option<String> }
  ```
  - **Validation**: `cargo build -p cognicode-explorer` fails (structs undeclared).
- [ ] 6.2 **GREEN — brain_open**: Replace the `TOOL_BRAIN_OPEN` stub with: parse `BrainOpenArgs`, validate `workspace_id` (non-empty → else `invalid_workspace_id` envelope), validate `ttl` (None or 1..=86400 → else `invalid_ttl` envelope; `0` is valid and means no expiry), call `self.registry.open(workspace_id, ttl.unwrap_or(DEFAULT_TTL_SECS), self.service.clone(), self.graph.clone())`, return envelope with `{ session_id, workspace_id, ttl_secs, created_at }` payload.
- [ ] 6.3 **GREEN — brain_attach**: Replace stub with: parse `BrainAttachArgs`, validate `session_id` (non-empty), call `self.registry.attach(&session_id)`, on `Err(NotFound)` return `session_not_found` envelope, on `Err(Expired)` return `session_expired` envelope, on `Ok(arc)` return envelope with `{ session_id, workspace_id, last_activity, ttl_secs, focus_node }`.
- [ ] 6.4 **GREEN — brain_ask**: Replace stub with: parse `BrainAskArgs`, validate both `session_id` and `question` (non-empty), call `self.registry.get(&session_id)` (NOT `attach` — we want the timestamp to not refresh on ask), on `Err(NotFound)` → `session_not_found`, on `Ok(arc)` call `arc.ask_with_session(&question).await`. The returned envelope MUST have `provenance.source = "brain-session"` (override via envelope helper). Payload is the inner envelope's payload (primary_result + supporting + follow_ups). On error envelope, do not append history (already handled in `ask_with_session`).
- [ ] 6.5 **GREEN — brain_focus**: Replace stub with: parse `BrainFocusArgs`, validate `session_id` (non-empty), validate `focus_node` if `Some` (non-empty → else `invalid_focus_node` envelope), call `self.registry.get(&session_id)`, on `Ok(arc)` call `arc.set_focus(focus_node)`, return envelope with `{ session_id, focus_node }` (echoes the new value, `null` if cleared).
- [ ] 6.6 **GREEN — brain_status**: Replace stub with: parse `BrainStatusArgs`, validate `session_id`, call `self.registry.get(&session_id)`, on `Ok(arc)` call `arc.state_snapshot()`, return envelope with the full state (including `history` which MUST serialize as `[]` if empty — verify via test).
- [ ] 6.7 **GREEN — brain_close**: Replace stub with: parse `BrainCloseArgs`, validate `session_id`, call `self.registry.close(&session_id)`, return envelope with `{ session_id, closed: <bool> }`. **Idempotent**: unknown id returns `closed: false` with HTTP-200 envelope, NOT an error envelope.
- [ ] 6.8 **GREEN — 6 tool schemas**: Add 6 entries to `build_tool_schemas()` (around line 372) following the existing JSON-Schema pattern. Each schema lists its required and optional fields per the 6 `tools/*.md` contracts in `openspec/changes/brain-session/specs/tools/`.
  - **Validation**: `cargo test -p cognicode-explorer mcp` passes all 6 dispatch tests. `cargo test -p cognicode-explorer` passes (regression guard: 18-tool tests still green). `cargo build --release` clean.
  - **LOC est.**: ~240 in `mcp.rs`.

## Phase 7: Integration & final wiring (TDD RED→GREEN)

> Goal: Module re-exports, end-to-end integration test covering a full session lifecycle, and final regression sweep.

- [ ] 7.1 **RED**: Add `pub use session::{BrainSessionService, BrainSessionState, HistoryEntry, SessionRegistry};` to `lib.rs` after `pub mod session;`. Write a new integration test in `crates/cognicode-explorer/tests/brain_session_lifecycle.rs`:
  - `open_then_ask_then_focus_then_status_then_close_lifecycle()` — uses `call_tool_args` helper to: `brain_open` → extract `session_id` → `brain_ask(question_with_no_entities)` → assert success envelope → `brain_focus` with a node → `brain_ask` again with a vague pronoun like "what does it call?" → assert `history.len() == 2` in `brain_status` → `brain_close` → assert `closed: true` → `brain_attach` with same id → assert `session_not_found`.
  - **Validation**: `cargo test -p cognicode-explorer --test brain_session_lifecycle` fails (re-exports missing or types inaccessible).
- [ ] 7.2 **GREEN**: Run the lifecycle test; fix any wiring gaps. Then run the full workspace test suite:
  - `cargo test --workspace --all-features` — must pass with zero regressions.
  - `cargo clippy --workspace -- -D warnings` — must be clean.
  - `cargo fmt --all -- --check` — must pass.
  - **Validation**: All three commands exit 0. `grep -c "TOOL_" crates/cognicode-explorer/src/mcp.rs` shows 24 tool constants.
  - **LOC est.**: +1 in `lib.rs`, +1 in `lib.rs` re-exports, ~80 in new integration test file.

---

## Validation Summary (run after each phase)

| Phase | Command | Expected |
|-------|---------|----------|
| 1 (types) | `cargo test -p cognicode-explorer session::state` | RED → GREEN |
| 2 (registry) | `cargo test -p cognicode-explorer session::registry` | RED → GREEN |
| 3 (service) | `cargo test -p cognicode-explorer session::service` | RED → GREEN |
| 4 (ask delta) | `cargo test -p cognicode-explorer` | All 18-tool tests stay GREEN; new arg compiles |
| 5 (wiring stub) | `cargo test -p cognicode-explorer mcp::tool_names_has_twenty_four_entries` | 24, not 18 |
| 6 (full arms) | `cargo test -p cognicode-explorer mcp` | All 6 dispatch tests pass |
| 7 (integration) | `cargo test --workspace --all-features` | Zero regressions |

## Dependency Graph

```
Phase 1 (state) ─→ Phase 2 (registry) ─→ Phase 3 (service) ─┐
                                                            ├─→ Phase 5 (wiring) ─→ Phase 6 (arms) ─→ Phase 7 (integration)
Phase 4 (ask signature) ──────────────────────────────────┘                            (independent of 1-3 but compiles in parallel)
```

Phase 4 can run in parallel with Phases 1-3 (different file). Phase 5 unblocks Phase 6. Phase 7 requires everything.

## TDD Discipline

For every task tagged **RED**, the test MUST be written and the failure confirmed (`cargo test` exits non-zero) BEFORE the **GREEN** task begins. Do not combine RED+GREEN into one commit. The work-unit commits in the forecast table above are GREEN commits only — RED tests live in the same commit as the GREEN implementation but the diff is structured so reviewers see the failing test addition first.

## Out of Scope (locked by spec)

- PostgreSQL persistence (Phase 3)
- `WorkspaceSession` reuse
- Background TTL sweeper
- Cross-session focus
- Streaming responses
- UI / dashboard
- Auth / authz
- Any modification to the 18 existing tools beyond the `dispatch_ask` signature
