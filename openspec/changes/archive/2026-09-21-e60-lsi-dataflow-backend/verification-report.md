# Verification Report — cycle e60 — dataflow backend (M5 adapter)

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e60 — dataflow backend (M5 adapter) |
| Path | A-lite |
| Base HEAD | `b465b2e0` (post-e59.1) |
| Verify verdict | **PASS** |

## Verification

### V-1 — Tests

```
cargo test -p cognicode-core --lib domain::findings        # 97 passed
cargo test -p cognicode-core --lib application::findings   # 8 passed
cargo test -p cognicode-core --test findings_ast_e2e       # 6 passed
cargo test -p cognicode-core --test findings_graph_e2e     # 4 passed
cargo test -p cognicode-core --test findings_dataflow_e2e  # 5 passed
```

### V-2 — The four invariants

| Invariant | Test |
|-----------|------|
| `capabilities == {Dataflow}`, `ceiling == B` (no `GraphQuery`) | `capabilities_and_ceiling_are_dataflow_only`, `u41_dataflow_backend_advertises_only_dataflow` |
| `source → a → b → sink` becomes `DataflowPath` B with `Source/Flow/Flow/Sink`, all one evidence id | `builds_source_flow_sink_causal_chain`, `u41_dataflow_flows_through_the_unchanged_seam` |
| A path M5 eliminated by `untaint` cannot reappear | `u41_sanitized_path_cannot_reappear`, `maps_untaints_to_excluded_subjects` |
| M5 errors stay errors (never an empty outcome) | `engine_errors_stay_errors` (`BackendError::Analysis`) |

### V-3 — Reuse proof (U7)

`calls_the_m5_engine_once_with_the_expected_sites` uses a counting spy:
`TAINT_FLOW` is invoked exactly once per qualifying function and the request
carries exactly the expected `sources = [1]`, `sinks = [3]`, `untaints` and a
non-empty `dfg_digest`. `real_program_analysis_service_is_used_end_to_end`
additionally composes the **real** service.

### V-4 — Planning (U8/U9)

- With AST + Graph + Dataflow registered and a `{Dataflow}` detector, the
  planner selects `m5_dataflow`.
- A `{Dataflow, SymbolicFeasibility}` detector fails loud (`ExecutionError::Plan`)
  rather than degrading.

### V-5 — Build / format / regressions

- `cargo check --workspace --all-targets` → 0 errors.
- `cargo fmt --all --check` → clean.
- `python3 scripts/check_known_failures.py` → exit 0 (41 baseline unchanged) —
  the M5 JSON `dispatch` behaviour is preserved by delegation.

## Conclusion

AST, Graph and Dataflow now traverse the **same** `admission → planner →
backend → outcome → evidence → assembler → verifier → gate` core with no
branch added to `DetectorExecutor`, `FindingAssembler` or `FindingVerifier`.
PASS.
