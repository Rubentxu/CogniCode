# M5.1 Archive Manifest

> Cycle: `p-c1fac1fea05615c6/m5-1-ast-lifting`
> Phase: archive (closes cycle)
> Date: 2026-09-14

## Cycle summary

| Property | Value |
|---|---|
| Cycle ID | `p-c1fac1fea05615c6/m5-1-ast-lifting` |
| Path | A-full |
| Phases completed | explore → specify → design → tasks → build → verify → release → archive |
| Final status | RELEASED (closes after archive) |
| Head SHA | `98ae749a` |
| Tag | `m5-1-ast-lifting@v1` |
| Merge receipt SHA | `98ae749a` |
| Release receipt SHA | `98ae749a` |

## Deliverables shipped

### Production code
- `crates/cognicode-core/src/application/program_analysis/ast_lift.rs` (NEW, 522 LOC)
- `crates/cognicode-core/src/application/program_analysis.rs` (+3 LOC: feature-gated `pub mod ast_lift;`)

### Test coverage
- 14 new `ast_lift` tests (9 module unit + 5 real-source acceptance including chain-shape and determinism)
- 49 program_analysis tests total (+14 vs M5.2 close)
- 8 mcp_roundtrip tests (M5.2 regression check, unchanged)

### Openspec change artifacts
- `openspec/changes/m5-1-ast-lifting/specs/ast-lifting/spec.md` — 8 requirements (REQ-LIFT-01..08), 8 scenarios
- `openspec/changes/m5-1-ast-lifting/design.md`
- `openspec/changes/m5-1-ast-lifting/tasks.md`
- `openspec/changes/m5-1-ast-lifting/implementation-receipt.md`
- `openspec/changes/m5-1-ast-lifting/verification-report.md`

### Commit chain (3 stacked-to-main commits)
1. `09467e27` — WU1: `ast_lift` module + module decl
2. `b06e7821` — WU2+WU3: real-source chain + determinism tests + M5.1 scope note
3. `98ae749a` — openspec change artifacts

## Acceptance contract — final state

| Requirement | Status | Evidence |
|---|---|---|
| REQ-LIFT-01 — pure value transform shape | satisfied | 9 module unit tests |
| REQ-LIFT-02 — function-or-method filter | satisfied | `lift_filters_non_function_symbols` test |
| REQ-LIFT-03 — statement.id per-function | DEFERRED to M5.1b | doc-comment documents the boundary |
| REQ-LIFT-04 — interproc_summary from real source | satisfied | `real_source_digest_for_interproc_matches_synthetic` test (gated) |
| REQ-LIFT-05 — coverage scope (1 of 6 in M5.1, +5 in M5.1b) | satisfied | `real_source_lift_mirrors_canonical_corpus_shape` test |
| REQ-LIFT-06 — honest empty-fallback | satisfied | `empty_real_source_returns_empty_view` test |
| REQ-LIFT-07 — no I/O imports, no new ports | satisfied | 0 `tokio`/`sqlx`/`reqwest` matches; clippy `-D warnings` clean |
| REQ-LIFT-08 — feature-gated + deterministic | satisfied | `real_source_lift_is_deterministic` test + `#[cfg]`-gated modules |

## Deferred scope (M5.1b, tracked)

Statement-level def/use extraction in `tree_sitter_facts` so the lift can
populate `FunctionLocalView.statements`. Once populated, M5.1b extends the
conformance harness to cover cfg/dominators/slice/taint from real source.

## Pre-existing unrelated failures documented

39 pre-existing test failures in `interface::mcp::{file_ops_handlers,
refactor_handlers, mcp_roundtrip_tests (complexity/edge_cases/file_operations),
security}` are documented in `verification-report.md` as unrelated to M5.1
(touched code is `ast_lift.rs` only).

## Cycle closure

This cycle closes upon successful `archive.complete` transition. M5.1 is **done**.
