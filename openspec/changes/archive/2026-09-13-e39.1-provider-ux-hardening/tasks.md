# Tasks: E39.1 Provider UX Hardening

## Review Workload Forecast

Estimated changed lines: ~150–220 authored (composite mapping/bound + tests; no goldens). Delivery strategy: **auto-chain** — single PR, two sequential units; apply no-commit, orchestrator commits post-verify.

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: stacked-to-main
400-line budget risk: Low

### Suggested Work Units

| Unit | Goal | Likely PR | Focused test command | Runtime harness | Rollback boundary |
|------|------|-----------|----------------------|-----------------|-------------------|
| 1 | W2 deepest-answer mapping | PR 1 | `cargo test -p cognicode-core --lib composite` | `just lsi-providers` | Revert mapping/cause hunks |
| 2 | W3 bounded readiness (after 1) | PR 1 | `cargo test -p cognicode-core --lib composite` | Timed fall-through test | Revert `TierPolicy`/gate hunks + conformance pins |

Spec surface: None/None (proposal flags W2's "Not spec-covered" status; no delta files). Threat matrix: no new subprocess boundary — the bound caps an existing spawn/initialize attempt (e39 matrix inherited).

## Phase 1: W2 RED Tests

- [x] 1.1 RED `composite.rs`: missing file, lsp off → `get_definition` `Err(FileNotFound)`; prove today's `Ok(None)`.
- [x] 1.2 RED: same for `hover`; out-of-range location → `Err(InvalidLocation)`, not `Ok(None)`.
- [x] 1.3 Green guard: S1 clean miss → `Ok(None)`; S0 terminal hover `Ok(None)` → `Ok(None)`.
- [x] 1.4 Rework inverted `test_hover_unresolved_maps_to_ok_none`; strengthen weak `/nonexistent` assertions.

## Phase 2: W2 GREEN

- [x] 2.1 Carry the originating `CodeIntelligenceError` on provider-`Err` attempts; mechanical update of all attempt sites.
- [x] 2.2 Track the deepest attempted-tier answer for definition/hover; old-trait mapping: deepest `Err` → original variant, else `Ok(None)`.
- [x] 2.3 Collection ops keep `Err(Internal(<exhausted summary>))`; no CLI/MCP edits (check `commands.rs` error path + `lsp_handlers` `found:false`).
- [x] 2.4 `cargo test -p cognicode-core --lib composite` green.

## Phase 3: W3 RED Tests

- [x] 3.1 RED: `TierPolicy::fallback_readiness` (`Duration`, 2s default) + budget helper: lower tier enabled → bounded; disabled → full `wait_timeout_secs` (compile-level RED).
- [x] 3.2 RED behavioral: tiny bound + hierarchy on a real root → fall-through within the bound (elapsed << 30s), S2 `Unavailable` diagnostic names it, counters match e39 shapes.
- [x] 3.3 Guard: hover/find_references still serve S0 (`test_policy_gating_applies_to_every_op` green).

## Phase 4: W3 GREEN

- [x] 4.1 Wrap `wait_for_lsp_ready` in `tokio::time::timeout(effective)`; compute `effective` per op fallback eligibility; else full wait.
- [x] 4.2 Diagnostic message names the bounded fallback; counters/status shape unchanged.
- [x] 4.3 `provider_conformance.rs`: Java unavailable pins 5s; available branch full readiness (preserve declared-S2).
- [x] 4.4 Fix `TIER_ORDER` doc comment (walks claim vs per-op sequence + `status()` ordering) — e39 S2.
- [x] 4.5 `cargo test -p cognicode-core --lib composite` + `--lib code_intelligence` green.

## Phase 5: Guards + Handoff

- [x] 5.1 `just lsi-providers`; `just lsi-fixtures check` 42/42; `just lsi-equivalence`; `just lsi-identity`.
- [x] 5.2 `cargo check` core ± features / explorer / runtime / core-mock; `just lint`; fmt; clippy delta zero on touched paths.
- [x] 5.3 Not executed: gated `fact_bridge` surface (tiered trait unchanged) — record reused evidence.
- [x] 5.4 Update `.agent/TESTING-STATE.md` (active change + handoff); note e39 S1 deferred to archive.
