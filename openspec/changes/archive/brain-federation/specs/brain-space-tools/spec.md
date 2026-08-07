# brain-space-tools Specification (NEW)

## Purpose

Three new MCP tools that let an agent mutate the space set of a running brain session at runtime: `brain_add_space`, `brain_remove_space`, `brain_spaces`. They join the existing 6 `brain_*` tools and the 18 one-shot tools (total: 21 brain + 18 = 27 tools). All three are additive; backward-compatible with sessions created via `brain_open` without `spaces[]`. Gated by `multimodal` feature.

## Tool Constants

| Constant | Wire Name | Position in `TOOL_NAMES` |
|----------|-----------|--------------------------|
| `TOOL_BRAIN_ADD_SPACE` | `brain_add_space` | 25 |
| `TOOL_BRAIN_REMOVE_SPACE` | `brain_remove_space` | 26 |
| `TOOL_BRAIN_SPACES` | `brain_spaces` | 27 |

`TOOL_NAMES.len()` grows from 24 to 27. All 3 schemas appear in `build_tool_schemas()`.

## Requirements

### Requirement: brain_add_space

MUST accept required `session_id` (non-empty) and `space` object `{ id, name, kind, source_path?, config? }`. The space id MUST NOT be empty and MUST NOT already be registered in the session. On success, returns `{ session_id, space_id, name, kind, space_count_after }`. All `brain_*` responses carry `provenance.source = "brain-session"`.

#### Scenario: Add new space
- GIVEN a session with no spaces
- WHEN `brain_add_space(S, { id: "repo-a", name: "auth", kind: "Repo" })` runs
- THEN `payload.space_id == "repo-a"` AND `payload.space_count_after == 1`

#### Scenario: Duplicate id rejected
- GIVEN a session that already has space `repo-a`
- WHEN `brain_add_space(S, { id: "repo-a", ... })` runs
- THEN `error.code == "duplicate_space"` AND no second registration occurs

#### Scenario: Unknown session rejected
- GIVEN no session with id `S`
- WHEN `brain_add_space("bogus", { ... })` runs
- THEN `error.code == "session_not_found"`

#### Scenario: Invalid kind rejected
- GIVEN `brain_add_space(S, { id: "x", name: "x", kind: "Bogus" })`
- THEN `error.code == "invalid_space_kind"` (must be one of `Repo | Docs | Issues`)

### Requirement: brain_remove_space

MUST accept required `session_id` and `space_id`. The space MUST be registered in the session. On success, returns `{ session_id, space_id, removed: true, space_count_after }`. Idempotent: removing a non-registered space returns `removed: false` with HTTP-200 envelope (not an error).

#### Scenario: Remove existing space
- GIVEN a session with 2 spaces including `repo-a`
- WHEN `brain_remove_space(S, "repo-a")` runs
- THEN `payload.removed == true` AND `payload.space_count_after == 1`

#### Scenario: Remove unknown is idempotent
- GIVEN a session that has only `repo-a`
- WHEN `brain_remove_space(S, "repo-b")` runs
- THEN `payload.removed == false` AND the response is not an error envelope

#### Scenario: Unknown session rejected
- GIVEN no session with id `S`
- WHEN `brain_remove_space("bogus", "repo-a")` runs
- THEN `error.code == "session_not_found"`

### Requirement: brain_spaces

MUST accept required `session_id`. Returns `{ session_id, spaces: [{ id, name, kind, source_path?, config }], space_count, merge_candidates: [...] }`. The `merge_candidates` array is computed lazily on each call by `FederatedGraphService::detect_merge_candidates()`. An empty session returns `spaces: []` and `merge_candidates: []` (NOT `null`).

#### Scenario: List spaces and candidates
- GIVEN a session with 2 spaces (each with 2 nodes of the same label and kind)
- WHEN `brain_spaces(S)` runs
- THEN `payload.spaces.len() == 2` AND `payload.merge_candidates.len() >= 2` (each cross-space pair)

#### Scenario: Empty session
- GIVEN a session with 0 spaces
- WHEN `brain_spaces(S)` runs
- THEN `payload.spaces == []` AND `payload.merge_candidates == []` AND `payload.space_count == 0`

#### Scenario: Single space → no candidates
- GIVEN a session with only `repo-a`
- WHEN `brain_spaces(S)` runs
- THEN `payload.merge_candidates == []` (same-space pairs are filtered)

#### Scenario: Unknown session rejected
- GIVEN no session with id `S`
- WHEN `brain_spaces("bogus")` runs
- THEN `error.code == "session_not_found"`

### Requirement: brain_open Extension (ADDED — no modification of existing semantics)

`brain_open` MUST accept an OPTIONAL `spaces: [SpaceSpec]` array. When present, the session is created with those spaces pre-registered. When absent or empty, the session starts with the default space (`SpaceId("default")`) — byte-for-byte identical to current behavior. The existing `workspace_id` and `ttl` arguments are unchanged.

#### Scenario: Open with spaces
- GIVEN `brain_open({ workspace_id: "ws-1", spaces: [{ id: "a", name: "auth", kind: "Repo" }, { id: "b", name: "docs", kind: "Docs" }] })`
- THEN `payload.space_count == 2` AND the spaces appear in `brain_spaces` afterwards

#### Scenario: Open without spaces is backward-compatible
- GIVEN `brain_open({ workspace_id: "ws-1" })` (no `spaces` field)
- THEN `payload.space_count == 0` (the default space is implicit and not listed) AND the session works exactly as before

#### Scenario: Open with empty spaces array
- GIVEN `brain_open({ workspace_id: "ws-1", spaces: [] })`
- THEN `payload.space_count == 0` (no spaces registered)

### Requirement: brain_status Extension (ADDED — no modification of existing semantics)

`brain_status` MUST include `space_count: usize` and `spaces: [{ id, name, kind }]` in its payload. The full state shape (session_id, workspace_id, focus_node, history, etc.) is unchanged. Empty session returns `space_count: 0, spaces: []`.

#### Scenario: Status includes space summary
- GIVEN a session with 3 spaces
- WHEN `brain_status(S)` runs
- THEN `payload.space_count == 3` AND `payload.spaces.len() == 3` AND each entry has `id, name, kind`

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| `brain_add_space` with `id` containing `::` | Reject with `invalid_space_id` (the `::` separator is reserved by `FederatedNodeId`) |
| `brain_remove_space` reduces session to 0 spaces | Allowed — the session can run with no spaces; `brain_ask` returns the default-space result |
| `brain_spaces` on a session with 100 spaces and 1000 nodes | Returns up to 1000 candidates; the response is paginated via `next_cursor` (Phase 2 enhancement) |
| Two spaces with the same `name` but different `id` | Allowed (only `id` is unique) |
| `brain_add_space` with `config: null` | Coerced to `{}` (default empty object) |

## Out of Scope

- Space persistence across sessions (the registry is in-memory; closing the session drops the spaces)
- Space-level permissions / ownership
- Bulk import of spaces (one at a time)
- Re-deriving `merge_candidates` on every `brain_ask` (computed on `brain_spaces` only)

## TDD RED Gate

Before implementation: (1) 3+ unit tests per new tool (happy path, duplicate, unknown session, idempotent remove); (2) `TOOL_NAMES.len() == 27` regression test; (3) `brain_open` backward-compat test (no `spaces` field → behaves as before); (4) `brain_open` with `spaces` field test; (5) `brain_status` extension test; (6) `brain_spaces` lazy merge-candidate detection test using 2 mock repos with matching nodes. RED gate fails if any test passes before the 3 dispatch arms are wired or `provenance.source` is wrong.

## Dependencies

- `federated-spaces` — `SpaceId`, `SpaceKind`, `Space` types
- `federated-graph-service` — `FederatedGraphService`, `FederatedNode`
- `merge-candidate-detection` — `MergeDetector`
- `brain-session` (modified) — `BrainSessionState` gains `spaces: Vec<SpaceId>`
- `multimodal` feature gate
