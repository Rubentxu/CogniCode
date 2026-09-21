# Verification Report — cycle e62.3 — grounding refs, scope sentinel, evidence allocator (U42, part 2a)

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e62.3 — grounding refs, scope sentinel, evidence allocator |
| Path | A-lite |
| Base HEAD | `005bb878` (post-e62.2) |
| Verify verdict | **PASS** |

## Verification

### V-1 — Tests

```
cargo test -p cognicode-core --lib domain::findings                      # 106 passed
cargo test -p cognicode-core --lib application::findings                 # 21 passed
cargo test -p cognicode-core --features evidence-kernel --lib evidence_kernel  # 88 passed
cargo test -p cognicode-core --test findings_ast_e2e                     # 6 passed
cargo test -p cognicode-core --test findings_graph_e2e                   # 4 passed
cargo test -p cognicode-core --test findings_dataflow_e2e                # 7 passed
cargo test -p cognicode-core --test findings_axiom_import_e2e            # 3 passed
```

`domain::findings` went 101 → 106 (+3 grounding, +1 scope sentinel, +1 executor
sentinel refusal).

### V-2 — The sentinel hole is closed at the trust boundary

`a_scope_pinned_to_the_none_sentinel_is_refused` builds `Some(scope)` with
`SnapshotId::NONE` — i.e. exactly the case that used to slip past
`MissingScope` — and asserts:

- `DetectorExecutor::execute` returns `ExecutionError::InvalidScope`;
- the evidence sink recorded **nothing**: the refusal happens before planning,
  not after a backend produced evidence it would then have to discard.

`none_snapshot_is_rejected` covers the same rule at the constructor
(`try_new` → `AnalysisScopeError::InvalidSnapshot`, `is_valid() == false`).

### V-3 — The allocator property that motivated WU0b

`append_batch_allocates_unique_ids_per_snapshot`:

| Call | Snapshot | Ids returned |
|------|----------|--------------|
| batch of 2 | A | `1, 2` |
| batch of 1 | A | `3` (no collision with the first batch) |
| batch of 1 | B | `1` (each snapshot has its own sequence) |

and `for_fact` sees 2 evidence atoms for `FactId(1)` in A and 1 in B, so the
`by_fact` index is scope-consistent too.

`explicit_add_advances_the_allocator`: an explicit `add(EvidenceId(7))` makes the
next batch return `8`, so legacy explicit-id writes cannot collide with
store-allocated ones.

### V-4 — `GroundingRef` is additive and authority-correct

- `grounding_round_trips`: serde round-trip, and an omitted entity hint
  deserialises to `None` (the field is genuinely optional on the wire).
- `entity_hint_is_checked_against_the_canonical_subject`:
  `entity_agrees_with` returns `Some(true)`, `Some(false)` and `None`, i.e. a
  hint is a *checkable claim* against the canonical subject, never an authority.
- All four DTOs (`AstConstruct`, `GraphNode`, `GraphEdge`, `DataflowStatement`)
  compile with the new field; every pre-existing literal now passes
  `grounding: None`, and no backend reads the field yet — the change is a pure
  capability addition, and the four E2E suites (AST/Graph/Dataflow/Axiom) still
  pass unchanged.

### V-5 — Build / format / regressions

- `cargo check --workspace --all-targets` → 0 errors.
- `cargo fmt --all --check` → clean.
- `scripts/check_known_failures.py` → exit 0, baseline 41 entries unchanged.

## Deliberately not executed

- Full workspace test suite beyond the known-failure baseline: the change is
  additive in `domain::findings` + `evidence_kernel` and the baseline covers the
  rest of the workspace by name.
- The three adversarial UATs (scope / fact mismatch / refuting evidence): they
  belong to U42 part 2b, which does not exist yet. Claiming them now would be a
  false positive.

## Unknown impact

- None material. The only cross-cutting edit is the `grounding` field on four
  DTOs, which is `skip_serializing_if` so no serialized shape changed for
  existing data (verified by the unchanged E2E suites and the axiom-import
  suite).

## Conclusion

The reviewer's two pre-conditions for building the write bridge are met, and the
grounding vocabulary is in place without altering any analysis. **PASS**; U42
remains open on part 2b.
