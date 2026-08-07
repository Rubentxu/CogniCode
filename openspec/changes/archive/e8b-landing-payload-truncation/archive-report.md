# Archive Report: e8b-landing-payload-truncation

**Change**: e8b-landing-payload-truncation
**Tag**: `v0.24.2` (PATCH)
**PR**: [#59](https://github.com/Rubentxu/CogniCode/pull/59)
**Verdict**: PASS
**Closed**: 2026-06-25

## Summary

Closes the backend truncation contract for `LandingPayload` that E8
(v0.24.1) left dormant in its verify-report W-1. The frontend banner
code is already in place; the backend now produces the `truncated` and
`truncated_reason` fields. The banner will activate as soon as the next
cycle (`e10-landing-real-data`) wires real `entry_points` data through
the `Graph` facade.

## Merged Commits

| SHA | Title |
|---|---|
| `c488523` | `feat(explorer): add LandingPayload.truncated + apply_landing_cap helper (e8b) (#59)` |

## What Changed

### Backend (Rust, single file pair)

- **`LandingPayload` DTO** (`crates/cognicode-explorer/src/dto.rs:782-799`):
  +2 fields (`truncated: bool`, `truncated_reason: Option<String>`). Both
  always serialised from v0.24.2 onward.
- **`LANDING_NODE_CAP = 50` constant** in `dto.rs`: UX choice for "how
  many nodes fit on one screen at default zoom". Not a perf knob.
- **`apply_landing_cap` helper** in `api.rs`: pure function with
  boundary `total > LANDING_NODE_CAP → (true, Some("node_cap"))`. Single
  source of truth for the truncation policy.
- **`landing_handler` updated**: calls `apply_landing_cap(0)` and
  populates the new fields. Handler still returns empty stubs because
  the data wiring is deferred; the truncation hook is in place for the
  future cycle.

### Tests (9 new in `api_landing_truncation.rs`)

- **5 helper boundary tests**: `apply_landing_cap(0)`, `(49)`, `(50)`,
  `(51)`, `(1000)` → verify the at-cap / over-cap boundary.
- **4 DTO serde tests**: round-trip with `truncated=true|false ×
  reason=Some|None` → verify wire compatibility.

All 9 pass. The 4 serde tests directly document the contract from
`openspec/specs/graphlanding-affordances/spec.md` Requirements 2 + 9.

### Spec delta (applied to canonical)

- **Requirement 2 MODIFIED**: was "schema accepts optional fields" (client-only).
  Now: "schema + backend contract — both fields REQUIRED in v0.24.2+, optional
  for legacy ≤ v0.24.1 backends". Added a third scenario for strict-mode
  failure on legacy servers.
- **Requirement 9 ADDED**: backend `landing_handler` MUST produce the
  fields; `apply_landing_cap` is the single source of truth; helper boundary
  at cap; transitional state documented (handler still returns empty stubs
  until `e10-landing-real-data`).

## Verification

| Phase | Result |
|---|---|
| `cargo test -p cognicode-explorer --test api_landing_truncation` | **9/9 pass** |
| `cargo test -p cognicode-explorer` (lib) | 598 pass |
| `cargo test -p cognicode-explorer` (all) | ~700 pass; 2 pre-existing failures unrelated (CI workflow file missing + flaky `file_operations` test) |
| `cargo test --workspace` | 1376 + 9 pass; 1 pre-existing flaky failure |
| `cargo check --workspace --tests` | exit 0; no new warnings |
| `npx vitest run` (apps/explorer-ui) | 671/671 pass |

## Artifacts

```
openspec/specs/graphlanding-affordances/spec.md   ← canonical updated (Req 2 MODIFIED + Req 9 ADDED)
openspec/changes/archive/e8b-landing-payload-truncation/
├── exploration.md
├── proposal.md
├── design.md
├── tasks.md
├── specs/graphlanding-affordances/spec.md       ← delta spec (frozen)
└── archive-report.md                              ← this file
```

## Open Follow-ups

These are explicitly out of scope for this cycle:

| Follow-up | Reason | Suggested cycle |
|---|---|---|
| Wire real `entry_points` / `hot_paths` data through the `Graph` facade | Handler still returns empty stubs; `entry_points.len() === 0` means `truncated` is always `false` and the banner never appears in production | `e10-landing-real-data` (MINOR or PATCH) |
| End-to-end integration test of `landing_handler` via `axum::Router` | Would require mocking all 6 services in `ApiState`; deferring to `e10-landing-real-data` which will need similar mocks | `e10-landing-real-data` |
| Harmonise `ContextualGraphResponse.truncation_reason` ↔ `SubgraphResponse.truncated_reason` field naming | Two existing endpoints use different names (one with extra 'i'). Should be a separate, wider refactor with its own ADR | `e11-context-response-field-naming` (PATCH) |
| Make `LANDING_NODE_CAP` configurable via `Config` | Constant is hard-coded as a UX choice; if users want it configurable later, the helper signature stays the same | Future config-port change |

## Lessons (jurisprudence)

### 1. Strict TDD pays off for contract-only changes

Even when the change is "just add a field", strict TDD:
- Documents the contract as code (the 9 tests ARE the contract).
- Catches wire-format drift (e.g., `"node_cap"` vs `"node-cap"`).
- Provides forward-compatible regression coverage for the future cycle
  that consumes the helper.

The cycle was small (≈50 LOC Rust + ≈150 LOC tests) but the tests are
the durable artefact, not the implementation.

### 2. Backend handler stubs are an anti-pattern

The `landing_handler` has been returning empty stubs since at least
2026-06-22 (per `feat/e7-renderer-scale-evaluation` commit `d4438b3`).
This cycle closes one aspect (truncation contract) but the data
wiring is still open. The TODO comment in `api.rs:670-671` is now
scoped to `e10-landing-real-data`. **The lesson**: when a handler
returns empty stubs, treat each stub field as a separate contract
gap with its own follow-up cycle, not a single big "wire everything"
task.

## Final Verdict

**PASS.** Contract closed. Banner remains dormant in production
(awaiting `e10-landing-real-data`). Future cycle can plug in by
replacing `apply_landing_cap(0)` with `apply_landing_cap(entry_points.len())`
in `landing_handler`.
