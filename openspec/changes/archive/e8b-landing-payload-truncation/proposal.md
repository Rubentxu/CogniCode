# Proposal: e8b-landing-payload-truncation

## Intent

Cycle `e8-graphlanding-affordances` (v0.24.1) shipped the frontend
truncation banner and its zod schema. The banner is dormant in production
because the backend `LandingPayload` (`crates/cognicode-explorer/src/dto.rs:782-799`)
does not return `truncated` or `truncated_reason`. This cycle closes the
contract gap so the frontend can light up the banner as soon as a future
cycle ships real landing data.

The user-facing outcome is **zero visible change today**: the banner stays
invisible because the handler still returns empty stubs. The visible change
will arrive with cycle `e10-landing-real-data` (which will populate
`entry_points` and `hot_paths`). Until then, this cycle:

1. Adds `truncated: bool` and `truncated_reason: Option<String>` to
   `LandingPayload`.
2. Adds a `LANDING_NODE_CAP` constant (50) and an `apply_landing_cap` helper
   that returns `(truncated, truncated_reason)`.
3. Sets `truncated = false` in the handler today (because the stub returns
   empty vectors), but the cap-aware helper is wired so the future cycle
   just plugs real data in.
4. Documents the contract delta in `graphlanding-affordances/spec.md`.

This is a **PATCH** semver target (`v0.24.2`). It is a wire-contract
addition (additive, non-breaking for old clients) and a strict-TDD test
suite. No public API removal, no Rust binary change of behaviour visible
to users.

## Scope

### In Scope

- Add `truncated: bool` field to `LandingPayload` (`crates/cognicode-explorer/src/dto.rs:782`).
- Add `truncated_reason: Option<String>` field to `LandingPayload` (same struct).
- Add `pub const LANDING_NODE_CAP: usize = 50;` in `dto.rs`.
- Add `apply_landing_cap(total: usize) -> (bool, Option<String>)` helper in `api.rs`.
  Today the helper is wired so it always returns `(false, None)` for `total == 0`.
  When a future cycle calls it with `total > LANDING_NODE_CAP`, it returns
  `(true, Some("node_cap"))`.
- Set `truncated: false, truncated_reason: None` in `landing_handler`.
- Update the handler's `TODO` comment to reflect that truncation is closed but
  data wiring is still open.
- New test file `crates/cognicode-explorer/tests/api_landing_truncation.rs`
  with RED-then-GREEN coverage:
  - DTO serializes/deserializes with `truncated: false, truncated_reason: None` defaults
  - DTO accepts `truncated: true, truncated_reason: Some("node_cap")` on deserialize
  - `apply_landing_cap(0)` returns `(false, None)`
  - `apply_landing_cap(49)` returns `(false, None)` (at cap)
  - `apply_landing_cap(50)` returns `(true, Some("node_cap"))` (over cap)
  - `apply_landing_cap(1000)` returns `(true, Some("node_cap"))`
  - Handler returns 200 with `truncated: false, truncated_reason: None` for an empty workspace
- Delta spec at `openspec/changes/e8b-landing-payload-truncation/specs/graphlanding-affordances/spec.md`.

### Out of Scope

- Wiring `entry_points`, `hot_paths`, `god_nodes` from the `Graph` facade to
  real data. (Tracked as `e10-landing-real-data`.)
- Changing the frontend banner behaviour. (E8 already shipped it.)
- Changing the existing `truncation_reason` (note: one 'i') field name in
  `ContextualGraphResponse`. (That is a separate, wider refactor that should
  become its own ADR.)
- Adding `LANDING_NODE_CAP` as a configurable parameter. (If we want runtime
  configurability, that's a config-port change in a future cycle.)

## Capabilities

### Modified Capabilities

- `graphlanding-affordances`:
  - Requirement 2 (schema accepts truncation fields): change from
    "MUST accept" to "MUST serialise and deserialise". The fields
    are no longer purely client-driven — the backend now produces them.
  - New Requirement 9 (backend `LandingPayload` contract): specifies
    the backend produces `truncated: false, truncated_reason: None`
    when the handler returns empty stubs, and
    `truncated: true, truncated_reason: Some("node_cap")` when
    `entry_points.len() > LANDING_NODE_CAP` (criterion for the future
    `e10-landing-real-data` cycle to satisfy).

(No new capability is created — `graphlanding-affordances` already
exists; this cycle modifies it.)

## Approach

Single Rust-only change. One PR off `main`:
`feat/e8b-landing-payload-truncation`.

Strict TDD: tests land first (RED), implementation makes them pass
(GREEN), refactor as needed.

- PR-1 (this cycle): backend DTO + helper + handler + tests. ≈50 LOC Rust
  + ≈100 LOC tests.

The future `e10-landing-real-data` will:
- Add a `pub async fn get_top_entry_points(&self, n: usize) -> ExplorerResult<Vec<InspectableObjectSummary>>`
  method to the `Graph` facade.
- Call it from `landing_handler` with `n = LANDING_NODE_CAP`.
- Apply `apply_landing_cap` to the result.

## Affected Areas

| Area | Impact | Description |
|---|---|---|
| `crates/cognicode-explorer/src/dto.rs` | Modified | +2 fields on `LandingPayload`, +1 constant |
| `crates/cognicode-explorer/src/api.rs` | Modified | New helper `apply_landing_cap`, handler sets `truncated: false`, TODO comment updated |
| `crates/cognicode-explorer/tests/api_landing_truncation.rs` | New | RED-then-GREEN test suite |
| `openspec/changes/e8b-landing-payload-truncation/specs/graphlanding-affordances/spec.md` | New | Delta spec (MODIFIED Req 2 + ADDED Req 9) |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Old clients (sub-v0.24.1) fail to deserialize new payload | Very Low | Fields are additive. Old clients ignore unknown fields on deserialize. New clients (≥ v0.24.1) parse the fields as `.optional()`. |
| Test runner parity issue with `sqlx::test` | Low | Use plain `#[tokio::test]` pattern (matching `api_graph_tests.rs`); the helper is a pure function, no DB needed |
| Wire format inconsistency with `ContextualGraphResponse.truncation_reason` | Low | Document explicitly in design.md; this cycle intentionally uses `truncated_reason` (matching `SubgraphResponse`) to avoid widening the existing inconsistency |
| Future cycle (`e10-landing-real-data`) changes the helper signature | Very Low | Helper signature is minimal (`(total: usize) -> (bool, Option<String>)`); easy to extend with a second parameter if needed |

## Rollback Plan

Single commit revert. The `LandingPayload` change is additive; reverting
removes the fields without affecting any other endpoint. Old clients (sub
v0.24.1) never saw the new fields anyway, so they remain forward-compatible.
The `apply_landing_cap` helper is only called from `landing_handler`; removing
the call from the handler is also safe.

No data migration, no schema migration, no DB changes. Revert is safe to
do at any commit within the PR.

## Dependencies

- None for this cycle (no upstream Rust changes required).
- Future cycle `e10-landing-real-data` depends on `apply_landing_cap`
  being present (it is, after this cycle lands).

## Success Criteria

- [ ] `cargo check --workspace --tests` passes with no new warnings.
- [ ] New tests in `api_landing_truncation.rs` pass (≥ 8 scenarios).
- [ ] Existing tests in `api_graph_tests.rs` and `api_rationale_tests.rs`
      still pass (no regression on related `LandingPayload` /
      `SubgraphResponse` / `ContextualGraphResponse` paths).
- [ ] `cargo clippy --workspace --tests` is no worse than baseline.
- [ ] `apps/explorer-ui` tests (vitest + playwright) still pass — no
      frontend change, but verify after Rust change that no MSW mock
      drift is introduced.
- [ ] `docs/ROADMAP.md` updated: `e8b-landing-payload-truncation`
      added under Completed at v0.24.2.
- [ ] `openspec/specs/graphlanding-affordances/spec.md` updated
      post-archive with MODIFIED Req 2 + ADDED Req 9.
