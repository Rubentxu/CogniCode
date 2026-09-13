# Proposal: E39.1 Provider UX Hardening

## Intent

e39 (M4, `87993f90`) verified PASS WITH WARNINGS; both warnings became decisions. W2: the old-trait mapping returns `Ok(None)` for definition/hover on tier exhaustion, swallowing `FileNotFound`/`InvalidLocation`/`ParseError`; the CLI dropped from error + non-zero exit to exit 0. W3: `get_hierarchy` pays an S2 readiness wait that `wait_timeout_secs` (30s) does not fully bound — `LspProcess::initialize` has its own 30s timeout.

## Scope

### In Scope
- W2: propagate the deepest tier's original error variant for definition/hover; clean misses stay `Ok(None)`; collection ops unchanged.
- W3: `TierPolicy::fallback_readiness` (2s default) bounds readiness only when a lower eligible tier exists; `Unavailable` diagnostic names it; full wait otherwise.
- RED-first tests; e39 guard suite; e39 S2 doc-comment fix.

### Out of Scope
S3, Java available branch, S2 ledger gap, M5/SCIP. e39 S1 deferred to archive. No CLI/MCP edits needed.

## Capabilities

New: None. Modified: None — e39 deltas stay satisfied (highest-first attempts, `TieredOutcome` shape unchanged). FLAG: e39 marked W2 "Not spec-covered"; this sharpens semantics without pinning them. No spec files edited.

## Approach

**W2 — deepest answer.** Provider-`Err` attempts carry the originating `CodeIntelligenceError`. Definition/hover take the deepest attempted tier's answer: `Err(e)` → original variant (never synthetic); clean miss or gate-only failure → `Ok(None)`. Restores the `87993f90^` passthrough (CLI error + non-zero exit); MCP keeps `found:false`.

**W3 — outer bound.** Readiness is wrapped in `tokio::time::timeout(effective)`; initialize blocks independently of the poll timeout. `effective = min(wait_timeout_secs, fallback_readiness)` when the op's lower tier is enabled, else full wait. Default 2s caps first-query degradation; later queries retry S2. Java conformance pins 5s unavailable / full readiness available.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/infrastructure/lsp/providers/composite.rs` | Modified | Mapping, `TierPolicy` field + gate, `TIER_ORDER` doc |
| `crates/cognicode-core/tests/provider_conformance.rs` | Modified | Java branch readiness pins |
| `.agent/TESTING-STATE.md` | Modified | Handoff record |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| CLI non-zero exit returns for missing files | Low | Pre-e39 intent; MCP shape unchanged; RED tests pin both directions |
| Cold-server first query degrades | Med | Later queries retry S2; bound configurable |

## Rollback Plan

Revert the single composite commit; conformance pin and TESTING-STATE revert with it. No data, schema, or spec change.

## Dependencies

e39 `87993f90`; verify report W2/W3.

## Success Criteria

- [ ] Missing/unreadable file → `Err(FileNotFound)` (definition AND hover); invalid location → `Err(InvalidLocation)`; clean serverless miss → `Ok(None)`.
- [ ] Hierarchy falls through within the bound without a ready server (elapsed << 30s), diagnostic recorded, counters correct; no-fallback policies wait fully.
- [ ] Guards green: `just lsi-providers`, `lsi-fixtures check` 42/42, `lsi-equivalence`, `lsi-identity`; `--lib composite`, `--lib code_intelligence`, checks, lint/fmt/clippy.
