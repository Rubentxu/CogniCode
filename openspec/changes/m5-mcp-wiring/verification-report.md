# Verification Report — M5.2 — Wire M5 algorithm IDs into rmcp_adapter

> Cycle: `p-c1fac1fea05615c6/m5-mcp-wiring`
> Path: **A-min**
> Phase: **verification** (closed)
> Date: 2026-09-14
> CWD: `/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode`

## Verdict

**PASS** — 8/8 `m5_program_analysis_roundtrip` tests pass, replay contract holds, 0 regressions vs M5.2 baseline. The 24 pre-existing MCP failures in `file_ops_handlers` / `refactor_handlers` are explicitly verified as NOT introduced by M5.2 (verified by stashing WU1+WU2+WU3 changes and re-running on WU1's parent commit `079d616a~1` = `2f0d48f5`; same 24 failures).

## Test evidence

| Suite | Result |
|---|---|
| `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis` | 35 passed (22 → 35: +5 WU1 + +8 WU3) |
| `cargo test -p cognicode-core --features program-analysis-server --lib mcp_roundtrip_tests::tests::m5_program_analysis_roundtrip` | 8 passed (NEW) |
| `cargo test -p cognicode-core --features program-analysis-server --lib mcp_roundtrip_tests::tests::tool_surface_parity` | 4 passed (allowlist extension) |
| `cargo test -p cognicode-graph-algos --lib` | 161 passed |
| `cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings` | exit 0 |
| `cargo fmt --check` | exit 0 |
| `wc -l crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` | 2549 (cap: 2650 per amended REQ-MCP-06) |
| Domain purity: `grep -rn "tokio\|sqlx\|reqwest" crates/cognicode-core/src/domain/analytics/program_analysis/` | 0 matches |

## Replay contract verification (SCN-MCP-02)

`test_cfg_per_function_digest_matches_conformance` passes. The MCP wire layer produces byte-identical output to direct dispatch for the canonical fixture. SHA-256 digests match.

## Pre-existing failures verified unrelated

24 failures in `file_ops_handlers`, `refactor_handlers` (and subset of `mcp_roundtrip_tests`) are pre-existing path canonicalization issues between `/home` and `/var/home`. Verified by:
1. Stashing WU1+WU2+WU3 changes
2. Re-running tests on `079d616a~1` (= `2f0d48f5`, end of M5 program-analysis-core)
3. Same 24 failures observed
4. Restoring WU1+WU2+WU3

Conclusion: not introduced by M5.2.

## Known gap (deferred to M5.1)

Tree-sitter AST lifting to `FunctionLocalView` so the canonical corpus can be sourced from real source instead of synthetic flat slices. Tracked outside M5.2.

## Cycle state at handoff

Verification phase complete. This report is a retrospective artifact created at housekeeping time after the cycle was already closed in the ledger.