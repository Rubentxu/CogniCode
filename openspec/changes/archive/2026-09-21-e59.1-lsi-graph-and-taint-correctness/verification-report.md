# Verification Report — cycle e59.1 — graph + M5 taint correctness

> Cycle: A-lite | Milestone: M6 (entry to e60) | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e59.1 — graph + M5 taint correctness |
| Path | A-lite |
| Base HEAD | `a298ab0b` (post-e59) |
| Verify verdict | **PASS** |

## Verification

### V-1 — M5 characterization (T1/T2)

```
cargo test -p cognicode-graph-algos --test taint_characterization
```

**Before the fix:** both FAILED (2 failed) — confirming two real M5 defects.
**After the fix:** **2 passed**.

### V-2 — Graph correctness

```
cargo test -p cognicode-core --lib domain::findings::graph_backend   # 7 passed
cargo test -p cognicode-core --test findings_graph_e2e              # 4 passed
```

| Test | Property |
|------|----------|
| `finds_an_alternate_clean_path_when_the_shortest_witness_is_sanitized` | exclusion during traversal |
| `multiple_flow_steps_fail_loud` | `BackendError::UnsupportedIr` |
| `excludes_a_path_through_a_sanitizer` | no witness ⇒ diagnostic |

### V-3 — No regressions

- `cargo test -p cognicode-graph-algos` → 161 + 2 + 1 passed.
- `cargo test -p cognicode-core --lib taint` → 6 passed.
- `python3 scripts/check_known_failures.py` → **exit 0**, 41 baseline entries
  matched exactly (no new failures caused by the M5 change).
- `cargo check --workspace --all-targets` → 0 errors; `cargo fmt --all --check`
  → clean.

### V-4 — Boundary preserved

The two taint defects were fixed in `cognicode-graph-algos`, **not** in M6.
M6 continues to import analysis primitives.

## Conclusion

Graph produces only *valid* causal witnesses, multi-FLOW fails loud, and the
M5 taint engine now yields both (a) true causal witnesses (T1) and (b) every
origin's path through a merge (T2). This is the evidence basis e60's dataflow
adapter needs. PASS.
