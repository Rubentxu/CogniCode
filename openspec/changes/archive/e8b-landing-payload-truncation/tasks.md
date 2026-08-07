# Tasks: e8b-landing-payload-truncation

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~50 Rust + ~100 tests + ~10 spec delta |
| 400-line budget risk | Low (well under budget) |
| Chained PRs recommended | No (single small PR fits easily) |
| Suggested split | Single PR |
| Delivery strategy | single-pr |
| Chain strategy | n/a |
| Decision needed before apply | No |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: n/a
400-line budget risk: Low

---

## Phase 1: RED — write failing tests

- [ ] 1.1 Create `crates/cognicode-explorer/tests/api_landing_truncation.rs`
- [ ] 1.2 Add `#[test] fn apply_landing_cap_zero_returns_no_truncation` — `apply_landing_cap(0)` returns `(false, None)`
- [ ] 1.3 Add `#[test] fn apply_landing_cap_under_cap_returns_no_truncation` — `apply_landing_cap(49)` returns `(false, None)`
- [ ] 1.4 Add `#[test] fn apply_landing_cap_at_cap_returns_no_truncation` — `apply_landing_cap(50)` returns `(false, None)` (at cap, not over)
- [ ] 1.5 Add `#[test] fn apply_landing_cap_over_cap_returns_node_cap_truncation` — `apply_landing_cap(51)` returns `(true, Some("node_cap"))`
- [ ] 1.6 Add `#[test] fn landing_payload_serializes_with_truncated_false_by_default` — `serde_json::to_string(&landing)` includes `"truncated":false,"truncated_reason":null`
- [ ] 1.7 Add `#[test] fn landing_payload_deserializes_with_truncated_true` — JSON with `truncated: true, truncated_reason: Some("node_cap")` round-trips
- [ ] 1.8 Add `#[test] fn landing_handler_returns_truncated_false_for_empty_workspace` — `#[tokio::test]` calling the handler returns 200 + JSON body with `truncated: false, truncated_reason: null`
- [ ] 1.9 Run `cargo test -p cognicode-explorer --test api_landing_truncation` — confirm RED (tests fail because fields/helper don't exist yet)

## Phase 2: GREEN — implement to make tests pass

- [ ] 2.1 In `crates/cognicode-explorer/src/dto.rs`, add `pub const LANDING_NODE_CAP: usize = 50;` near the `LandingPayload` definition
- [ ] 2.2 Same file, add `pub truncated: bool` field to `LandingPayload`
- [ ] 2.3 Same file, add `pub truncated_reason: Option<String>` field to `LandingPayload`
- [ ] 2.4 In `crates/cognicode-explorer/src/api.rs`, add `pub(crate) fn apply_landing_cap(total: usize) -> (bool, Option<String>)` near the top of the file
- [ ] 2.5 In `landing_handler`, call `let (truncated, truncated_reason) = apply_landing_cap(0);`
- [ ] 2.6 Same handler, populate `LandingPayload { ..., truncated, truncated_reason }`
- [ ] 2.7 Update the handler's `TODO` comment to reflect that truncation is closed but data wiring is still open
- [ ] 2.8 Run `cargo test -p cognicode-explorer --test api_landing_truncation` — confirm GREEN
- [ ] 2.9 Run `cargo test -p cognicode-explorer` — confirm no regressions on existing tests
- [ ] 2.10 Run `cargo check --workspace --tests` — confirm exit 0 with no new warnings

## Phase 3: Verify & cleanup

- [ ] 3.1 Run `cargo clippy --workspace --tests` — no new warnings vs baseline
- [ ] 3.2 Push branch and open PR against `main`
- [ ] 3.3 Wait for CI to pass (or run `cargo test --workspace` locally)
- [ ] 3.4 Squash-merge to main
- [ ] 3.5 Tag `v0.24.2` (PATCH)
- [ ] 3.6 Archive the change:
      - `mv openspec/changes/e8b-landing-payload-truncation openspec/changes/archive/e8b-landing-payload-truncation`
      - Apply the delta in `openspec/changes/archive/e8b-landing-payload-truncation/specs/graphlanding-affordances/spec.md`
        to the canonical `openspec/specs/graphlanding-affordances/spec.md`
- [ ] 3.7 Update `docs/ROADMAP.md` — add `e8b-landing-payload-truncation` under Completed at v0.24.2
- [ ] 3.8 Write `openspec/changes/archive/e8b-landing-payload-truncation/archive-report.md`
