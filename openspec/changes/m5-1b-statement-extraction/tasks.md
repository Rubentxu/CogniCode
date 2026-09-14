# Tasks: M5.1b — Statement-level extraction to enable all 6 algorithm shapes from real source

> Change: `m5-1b-statement-extraction` | Cycle: `p-c1fac1fea05615c6/m5-1b-statement-extraction` | Phase: tasks

3 stacked WUs, ~325 LOC net.

## Review Workload Forecast

```
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: stacked-to-main
400-line budget risk: Low
```

## WU1 — `ExtractionResult.statements_by_function` + statement walker (~160 LOC)

**Goal**: Grow `ExtractionResult` with the per-function statement map and implement the Rust statement walker in the extractor.

### Files

- `crates/cognicode-core/src/application/ingest/types.rs` (MODIFY, +6 LOC)
  - Add `#[serde(default)]` to `ExtractionResult` struct (derives + Serialize + Deserialize).
  - Add `statements_by_function: BTreeMap<NodeId, Vec<Statement>>` field.
  - Initialize field to `BTreeMap::new()` in both `ExtractionResult::ok` and `ExtractionResult::failed`.

- `crates/cognicode-core/src/application/ingest/extractor.rs` (MODIFY, +150 LOC)
  - Add import: `cognicode_graph_algos::algorithms::Statement`.
  - New private fn `extract_statements_from_node(function_node: &Node, source: &[u8]) -> Vec<Statement>`.
    Handles: `let_declaration`, `assignment_expression`, `expression_statement`,
    `return_expression`, `if_expression`, `match_expression`, `match_arm`,
    `for_expression`, `while_expression`, `loop_expression`, `block`.
    defs/uses are sorted + deduped. Statement.id 0..N-1 in DFS order.
  - In `extract_file`: after building the symbol node for each function, call
    `extract_statements_from_node` wrapped in `std::panic::catch_unwind` (ADR-023).
    Insert non-empty results into `result.statements_by_function` keyed by `symbol_id`.
  - Add 7 module-level tests in `tests::statement_extraction` (SCN-STMT-01..07).

### Tests

- SCN-STMT-01: empty body → `Vec::new()`
- SCN-STMT-02: `let x = 1;` → 1 Statement with `defs=[x]`, `uses=[]`
- SCN-STMT-03: two statements chain → correct defs/uses per statement
- SCN-STMT-04: `return x + y;` → `defs=[]`, `uses=[x, y]` (sorted)
- SCN-STMT-05: `if` branch → 1 leaf Statement; `if` itself emits nothing
- SCN-STMT-06: Statement.id monotonicity 0..N-1
- SCN-STMT-07: byte-identical determinism on two extractions

## WU2 — Lift injects `FunctionLocalView.statements` from map + module tests (~45 LOC)

**Goal**: Connect the extracted statements into the lift output.

### Files

- `crates/cognicode-core/src/application/program_analysis/ast_lift.rs` (MODIFY, +45 LOC)
  - In `lift`: after Pass 1 (function discovery), Pass 2 (call graph), inject
    `statements` from `result.statements_by_function.get(&node.id).cloned()`
    into each `FunctionLocalView`. Fallback to `Vec::new()`.
  - Update `node_to_function_view` doc comment: no longer mentions "empty statements".
  - Update module doc: remove the M5.1 limitation note.
  - Add 2 tests in `tests::statement_lift` (SCN-STMT-08):
    - Lift populates `FunctionLocalView.statements` from map.
    - Lift falls back to empty `statements` when map entry is absent.

## WU3 — 5 conformance acceptance tests + change artifacts (~150 LOC)

**Goal**: Prove all 5 deferred algorithms now run on real source with matching digests.

### Files

- `crates/cognicode-core/src/application/program_analysis/ast_lift.rs::tests::real_source` (MODIFY, +150 LOC)
  - Add 5 new tests in `tests::real_source` (one per deferred algorithm, REQ-STMT-08):
    - `cfg_per_function` from linear chain source
    - `dominators_cfg` from diamond source
    - `slice_forward` from let-chain source
    - `slice_backward` from return-from source
    - `taint_flow` from source-sink source
  - Each test lifts inline Rust source → `FunctionLocalView[]` → dispatch → digest.
    Digest is compared against synthetic corpus baseline (same pattern as
    M5.1's `real_source_digest_for_interproc_matches_synthetic`).
  - Update `real_source_lift_mirrors_canonical_corpus_shape` test comment:
    `statements` is now populated (remove M5.1 limitation note).
  - Update `real_source_chain_shape_yields_single_function`: remove the
    `assert!(f.statements.is_empty(), "M5.1 lift must NOT emit statements")`
    assertion (statements ARE emitted after M5.1b).

### Acceptance gate (across all 3 WUs)

- `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis` → ~56 (49 baseline + ~7 new)
- `cargo test -p cognicode-core --features program-analysis-server --lib ingest::extractor` → ~8 (1 baseline + 7 new)
- `cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings` → exit 0
- `cargo fmt --check` → exit 0
- `grep -rn 'tokio\|sqlx\|reqwest' application/program_analysis/ast_lift.rs application/ingest/extractor.rs` → 0 matches
- `wc -l ast_lift.rs` ≤ M5.1 cap (519 + ~20 ≤ ~545)

## Commit chain

```
<sha1> WU1: types + extractor statement walker + 7 unit tests
<sha2> WU2: ast_lift injects statements + 2 module tests
<sha3> WU3: 5 conformance acceptance tests + test-comment updates
```

All three land as stacked-to-main commits (no PR — direct-to-main per AGENTS.md).
