# Tasks — e42 — CP-3 cross-producer edge cases

> Change: `e42-lsi-cp3-cross-producer-edge-cases` | Phase: tasks | Date: 2026-09-15

Single WU. ~262 LOC net. No review concerns; the file is pure
test-only code in a `#[cfg(test)] mod tests`.

## Review Workload Forecast

```
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: single-commit
400-line budget risk: Low (262 LOC, mostly mock providers)
```

## WU1 — Two RED-first tests for the CP-3 fallback contract

**Goal**: Add the two missing corner-case tests to
`lsp_facts::tests`, mirroring the mock-provider pattern from the
existing happy-path test.

### Files

- `crates/cognicode-core/src/application/fact_bridge/lsp_facts.rs` (+262 LOC, tests only)

### Sub-steps

1. Add `RelationKind` to the `mod tests` imports (was missing).
2. Add `SnapshotEntityView` to the `mod tests` imports (was missing).
3. Write test `cp3_unreported_container_falls_back_to_file_path`:
   - Mock `NoContainerObserver` returning `container: None`.
   - Pre-declare `src/lib.rs:thing:1` via `add_observation`.
   - Build snapshot, run `SnapshotEntityView::from_facts`.
   - Assert 1 call fact exists, `thing.callees` empty, call subject
     != thing entity.
4. Write test `cp3_unresolvable_container_falls_back_to_file_path`:
   - Mock `PhantomContainerObserver` returning
     `container: Some("phantom_container")`.
   - Pre-declare `src/lib.rs:real_fn:1` via `add_observation`.
   - Build snapshot, run `SnapshotEntityView::from_facts`.
   - Assert 1 call fact exists, `real_fn.callees` empty, call
     subject != real_fn entity, no entity carries the string
     "phantom_container".
5. Run `cargo test -p cognicode-core --lib --features evidence-kernel 'lsp_facts::tests'`
   — 9 tests MUST pass (7 existing + 2 new).

### Verification

```
cargo test -p cognicode-core --lib --features evidence-kernel 'lsp_facts::tests'
# Expected: 9 passed; 0 failed (7 pre-existing + 2 new CP-3 edge cases)
```

### Risk

- None. Pure test code. Any failure signals a regression in the
  `reference_subject` contract — which is the goal of the harness.
- The added `RelationKind` and `SnapshotEntityView` imports are
  test-scope; no production-code API surface change.

### Rollback

`git revert <commit>` — no migration, no schema, no runtime state.
