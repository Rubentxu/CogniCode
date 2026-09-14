# Tasks: M5.1 — Lift tree-sitter extraction into FunctionLocalView

> Change: `m5-1-ast-lifting` | Cycle: `p-c1fac1fea05615c6/m5-1-ast-lifting` | Phase: tasks

3 stacked WUs, ~500 LOC net.

## WU1 — Lift module + module decl (~200 LOC)

**Goal**: Build `ast_lift::lift` as a pure value transform.

### Files
- `crates/cognicode-core/src/application/program_analysis/ast_lift.rs` (NEW, ≤ 200 LOC)
  - `pub fn lift(extractions: &[ExtractionResult]) -> (Vec<FunctionLocalView>, Vec<Vec<usize>>)`.
  - Private helpers: `function_indices_by_id`, `build_call_adjacency`,
    `extract_calls_from_edges`.
  - Per-function 0..N statement-id assignment (D3).
  - No `tokio`/`sqlx`/`reqwest` imports (D1, REQ-LIFT-07).
- `crates/cognicode-core/src/application/program_analysis.rs` (MODIFY, +1 line)
  - `pub mod ast_lift;`

### Tests in module
- `lift_produces_one_view_per_function`
- `lift_filters_non_function_symbols`
- `lift_assigns_per_function_statement_ids`
- `lift_on_empty_file_returns_empty`

## WU2 — Real-source fixtures + acceptance test (~250 LOC)

**Goal**: Embed inline Rust sources and assert digest equality with synthetic corpus.

### Files (in `ast_lift.rs::tests`)
- Inline `const LINEAR_CHAIN_SRC: &str = "fn linear() { let x = 1; let y = x; use(y); }";`
  plus 3-4 more (one per algorithm shape).
- Helper: `extract_inline(src: &str) -> ExtractionResult` that calls
  `crate::application::ingest::extractor::extract_one` (sync).
- Acceptance test `test_real_source_digest_matches_synthetic`:
  - Lift `LINEAR_CHAIN_SRC` → `FunctionLocalView[]`.
  - Build `serde_json::json!({ "function_id": "linear", "call_graph": [...], "functions": [...] })`.
  - Call `ProgramAnalysisService::dispatch(&CFG_PER_FUNCTION, …)`.
  - Assert the resulting digest equals the synthetic
    `linear_chain_three_blocks` digest (computed by re-running the
    conformance harness on the synthetic fixture).
- 3 more digest-equality tests for dominators_cfg, slice_forward, slice_backward.

### Feature gating
- All real-source tests `#[cfg(feature = "program-analysis-server")]`.
- Existing synthetic tests unchanged.

## WU3 — docs + change artifacts (~50 LOC)

**Goal**: Track the change in the standard artifacts.

### Files
- `openspec/changes/m5-1-ast-lifting/specs/ast-lifting/spec.md` (already done in WU1 of design phase)
- `openspec/changes/m5-1-ast-lifting/design.md` (already done)
- `openspec/changes/m5-1-ast-lifting/tasks.md` (this file)
- `crates/cognicode-core/src/application/program_analysis/ast_lift.rs` module-level
  doc-comment update with the honest "empty statements[] for now" note (D2).

### Acceptance gate (across all 3 WUs)
- `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis`:
  - without feature → 22 (current M5 baseline) + 5 (WU1 module tests) + 8 (M5.2 roundtrip) = 35 still green
  - with feature → 35 + ~4 acceptance tests + ~2 helper tests = ~41
- `cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings` → exit 0
- `cargo fmt --check` → exit 0
- grep `tokio`/`sqlx`/`reqwest` under `application/program_analysis/ast_lift.rs` → 0 matches

## Commit chain

```
<sha1> WU1 lift module + module decl
<sha2> WU2 real-source fixtures + acceptance test
<sha3> WU3 docs (already in place) + module doc-comment update
```

All three land as stacked-to-main commits.
