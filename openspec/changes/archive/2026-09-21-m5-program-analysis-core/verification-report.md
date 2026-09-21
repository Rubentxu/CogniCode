# Verification Report — M5 — Program Analysis Core

> Cycle: `p-c1fac1fea05615c6/m5-program-analysis-core`
> Path: **A-lite**
> Phase: **verification** (closed)
> Date: 2026-09-14
> CWD: `/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode`

## Verdict

**PASS_WITH_WARNINGS** — all 6 algorithm IDs COMPLIANT, harness determinism proven, 1 warning (bench target pre-existing unused_imports, NOT introduced by M5).

## Test evidence

| Suite | Result |
|---|---|
| `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis` | 22 passed (16 → 22: +4 acceptance_evidence + +1 interproc canonical fixture) |
| `cargo test -p cognicode-graph-algos --lib` | 161 passed, 0 failed |
| `cargo check --workspace --all-features` | exit 0 |
| `cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings` | exit 0 |
| `cargo fmt --check` | exit 0 |
| Domain purity: `grep -rn "tokio\|sqlx\|reqwest" crates/cognicode-core/src/domain/analytics/program_analysis/` | 0 matches |

## What the acceptance evidence (WU7) covers

- `public_dispatcher_accepts_every_m5_algorithm_id` — runs the canonical corpus through the **real** `ProgramAnalysisService::dispatch(id, params, limits)` surface (not just unit tests on the algorithms). Every required M5 id (`cfg_per_function`, `dominators_cfg`, `slice_forward`, `slice_backward`, `taint_flow`, `interproc_summary`) returns `Ok`. Negative invariant: unknown id → `Err`.
- `replay_guard_is_byte_identical_for_entire_corpus` — determinism contract verified at the dispatcher boundary.
- `perf_envelope_publishes_median_p95_max` — median ≤ max, p95 ≤ max, over_budget_count = 0.
- `mcp_fixture_report_serde_round_trips` — the MCP-facing JSON shape is serialize-stable.

## Explicit constraint (NOT a gap, recorded honestly)

The canonical corpus today is fed **synthetic flat slices**, not parsed tree-sitter ASTs. The harness module's own docs (`conformance.rs`) explicitly note: "The corpus is synthetic today — the inputs are flat slices the algorithms already accept. A later M5.1 slice lifts these inputs from tree-sitter ASTs; the fixture shape will then change to 'source + snapshot id' instead of 'raw adjacency + statements'." This is by design: M5 scope per ROADMAP is the algorithm layer; tree-sitter → `FunctionLocalView` lifting is M5.1.

## Pre-existing failures documented

- `fact_bridge_benchmarks.rs` — pre-existing `unused_imports` warning on bench target. NOT introduced by M5. Same warning present on WU1 baseline.

## Known gap (deferred)

Wire ProgramAnalysisService descriptors into `rmcp_adapter/mod.rs` (currently the `SlicingBackwardAdapter` alias is registered but the 6 algorithm IDs are not yet exposed as MCP tools). Tracked outside M5, addressed in M5.2 (`m5-mcp-wiring`).

## Cycle state at handoff

Verification phase complete. This report is a retrospective artifact created at housekeeping time after the cycle was already closed in the ledger.