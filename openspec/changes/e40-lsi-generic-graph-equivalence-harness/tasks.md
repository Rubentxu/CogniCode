# Tasks — e40 — GenericGraph equivalence harness

> Change: `e40-lsi-generic-graph-equivalence-harness` | Phase: tasks | Date: 2026-09-15

Single WU. ~340 LOC net. No review concerns; the file is pure
test-only code under `tests/`.

## Review Workload Forecast

```
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: single-commit
400-line budget risk: Low (under 400 LOC net)
```

## WU1 — `equivalence_harness/generic_graph.rs` (full coverage)

**Goal**: Create the sibling harness module + wire it into the parent
harness binary.

### Files

- `crates/cognicode-core/tests/equivalence_harness/generic_graph.rs` (NEW, ~340 LOC)
- `crates/cognicode-core/tests/equivalence_harness.rs` (modify: +3 LOC for include)

### Sub-steps

1. Implement helpers: `define_fact`, `contains_fact`, `call_fact`,
   `projection_digest`, `project_with`, `sample_facts`.
2. Implement scenario `rebuild_byte_identical` (REQ-EQGG-02).
3. Implement scenario `dangling_edge_skipped` (REQ-EQGG-03).
4. Implement scenario `kind_multiset_matches` (REQ-EQGG-05).
5. Implement scenario `empty_facts_yields_empty_projection` (REQ-EQGG-06).
6. Implement scenario `pinned_digest_matches` (REQ-EQGG-07). Seed digest from
   actual run output.
7. Implement scenario `helper_kinds_are_canonical` (compile-time sanity).
8. Wire `#![cfg(all(feature = "evidence-kernel", feature = "multimodal"))]`
   (REQ-EQGG-08).
9. Add `#[path = ...] mod generic_graph;` to parent harness (REQ-EQGG-09).
10. Run `cargo test -p cognicode-core --test equivalence_harness --features evidence-kernel,multimodal`
    — all scenarios MUST be green.

### Verification

```
cargo test -p cognicode-core --test equivalence_harness --features evidence-kernel,multimodal
# Expected: 13 passed; 0 failed (8 existing + 5 new generic_graph scenarios + 0 diff)
```

### Risk

- None. Pure test code. Any failure signals a regression in
  `build_generic_projection` (which is the goal of the harness).
- The structural fingerprint is a *new* contract surface; REQ-EQGG-07
  pins it. Future regression that touches the projection MUST consciously
  re-pin the digest.

### Rollback

`git revert <commit>` — no migration, no schema, no runtime state.
