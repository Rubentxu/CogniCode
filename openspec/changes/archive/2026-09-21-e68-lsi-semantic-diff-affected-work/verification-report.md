# e68 — Verification Report

## Status

```text
e68 implementation   CLOSED
e68 verification     PASS
e68 SDDK lifecycle   not formally instantiated
```

e68 was implemented as a bounded A-lite slice without a formal SDDK
cycle (proposal → spec → tasks → verify → archive). This document
records the honest lifecycle state without fabricating artifacts that
were never produced.

## Scope delivered

Three work units, all GREEN and committed:

| WU | Module |
|----|--------|
| WU1 | `crates/cognicode-core/src/domain/evidence_kernel/semantic_diff.rs` |
| WU2 | `crates/cognicode-core/src/application/change_tracking/planner.rs` |
| WU3 | (adversarial + invariants tests inside the WU2 module) |

The single e68 commit (see git log) covers all three work units
together — the modules are not independently compilable (WU2 depends
on WU1 types), and splitting them would have produced artificial
checkpoint boundaries.

## Verification evidence

### WU1 — Semantic Fact Delta

9/9 tests in `domain::evidence_kernel::semantic_diff::tests`:

- empty_delta_when_only_fact_ids_differ
- one_relation_added
- one_relation_removed
- object_change_decomposes_into_removed_plus_added
- input_ordering_is_irrelevant
- same_snapshot_is_rejected
- duplicates_within_one_side_collapse
- empty_inputs_with_distinct_snapshots_produce_empty_delta
- distinct_predicate_namespaces_are_not_coalesced

### WU2 — Affected Work Planner (UAT + WU3 adversarial)

16/16 tests in `application::change_tracking::planner::tests`:

UAT (positive cases):

- removed_read_fact_yields_affected
- read_fact_semantic_change_yields_affected
- unrelated_change_yields_unaffected
- addition_without_removals_yields_unknown
- truncated_read_set_yields_unknown
- addition_plus_unrelated_change_does_not_collapse_to_unaffected
- planner_is_deterministic_under_input_permutation
- empty_delta_yields_all_unaffected

WU3 adversarial matrix and invariants:

- additions_with_aggregate_query_must_collapse_to_unknown
- multiple_executions_merge_conservatively
- disposition_merge_order_is_unknown_beats_unaffected_affected_beats_unknown
- planner_rejects_invalid_snapshot_pair_via_diff_error
- renumbered_facts_yield_no_planning_disruption
- planner_is_deterministic_under_dependency_permutation
- empty_dependencies_yield_empty_plan
- unaffected_must_never_coexist_with_additions

### Total: 25/25 PASS

## Regression (all green)

| Suite | Result |
|-------|--------|
| e68 WU1+WU2+WU3 (lib tests) | 25/25 |
| e67 WU1+WU2+WU3 | 20/20 |
| e66 readset | 2/2 |
| `just lsi-equivalence` | 7/7 (same scores; multi-lang-types still QUARANTINED) |
| `findings_ast_e2e` | 8/8 |
| `findings_graph_e2e` | 4/4 |
| `findings_dataflow_e2e` | 7/7 |
| `findings_canonical_grounding_e2e` | 10/10 |
| `intelligence_event_log_e2e` | 4/4 |

## Static gates (clean for e68 files)

- `rg 'Handle::current\(\)\.block_on' <file>` → no match.
- `cargo fmt --check -p cognicode-core` → clean.
- `cargo clippy -p cognicode-core --all-targets --features evidence-kernel -- -D warnings` →
  no warnings on e68 files; pre-existing warnings on other modules
  (`admission.rs`, etc.) are unchanged.

## Architecture invariants preserved

- **`FactId` and `SnapshotId` do not participate in semantic equality**:
  renumbering facts across snapshots produces an empty delta when the
  underlying `(subject, predicate, object)` triples are unchanged
  (`renumbered_facts_yield_no_planning_disruption`).
- **`Unknown` MUST NOT collapse to `Unaffected`**:
  - For aggregate read-sets, additions always trigger Unknown
    (`additions_with_aggregate_query_must_collapse_to_unknown`).
  - Truncated read-sets always yield Unknown regardless of the delta
    (`truncated_read_set_yields_unknown`).
  - The invariant is asserted over every read-set shape combination
    (`unaffected_must_never_coexist_with_additions`).
- **Logical work vs execution instance** is explicit: planner output
  speaks `WorkId` (an opaque `NamespacedName`); `ExecutionId` lives on
  the input side only.
- **`SemanticFactDelta` and `AffectedWorkPlan` are derived values**, not
  canonical Facts. Nothing is written to the Evidence Kernel.
- **Determinism**: the same delta + same dependencies produce the
  same plan regardless of input order or dependency list order
  (`planner_is_deterministic_under_input_permutation`,
  `planner_is_deterministic_under_dependency_permutation`).
- **Fail-closed**: invalid snapshot pairs are rejected by the diff
  (`planner_rejects_invalid_snapshot_pair_via_diff_error`); the
  planner never silently swallows an invalid input.
- **No job taxonomy built yet**: `WorkId` is opaque; no `enum WorkKind`.

## Deviations vs. the proposal

None.

## Lifecycle honesty

e68 did not instantiate the formal SDDK lifecycle. The proposal,
verification, and test evidence above were captured locally under
`openspec/changes/e68-lsi-semantic-diff-affected-work/`. Future cycles
should not reference e68 as having gone through `archive.complete` or
any release-receipt chain — those artifacts were never produced.
