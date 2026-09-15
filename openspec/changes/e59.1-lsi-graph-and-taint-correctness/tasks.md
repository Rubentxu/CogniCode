# Tasks — cycle e59.1 — graph + M5 taint correctness

> Cycle: A-lite | Milestone: M6 (entry to e60) | Date: 2026-09-15

### WU-1 — Graph exclusion during traversal
- `find_paths(..., excluded: &[&SubjectPattern])`; excluded nodes never entered.
- Remove the post-hoc path filter; add the `path_excluded` diagnostic only when
  no valid witness exists but unconstrained paths do.

### WU-2 — Multi-FLOW fail-loud
- `BackendError::UnsupportedIr`; `>1 FLOW` is an error, `0` is a diagnostic.

### WU-3 — M5 taint characterization (T1/T2)
- `crates/cognicode-graph-algos/tests/taint_characterization.rs` (black-box).

### WU-4 — Fix M5 `taint_forward`
- Re-enqueue on any new origin (T2).
- Constraint-aware witness reconstruction (T1).

## Acceptance gate
- T1/T2 pass.
- `cargo test -p cognicode-graph-algos` green (161 + 2 + 1).
- `python3 scripts/check_known_failures.py` exit 0 (no new failures).
- Graph tests: alternate clean path found; multi-FLOW fails loud.
- fmt clean; `cargo check --workspace --all-targets` 0 errors.
