# Tasks: MCP Result Envelope

> `mcp-result-envelope` · `crates/cognicode-explorer/src/mcp.rs` (2398 → ~2551) · Strict TDD · 3 chained PRs · +153 net

## Review Workload Forecast

Decision needed before apply: Yes
Chained PRs recommended: Yes
Chain strategy: feature-branch-chain
400-line budget risk: Medium

| Unit | Goal | PR | Base | ~Lines |
|------|------|----|------|--------|
| 1 | Types + EnvelopeError | PR 1 | `feature/mcp-result-envelope` | +50 |
| 2 | Helpers + remove old + migrate 4 tests | PR 2 | PR 1 branch | +25 |
| 3 | 17 arms + 11 test unwraps + 9 envelope tests | PR 3 | PR 2 branch | +78 |

## Phase 1 — Types (RED → GREEN)

- [ ] 1.1 RED: `envelope_struct_has_six_fields`, `provenance_metadata_default_is_none`, `follow_up_default_constructs`, `envelope_error_confidence_out_of_range_constructs`. Validate: compile error.
- [ ] 1.2 RED: `provenance_new_accepts_boundary_zero`, `provenance_new_accepts_boundary_one`, `provenance_new_rejects_above_one`, `provenance_new_rejects_negative`. Validate: compile error.
- [ ] 1.3 GREEN: module-scope `pub struct McpResultEnvelope<T>` (6 fields; `Debug,Clone,Serialize,Deserialize`); `pub struct ProvenanceMetadata` (+`Default`, `fn new(c,s)->Result<Self,EnvelopeError>` validates `0.0..=1.0`); `pub struct FollowUp` (+`Default`); `#[derive(thiserror::Error)] pub enum EnvelopeError { #[error("confidence {0} out of range [0.0, 1.0]")] ConfidenceOutOfRange(f64) }`. Validate: 8 RED pass.
- [ ] 1.4 `cargo test -p cognicode-explorer` → 53 + 8 = 61 pass. Commit `feat(mcp): add McpResultEnvelope, ProvenanceMetadata, FollowUp, EnvelopeError types`.

## Phase 2 — Helpers (RED → GREEN, atomic old-removal)

- [ ] 2.1 RED 9 tests: `envelope_ok_success_wraps_payload`, `envelope_ok_err_returns_error_result`, `envelope_ok_provenance_none_serializes_as_null`, `envelope_ok_follow_ups_default_empty`, `envelope_ok_timestamp_rfc3339_utc`, `envelope_ok_version_matches_pkg`, `envelope_ok_direct_raw_value`, `envelope_ok_provenance_confidence_out_of_range`, `envelope_version_field_detects_wrapper`. Validate: 9 fail to compile.
- [ ] 2.2 GREEN-Add after `err` (L714): `envelope_ok<T:Serialize>(tool_name, &ExplorerResult<T>, Option<ProvenanceMetadata>)` (Err → `err(e.to_string())`); `envelope_ok_direct<T:Serialize>(tool_name, &T, Option<ProvenanceMetadata>)`; private `ok_envelope_inner` builds `json!({tool_name, version: env!("CARGO_PKG_VERSION"), timestamp: Utc::now().to_rfc3339(), provenance, payload: to_value(v), suggested_follow_ups: []})`. Validate: build green.
- [ ] 2.3 GREEN-Remove **ATOMIC WITH 2.4**: delete `fn ok` (L702) and `fn ok_direct` (L736).
- [ ] 2.4 GREEN-Migrate **SAME COMMIT**: L1370 `ok(&Ok::<_,_>(s))` → `envelope_ok(TOOL_OPEN_WORKSPACE, &Ok::<_,_>(s), None)` (+assert envelope); L1381 → `envelope_ok::<_>(TOOL_OPEN_WORKSPACE, &Err(_), None)` (rename `envelope_ok_serializes_error_without_envelope`); L2099 `ok_direct(&v)` → `envelope_ok_direct(TOOL_IMPACT_RADIUS, &v, None)` + parse as `McpResultEnvelope<Vec<String>>` assert `env.payload`; L2113 `ok_direct(&none)` → `envelope_ok_direct(TOOL_IMPACT_RADIUS, &none, None)` + assert `obj["payload"].is_null()`. Validate: 51 + 4 + 8 = 63 pass. Commit `refactor(mcp): replace ok/ok_direct with envelope_ok/envelope_ok_direct`.

## Phase 3 — Dispatch + Test Migration (atomic)

- [ ] 3.1 RED `dispatch_arms_no_legacy_helpers`: `include_str!("mcp.rs")` assert `matches("ok(&").count() == 0` AND `matches("ok_direct(&").count() == 0`. Validate: fails.
- [ ] 3.2 GREEN 8 explorer arms L336/L349/L362/L375/L394/L407/L426/L441: `ok(&...)` → `envelope_ok(TOOL_OPEN_WORKSPACE|TOOL_SPOTTER_SEARCH|TOOL_INSPECT_OBJECT|TOOL_GET_VIEWS|TOOL_GET_VIEW|TOOL_GET_LENSES|TOOL_APPLY_LENS|TOOL_QUERY_MOLDQL, &..., None)`.
- [ ] 3.3 GREEN 6 impact + 3 graph arms L470/L495/L524/L561/L574/L603/L638/L658/L690: `ok_direct(&...)` → `envelope_ok_direct(TOOL_IMPACT_RADIUS|TOOL_IMPACT_FORWARD_RADIUS|TOOL_IMPACT_HAS_PATH|TOOL_IMPACT_SHORTEST_PATH|TOOL_IMPACT_DETECT_CYCLES|TOOL_IMPACT_COMPONENT|TOOL_GRAPH_SUBGRAPH|TOOL_GRAPH_CLUSTER|TOOL_GRAPH_EXPLAIN, &..., None)`.
- [ ] 3.4 GREEN unwrap envelope in 11 dispatch tests: parse `first_text(&result)` as `McpResultEnvelope<T>` then use `env.payload`. Sites L1679, L1694, L1707, L1728, L1774, L1802, L1826 (Value); L1855, L1892 (Vec<SccDto>); L1917 (Vec<String>). **Skip L2086** — error-path, no envelope.
- [ ] 3.5 GREEN 9 envelope tests from 2.1 pass in full suite. Validate: 53 + 8 + 1 + 9 = 71 pass. Commit `feat(mcp): migrate 17 dispatch arms and 11 dispatch tests to envelope wrapper`.

## Phase 4 — Verification Gate

- [ ] 4.1 `cargo build --workspace --all-targets` — no new warnings.
- [ ] 4.2 `cargo test --workspace` — 0 regressions.
- [ ] 4.3 Count envelope-named tests → expect 22.
- [ ] 4.4 Trace 36 scenarios in `specs/{mcp-result-envelope,mcp-tools-envelope,mcp-handler-helpers}/spec.md` (deferred to sdd-verify).
- [ ] 4.5 `cargo fmt --check && cargo clippy -p cognicode-explorer --all-targets -- -D warnings`.
- [ ] 4.6 Add rustdoc one-liner to each new `pub` type.

## Atomic-Commit Constraints

- **2.3 + 2.4** = single commit (remove old + migrate 4 helper-direct tests).
- **3.2 + 3.3 + 3.4** = single commit (17 arm edits + 11 test unwraps).
- Splitting either breaks the build between commits — breaks the chained-PR base.

## Dependencies

1.1,1.2 → 1.3 → 1.4 → 2.1 → 2.2 → 2.3+2.4 (atomic) → 3.1 → 3.2+3.3+3.4 (atomic) → 3.5 → 4.x

## Estimates

| Phase | Tasks | + | − | Net |
|-------|-------|---|---|-----|
| 1 | 4 | 50 | 0 | +50 |
| 2 | 4 | 50 | 25 | +25 |
| 3 | 5 | 90 | 17 | +73 |
| 4 | 6 | 5 | 0 | +5 |
| **Total** | **19** | **195** | **42** | **+153** |
