# Delta for brain-session (MODIFIED)

> Base spec: `openspec/changes/brain-session/specs/brain-session/spec.md` (in-flight; not yet archived). This delta adds federation awareness to the existing `brain-session` capability.

## Summary of Changes

| Aspect | Before | After |
|--------|--------|-------|
| `BrainSessionState` fields | session_id, workspace_id, created_at, last_activity, ttl, focus_node, history | **+ `spaces: Vec<SpaceId>`** (new, see MODIFIED §Session State Model) |
| `BrainSessionService` holds | `Arc<ExplorerService>`, `Option<Arc<CallGraph>>` | **+ `Arc<FederatedGraphService>`** (new, see MODIFIED §Session Service) |
| `brain_open` args | `workspace_id`, `ttl` | **+ optional `spaces: Vec<SpaceSpec>`** (see ADDED §brain_open spaces) |
| `brain_status` payload | session_id, workspace_id, last_activity, ttl_secs, focus_node, history | **+ `space_count`, `spaces: [{id, name, kind}]`** (see ADDED §brain_status spaces) |
| `brain_ask` semantics | Asks against single graph | **Routes through `FederatedGraphService` for the session's spaces; default space = current behavior** (see MODIFIED §brain_ask federation) |
| TOOL_NAMES length | 24 (6 brain + 18 one-shot) | **27** (3 new federation tools added) |

## MODIFIED Requirements

### Requirement: Session State Model

`BrainSessionState` MUST contain: `session_id: String` (UUIDv4), `workspace_id: String`, `created_at: i64` (epoch ms), `last_activity: i64` (epoch ms), `ttl: u64` (seconds, default 1800), `focus_node: Option<String>`, `history: Vec<HistoryEntry>` (capped FIFO, default 50), **`spaces: Vec<SpaceId>` (default empty, registration order preserved)**. `HistoryEntry = { question, answer_summary, pattern_id: u8, ts: i64 }`. State MUST be private, exposed only via read-only accessors.
(Previously: `spaces` field did not exist; session was bound to a single workspace.)

#### Scenario: Fresh state populated

- GIVEN `BrainSessionService::open(ws, ttl)` runs
- THEN `session_id` is a non-empty UUIDv4
- AND `created_at == last_activity`
- AND `focus_node` is `None`
- AND `history` is empty
- AND **`spaces` is empty**

#### Scenario: Spaces persist across the session

- GIVEN a session opened with `spaces = [SpaceId("repo-a"), SpaceId("repo-b")]`
- WHEN `brain_status(S)` runs
- THEN `state.spaces == [SpaceId("repo-a"), SpaceId("repo-b")]` (registration order preserved)

### Requirement: Session Service

`BrainSessionService` MUST expose `state_snapshot()`, `set_focus()`, `focus_node()`, `push_history()`, `ask_with_session()`, and the new **federation methods** `add_space(Space) -> Result<(), SpaceError>`, `remove_space(SpaceId) -> bool`, `spaces() -> Vec<Space>`, `federated_graph() -> Arc<FederatedGraphService>`. The service MUST hold an `Arc<FederatedGraphService>` (constructed in `new`) so `ask_with_session` can route queries through it.
(Previously: the service held `Arc<ExplorerService>` and `Option<Arc<CallGraph>>` and asked against the single graph; no federation.)

#### Scenario: add_space stores the space

- GIVEN a fresh session
- WHEN `add_space(Space { id: SpaceId("a"), name: "auth", kind: Repo })` runs
- THEN `spaces().len() == 1` AND the stored space matches the input

#### Scenario: add_space rejects duplicate id

- GIVEN a session with space `repo-a`
- WHEN `add_space(Space { id: SpaceId("repo-a"), ... })` runs
- THEN it returns `Err(SpaceError::Duplicate)` AND the space count is unchanged

#### Scenario: remove_space is idempotent

- GIVEN a session with only `repo-a`
- WHEN `remove_space(SpaceId("repo-b"))` runs
- THEN it returns `false` AND no error is raised

### Requirement: brain_ask (Federation)

`brain_ask` MUST look up the session, refresh `last_activity`, prepend the focus-node token (if any), then route through the session's `FederatedGraphService` (not the legacy single graph). When `spaces` is empty, the federation service uses the default space and the behavior is byte-for-byte identical to the pre-federation implementation. When `spaces` is non-empty, results are merged and the response payload includes `space_id` per result.
(Previously: routed through `dispatch_ask` against the session's `Option<Arc<CallGraph>>`.)

#### Scenario: Single-space ask is backward-compatible

- GIVEN a session opened without `spaces` (no `brain_add_space` calls)
- WHEN `brain_ask(S, "...")` runs
- THEN the inner `ask_envelope` is produced exactly as before (no `space_id` annotations in the payload; same wire shape as the pre-federation response)

#### Scenario: Multi-space ask tags results

- GIVEN a session with 2 spaces `repo-a` and `repo-b`
- WHEN `brain_ask(S, "find all `User` nodes")` runs
- THEN every result item in the payload carries a `space_id` field equal to `repo-a` or `repo-b`

#### Scenario: Multi-space ask preserves focus injection

- GIVEN `focus_node = Some("AuthService")` and 2 spaces
- WHEN `brain_ask(S, "what does it call?")` runs
- THEN the enriched question is `` `AuthService` what does it call? `` and the ask router receives it (focus injection is preserved across the federation boundary)

## ADDED Requirements

### Requirement: brain_open spaces (NEW)

`brain_open` MUST accept optional `spaces: Vec<SpaceSpec>` where `SpaceSpec = { id, name, kind, source_path?, config? }`. When present and non-empty, each entry is registered in the session via `add_space` BEFORE the response is returned. The `workspace_id` and `ttl` semantics are unchanged.

#### Scenario: Open with 2 spaces

- GIVEN `brain_open({ workspace_id: "ws-1", spaces: [{id: "a", name: "auth", kind: "Repo"}, {id: "b", name: "docs", kind: "Docs"}] })`
- THEN `payload.space_count == 2` AND `brain_spaces(S)` returns both spaces

#### Scenario: Open with empty spaces array is backward-compatible

- GIVEN `brain_open({ workspace_id: "ws-1", spaces: [] })` or `brain_open({ workspace_id: "ws-1" })`
- THEN `payload.space_count == 0` AND the session behaves byte-for-byte as in the pre-federation implementation

### Requirement: brain_status spaces (NEW)

`brain_status` MUST include `space_count: usize` and `spaces: [{ id: String, name: String, kind: String }]` in the payload. Empty session returns `space_count: 0, spaces: []`. The existing fields (session_id, workspace_id, focus_node, history, last_activity, ttl_secs) are unchanged.

#### Scenario: Status includes space summary

- GIVEN a session with 3 spaces
- WHEN `brain_status(S)` runs
- THEN `payload.space_count == 3` AND `payload.spaces.len() == 3` AND each entry has `id, name, kind`

#### Scenario: Empty session reports zero spaces

- GIVEN a session with no spaces
- WHEN `brain_status(S)` runs
- THEN `payload.space_count == 0` AND `payload.spaces == []`

### Requirement: Lazy Merge Candidate Detection (NEW)

`brain_spaces` (the new tool, see `brain-space-tools` spec) MUST compute merge candidates lazily via `FederatedGraphService::detect_merge_candidates()`. The result is returned inside `payload.merge_candidates: Vec<MergeCandidate>`. Detection runs ONLY on `brain_spaces` calls — not on `brain_ask` or `brain_status` (avoids expensive N² scans on the hot path).

#### Scenario: Candidates recomputed on each call

- GIVEN a session with 2 spaces, each containing a `User` symbol
- WHEN `brain_spaces(S)` is called twice in succession
- THEN both calls return the same `merge_candidates` (deterministic detection)

#### Scenario: No candidates for single-space sessions

- GIVEN a session with only `repo-a`
- WHEN `brain_spaces(S)` runs
- THEN `payload.merge_candidates == []` (same-space filter drops all pairs)

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| `brain_open` with 0-length `spaces` array | Treated as absent (default behavior) |
| `brain_add_space` of a space already registered in the session | Rejected with `duplicate_space` (the registry rejects re-registration) |
| `brain_remove_space` reduces session to 0 spaces | Allowed; the session still works; `brain_ask` uses the default space |
| A `Space` whose `id` contains `::` | Rejected at registration time (`invalid_space_id`) |
| `brain_ask` called on a session with 10 spaces | The federation service fans out to all 10 repos in parallel; results are merged; the call returns within bounded time (assertion in integration test) |

## Out of Scope

- Auto-federation (spaces are added manually)
- Cross-space edge creation
- Federated crate separation (service lives in `cognicode-explorer`)

## TDD RED Gate

All MODIFIED and ADDED requirements above MUST have failing tests. Suite MUST: (1) verify `BrainSessionState` serde includes `spaces: []` (not `null`); (2) cover `add_space` / `remove_space` happy + duplicate + idempotent paths; (3) verify `brain_open` backward-compat (no `spaces` arg → existing behavior); (4) verify `brain_ask` with empty `spaces` produces a payload byte-for-byte equivalent to the pre-federation response (regression gate); (5) verify `brain_ask` with non-empty `spaces` tags each result with `space_id`; (6) verify `brain_status` extension; (7) verify `TOOL_NAMES.len() == 27`. Gate fails if any scenario lacks a test, any test compiles/passes before production code exists, or the pre-federation test suite regresses.

## Dependencies

- `federated-spaces` — `Space`, `SpaceId`, `SpaceKind`, `SpaceRegistry`
- `federated-graph-service` — `FederatedGraphService`, `FederatedNode`
- `merge-candidate-detection` — `MergeDetector`, `MergeCandidate`
- `brain-space-tools` — the 3 new MCP tool dispatch arms

## Multimodal Feature Gate

The new `spaces` field, the new service methods, the new MCP tool arms, and the federation routing in `ask_with_session` are all gated by `#[cfg(feature = "multimodal")]`. With the feature OFF, the existing 6 `brain_*` tools and the existing 18 one-shot tools work exactly as before (the `spaces` field is omitted from the state, and `ask_with_session` routes through the legacy `dispatch_ask` path). This preserves the default build's behavior byte-for-byte.
