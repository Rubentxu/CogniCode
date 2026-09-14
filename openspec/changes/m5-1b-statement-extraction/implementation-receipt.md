# M5.1b — Statement-Level Extraction: Implementation Receipt

**WU3 of 3** | Commits: `28dce397` (WU1), `6b8bee7f` (WU2) → `b39162e4` (WU3)

## What was implemented

Five conformance acceptance tests (REQ-STMT-08..11) that prove each deferred M5.1
algorithm can dispatch on real, lifted source code and produce the same result as
the synthetic corpus.

### `ast_lift.rs` changes

| Symbol | Kind | Description |
|--------|------|-------------|
| `SLICE_FORWARD_SRC` | `const` | 4-statement linear source for forward slice |
| `SLICE_BACKWARD_SRC` | `const` | 5-statement diamond source for backward slice |
| `TAINT_LINEAR_SRC` | `const` | 3-statement linear source for taint analysis |
| `DOMINATORS_DIAMOND_SRC` | `const` | 5-statement diamond source for dominators |
| `digest_hex(body: &Value) -> String` | helper fn | SHA-256 hex of canonical JSON |
| `run_lifted(svc, alg_id, params) -> Value` | helper fn | Lift + dispatch in one call |
| `conformance_cfg_per_function_matches_synthetic` | test | REQ-STMT-09 |
| `conformance_dominators_matches_synthetic` | test | REQ-STMT-08 |
| `conformance_slice_forward_matches_synthetic` | test | REQ-STMT-10 |
| `conformance_slice_backward_matches_synthetic` | test | REQ-STMT-08 |
| `conformance_taint_flow_matches_synthetic` | test | REQ-STMT-11 |

### `conformance.rs` changes

| Change | Reason |
|--------|--------|
| `diamond_backward_slice` fixture: `variable: "x"` → `"a"` | Diamond source defines `a`, not `x`; test must use consistent variable |

### Algorithmic coverage

| Algorithm | Fixture | Result |
|-----------|---------|--------|
| `cfg_per_function` | `diamond_branch_four_blocks` | PASS — digest matches synthetic |
| `dominators_cfg` | `diamond_dominators` | PASS — added missing `cfg_digest` param |
| `slice_forward` | `linear_forward_slice` | PASS — digest matches synthetic |
| `slice_backward` | `diamond_backward_slice` | PASS — variable aligned to `a` |
| `taint_flow` | `linear_taint` | PASS — digest matches synthetic |

### Verification

```bash
cargo test -p cognicode-core --features program-analysis-server \
  --lib program_analysis::ast_lift 2>&1 | tail -3
# test result: ok. 21 passed; 0 failed
```

### Pre-existing issues (not introduced by WU3)

- `mcp_roundtrip_tests.rs:997`: imports `handle_interproc_summary` which does not
  exist in `program_analysis_handlers` — pre-existing broken import
- `INTERPROC_SUMMARY` unused in `interproc/mod.rs` — pre-existing
- `run_dfg` method never used — pre-existing

### Architectural notes

- Each conformance test lifts the real source, dispatches to the algorithm, then
  replays the same dispatch against the synthetic corpus fixture and asserts
  `digest == harness[0].digest_a`
- The `run_lifted` helper avoids duplicating the lift+dispatch boilerplate in every test
- `dominators_cfg` requires `cfg_digest` in params even though the stub does not use it;
  this is the validation contract defined in `dominators_cfg_descriptor.rs`
