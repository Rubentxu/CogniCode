# Verification Report — cycle e62.2 — analysis scope pinning (U42, part 1)

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e62.2 — analysis scope pinning |
| Path | A-lite |
| Base HEAD | `ce2196f4` (post-e62.1) |
| Verify verdict | **PASS** |

## Verification

### V-1 — Tests

```
cargo test -p cognicode-core --lib domain::findings        # 101 passed
cargo test -p cognicode-core --lib application::findings   # 21 passed
cargo test -p cognicode-core --test findings_ast_e2e       # 6 passed
cargo test -p cognicode-core --test findings_graph_e2e     # 4 passed
cargo test -p cognicode-core --test findings_dataflow_e2e  # 7 passed
cargo test -p cognicode-core --test findings_axiom_import_e2e  # 3 passed
```

### V-2 — The decisive property

`scope_mismatch_is_rejected_before_any_id_is_resolved`:

| Scenario | Result |
|----------|--------|
| finding(scope A) + lookup(scope A) | `verify_for_gate` **Ok** |
| finding(scope A) + lookup(scope B), same ids | `Err(ScopeMismatch)` |
| finding(scope A) + lookup(scope B) | `can_block == false` |

The two snapshots deliberately share the numeric evidence id (and the fact id),
so success cannot come from the numbers differing.

### V-3 — Scope is captured, and required

- An E2E assertion checks `finding.detector.scope.snapshot == Snap(1)`.
- `DetectorExecutor::execute` returns `ExecutionError::MissingScope` when the
  input carries no scope (covered by the executor tests, which now supply one).

### V-4 — Build / format / regressions

- `cargo check --workspace --all-targets` → 0 errors; `cargo fmt --all --check`
  → clean.
- known-failure baseline unchanged (41).

## Conclusion

A finding can no longer be verified against another snapshot's read model, even
when every id coincides. This is the property the user asked for as the
"decisive negative". PASS.
