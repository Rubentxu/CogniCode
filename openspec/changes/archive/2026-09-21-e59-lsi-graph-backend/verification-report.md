# Verification Report — cycle e59 — graph detector backend

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e59 — graph detector backend (+ WU-0 promotion binding) |
| Path | A-lite |
| Base HEAD | `433e3397` (post-e58.2) |
| Verify verdict | **PASS** |

## Verification

### V-1 — Tests

```
cargo test -p cognicode-core --lib domain::findings      # 95 passed
cargo test -p cognicode-core --test findings_ast_e2e     # 6 passed
cargo test -p cognicode-core --test findings_graph_e2e   # 4 passed
```

### V-2 — WU-0 promotion binding

| Property | Test |
|----------|------|
| Source cannot be rewritten | `promotion_is_bound_to_the_admission_source` |
| Version/logic binding | `promotion_is_bound_to_version_and_logic` |
| Policy change ⇒ new approval | `policy_change_requires_a_new_approval` |
| Rename keeps approval valid | `renaming_does_not_invalidate_an_approval` |
| Wrong permit's target rejected | `promote_rejects_a_target_from_another_permit` |
| Non-empty approver | `promotion_requires_a_non_empty_approver` |

### V-3 — Graph end-to-end (U41)

`tests/findings_graph_e2e.rs`:

| Test | Property |
|------|----------|
| `u41_graph_flow_through_the_unchanged_seam` | planner picks `graph`; class B; `Source→Flow→Sink` causal chain; verifies; Candidate cannot block |
| `u41_graph_promoted_detector_blocks` | promotion ⇒ blocks |
| `u41_excluded_path_produces_no_finding` | sanitizer on the path ⇒ no finding + diagnostic |
| `u41_planning_rejects_a_detector_the_graph_backend_cannot_run` | multi-capability detector fails loud; `GraphBackend` advertises only `GraphQuery`, ceiling B |

### V-4 — Build / purity / format / kernel / checker

- `cargo check --workspace --all-targets` → 0 errors.
- `cargo fmt -p cognicode-core --check` → clean.
- Domain purity: no I/O imports in `domain/findings/`.
- Gated kernel: 82 tests green.
- `scripts/check_known_failures.py` → exit 0.

## Conclusion

The seam generalises to a second analysis paradigm **without any special
case**: graph traversal, evidence class, causal chain, assembly, verification
and the gate all reuse the e57–e58.2 contract. PASS.
