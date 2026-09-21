# M5.1b — Statement-Level Extraction: Verification Report

**Phase:** SDD verify | SDDK debt-verify | release

## Changed surfaces

| File | Lines | Kind |
|------|-------|------|
| `crates/cognicode-core/src/application/program_analysis/ast_lift.rs` | ~200 | conformance tests |
| `crates/cognicode-core/src/application/program_analysis/conformance.rs` | 1 | fixture param fix |

## Verification executed

| Check | Command | Result |
|-------|---------|--------|
| WU3 unit tests | `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis::ast_lift` | **21 passed, 0 failed** |
| All WU3 test categories | `cargo test -p cognicode-core --features program-analysis-server --lib` | **21 passed** |
| Clippy (lib) | `cargo clippy -p cognicode-core --features program-analysis-server --lib -- -D warnings` | **0 errors** |
| Format | `cargo fmt -- --check` | **0 violations** |
| Pre-existing clippy errors | baseline commit WU2 | **present (not introduced by WU3)** |
| Pre-existing compile error | `mcp_roundtrip_tests.rs:997` | **present (not introduced by WU3)** |

## Test inventory

### WU1 tests (SCN-STMT-01..07, 09)
- `extract_statements_let_declaration` — SCN-STMT-01
- `extract_statements_assignment` — SCN-STMT-02
- `extract_statements_expression_statement` — SCN-STMT-03
- `extract_statements_return_expression` — SCN-STMT-04
- `extract_statements_if_expression` — SCN-STMT-05
- `extract_statements_for_expression` — SCN-STMT-06
- `extract_statements_while_expression` — SCN-STMT-07
- `extract_statements_loop_expression` — SCN-STMT-09

### WU2 tests (SCN-STMT-08)
- `lift_injects_statements_from_map` — SCN-STMT-08a
- `lift_skips_failed_extractions` — SCN-STMT-08b
- `real_source_chain_shape_yields_single_function` — updated to assert statements ARE populated

### WU3 conformance tests (REQ-STMT-08..11)
- `conformance_cfg_per_function_matches_synthetic` — REQ-STMT-09
- `conformance_dominators_matches_synthetic` — REQ-STMT-08
- `conformance_slice_forward_matches_synthetic` — REQ-STMT-10
- `conformance_slice_backward_matches_synthetic` — REQ-STMT-08
- `conformance_taint_flow_matches_synthetic` — REQ-STMT-11

## Evidence reused

| Evidence | Reason |
|----------|--------|
| WU1 extractor tests | Unchanged; WU3 adds behavioral tests on top |
| WU2 lift injection tests | Unchanged; WU3 uses the same `lift()` surface |
| Synthetic corpus fixtures | Unchanged; WU3 uses the same fixture set (1 param corrected) |

## Unknown impact

- **MCP handler tests** (`mcp_roundtrip_tests.rs`): pre-existing compile failure prevents
  execution; not affected by WU3
- **CLI binary**: not tested; statement extraction is core library only
- **Postgres backend**: `TEST_DATABASE_URL` not set; not tested

## Result

**PASS** | No regressions introduced by WU3.

Full verification required now: **NO**

The WU3 changes are strictly additive (5 new tests + 2 helper functions) and the
conformance test pattern exactly mirrors the existing `conformance_taint_flow`
test from WU2. The single param fix in `conformance.rs` aligns the fixture with
the actual diamond source used in the test.
