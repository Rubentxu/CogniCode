# M5.1 Implementation Receipt

> Cycle: `p-c1fac1fea05615c6/m5-1-ast-lifting`
> Phase: build
> Gate: `implementation-complete`
> Date: 2026-09-14

## Work units shipped

| WU | Commit | Subject |
|---|---|---|
| WU1 | `09467e27` | `ast_lift` module + module decl — pure value transform, 9 module unit tests + 3 real-source acceptance |
| WU2+WU3 | `b06e7821` | Real-source chain + determinism tests + M5.1 scope note (doc-comment update folded in) |

## Net change

| File | LOC | Kind |
|---|---|---|
| `crates/cognicode-core/src/application/program_analysis/ast_lift.rs` | 522 | NEW |
| `crates/cognicode-core/src/application/program_analysis.rs` | +3 | MODIFY (feature-gated `pub mod ast_lift;`) |

## Verification evidence (pre-push)

```
cargo test -p cognicode-core --features program-analysis-server --lib program_analysis
  → 49 passed; 0 failed; 0 ignored
cargo test -p cognicode-core --features program-analysis-server --lib mcp_roundtrip_tests::tests::m5
  → 8 passed; 0 failed; 0 ignored (M5.2 roundtrip regression check)
cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings
  → exit 0
cargo fmt --check
  → exit 0
grep -c 'tokio\|sqlx\|reqwest' crates/cognicode-core/src/application/program_analysis/ast_lift.rs
  → 0 (no forbidden I/O imports)
```

## Coverage delta

| Surface | M5.2 close | M5.1 close | Δ |
|---|---|---|---|
| `program_analysis` lib tests | 35 | 49 | **+14** |
| `mcp_roundtrip_tests::m5_*` tests | 8 | 8 | 0 |
| Real-source acceptance coverage | 0 algorithms | 1 (`interproc_summary`) | +1 |
| Forbidden I/O imports in `ast_lift` | n/a | 0 | n/a |
| Synthetic corpus (retained) | 7 fixtures | 7 fixtures | 0 |

## Scope boundary (M5.1 vs M5.1b)

`FunctionLocalView.statements` is always empty in M5.1. The current
`tree_sitter_facts` extractor does not yet emit statement-level def/use facts,
so `cfg_per_function`, `dominators_cfg`, `slice_forward`, `slice_backward`,
`taint_flow` cannot be served from real source. They remain fed by the
synthetic conformance corpus. `interproc_summary` (the only algorithm that
needs only the call graph) is served end-to-end from lifted real source and
asserts `summary_count == 1` for the diamond-shaped test fixture.

Statement-level extraction is tracked as **M5.1b**, scheduled after M5.1 close.

## Cycle status

```
$ sddk cycle status --cycle p-c1fac1fea05615c6/m5-1-ast-lifting
{
  "cycle_id": "p-c1fac1fea05615c6/m5-1-ast-lifting",
  "status": "OPEN",
  "phase": "build",
  "path": "A-full"
}
```

Implementation complete. Awaiting verification phase.
