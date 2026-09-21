# Verification Report — cycle e60.1 — semantic contract tightening

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e60.1 — semantic contract tightening |
| Path | A-lite |
| Base HEAD | `5516d63e` (post-e60) |
| Verify verdict | **PASS** |

## Verification

### V-1 — Tests

```
cargo test -p cognicode-core --lib domain::findings        # 98 passed
cargo test -p cognicode-core --lib application::findings   # 13 passed
cargo test -p cognicode-core --test findings_ast_e2e       # 6 passed
cargo test -p cognicode-core --test findings_graph_e2e     # 4 passed
cargo test -p cognicode-core --test findings_dataflow_e2e  # 7 passed
```

### V-2 — C1/C2 (FLOW.source authoritative)

| Property | Test |
|----------|------|
| Request sources contain only the `FLOW.source` site | `request_sources_are_only_the_flow_source` |
| An unrelated `MATCH` cannot reach the sink | `unrelated_match_subject_cannot_seed_a_traversal`, E2E `u41_unrelated_match_subject_cannot_seed_a_traversal` |
| Undeclared `FLOW.source` is rejected at admission | `flow_source_must_be_a_declared_match_subject` (V11) |

### V-3 — C4/C5 (limits)

| Property | Test |
|----------|------|
| `max_path_count` exceeded ⇒ error | `exceeding_the_path_budget_is_an_error_not_a_truncation` |
| `max_visited_nodes` exceeded ⇒ error | `exceeding_the_node_budget_is_an_error` |
| Within budget the paths are reported | `within_budget_two_paths_are_reported` |
| End-to-end: `ExecutionError::Backend(Analysis)` | `u41_exceeded_limits_stay_execution_errors` |

### V-4 — C6 + hygiene

- e60's design contract corrected; correction note added.
- `cargo check --workspace --all-targets` → 0 errors; `cargo fmt --all --check`
  → clean; known-failure baseline unchanged (41).

## Conclusion

`MATCH` no longer influences reachability in any backend, an incoherent
`MATCH`/`FLOW` pair cannot be admitted, and the declared `PlanLimits` are
genuinely enforced where checkable (erroring, not truncating) with the rest
documented honestly. The AST/Graph/Dataflow contract can be frozen. PASS.
