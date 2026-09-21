# Implementation Receipt — m5-1b-debt-cleanup

> Cycle: `p-c1fac1fea05615c6/m5-1b-debt-cleanup`
> Phase: build (complete)
> Path: **A-min**
> HEAD: `aaa821c4d8f148bfdb19c1c334dadc6cc51c9f0d` (2 stacked commits on `main`)

## Goal

Close the 2 follow-up items flagged by the upstream `m5-1b-statement-extraction`
verify report (PASS_WITH_WARNINGS, 2026-09-14).

## Work Units

### WU1 — `ast_lift.rs` production code ≤ 545 LOC

**Commit**: `5cb4a644` \
**refactor(ast_lift): split tests into ast_lift_tests.rs to meet REQ-LIFT-06 LOC cap**

- `crates/cognicode-core/src/application/program_analysis/ast_lift.rs`:
  trimmed from 845 LOC to **181 LOC** (production code only).
- `crates/cognicode-core/src/application/program_analysis/ast_lift_tests.rs`:
  NEW, 676 LOC (includes a 13-line module-doc header explaining the split,
  plus the original 663-line `mod tests { ... }` body).
- Inclusion: `#[cfg(test)] #[path = "ast_lift_tests.rs"] mod tests;` — uses
  the same `#[path]` pattern already present elsewhere in the workspace
  (`crates/cognicode-cli/src/bin/cogh.rs`).

### WU2 — `mcp_roundtrip_tests` compiles without `program-analysis-server`

**Commit**: `aaa821c4` \
**fix(mcp): cfg-gate handle_interproc_summary import for non-feature builds**

- `crates/cognicode-core/src/interface/mcp/mcp_roundtrip_tests.rs:993-1000`:
  moved `handle_interproc_summary` from the multi-symbol `use` group into a
  separate `use` statement under `#[cfg(feature = "program-analysis-server")]`.
  All existing usages (line 1124, 1130) were already behind the same cfg gate.
- The other handlers in the original group (`handle_cfg`, `handle_slice_forward`,
  `handle_taint_flow`, `ProgramAnalysisToolInput`) remain unconditional.

## Evidence (orchestrator re-check, 2026-09-14)

### REQ-DC-01 — Split test bodies

| Check | Result |
|---|---|
| `wc -l ast_lift.rs` (production) | **181 LOC** (cap 545) ✓ |
| `wc -l ast_lift_tests.rs` (tests) | 676 LOC |
| `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis::ast_lift` | **21 / 21 passed** ✓ |
| `cargo test -p cognicode-core --features program-analysis-server --lib ingest::extractor::tests` | **9 / 9 passed** ✓ |

### REQ-DC-02 — Compile in both feature configurations

| Check | Result |
|---|---|
| `cargo test --lib -p cognicode-core --no-run` (feature OFF) | exit 0 ✓ |
| `cargo test --lib -p cognicode-core --features program-analysis-server --no-run` (feature ON) | exit 0 ✓ |
| `cargo test -p cognicode-core --features program-analysis-server --lib m5_program_analysis_roundtrip` | **8 / 8 passed** ✓ |

### REQ-DC-03 — Lint, fmt, forbidden imports

| Check | Result |
|---|---|
| `cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings` | exit 0 (no findings) ✓ |
| `cargo fmt --check` | exit 0 (no diff) ✓ |
| `grep -rn -E '^\s*(use\s+(tokio\|sqlx\|reqwest)\|extern crate …)'` over the 3 changed files | **0 matches** ✓ |

> Note: the over-broad grep `grep -rn 'tokio\|sqlx\|reqwest'` over
> `mcp_roundtrip_tests.rs` matches pre-existing `#[tokio::test]` attributes
> (test runtime, not domain code). The AGENTS.md rule targets domain imports;
> this cycle introduces none. The `use`-statement-only grep is the
> authoritative check and reports 0.

## Behavior deltas (intended, scoped to verification surface)

1. **No production behavior change in `ast_lift`**: tests are still gated
   `#[cfg(test)]` and included via `mod tests;`; only the file location changed.
2. **Compilation works without `program-analysis-server`**: the
   `handle_interproc_summary` import is now cfg-gated. Test usage at line
   1124 was already behind the same cfg gate.

## Stale evidence

None — all targeted suites were re-run after the change; nothing else is
regressed. Pre-existing failures in `workspace_session`, `file_ops_handlers`,
`refactor_handlers` and a handful of `mcp_roundtrip_tests::tests::{complexity,
edge_cases, file_operations_roundtrip}` are documented in the
`m5-1b-statement-extraction` cycle handoff (TESTING-STATE.md) and confirmed
unrelated to this cycle (no overlap with the 3 changed files).

## Next

→ evaluate `phase.build.complete` → spawn `sddk-verify`.
