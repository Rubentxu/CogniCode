# e82.2 — Historical Score Correctness

> Cycle: e82.2-lsi-historical-score-correctness | Phase: characterize + apply | Date: 2026-09-17
> Closure pattern: administrative archive (git-level).

## Why this cycle exists

e82 characterized a real semantic defect in `self_hosting::prediction::score`.
The documented contract says:

```text
expected=true  observed=true   => TP
expected=true  observed=false  => FN
expected=false observed=true   => FP
expected=false observed=false  => TN
```

The implementation matched on the **observation alone**, so
`expected=false + observed=true` was counted as **TP**. While scores were only
observation (e81/e82) that was a tolerated, recorded discrepancy. In e83 those
numbers enter a promotion gate, so known-incorrect semantics are no longer
acceptable.

## WU0 — blast radius

Production consumers of the scorer:

| Consumer | Location | Effect of the fix |
|----------|----------|-------------------|
| `HistoricalReplay::score()` | `self_hosting/platform_equivalence.rs:258` | none on existing fixtures |
| e81 replay execution | `historical_replay/plan.rs` (`score`) | none on existing fixtures |
| e82 shadow comparison | `shadow_evaluation/compare.rs` (via `ScoreMatrix`) | none on existing fixtures |

Fixture classification (every `expected_present: false` site):

| Fixture | Classification |
|---------|----------------|
| `prediction.rs::score_perfect_match_yields_only_tp_and_tn` | CORRECT — the absent expectation is observed absent |
| `prediction.rs::score_false_negative_*`, `*_false_positive_*`, `*_missing_observation_*` | CORRECT |
| `prediction_acceptance.rs::acceptance_score_perfect_match_returns_zero_fp_zero_fn` | CORRECT |
| `prediction_acceptance.rs::acceptance_score_surfaces_false_negatives` | CORRECT |
| `platform_equivalence.rs::historical_replay_can_score_against_sealed_prediction` | CORRECT |
| `closure_replay_shape.rs:105` (`expected_present: false`) | AMBIGUOUS but unused for scoring — the value is only asserted structurally |
| `MutationCorpus::to_sealed_prediction` (`forbidden_oracle_ids`) | **the real-world exposure**: a forbidden oracle that fired was scored TP instead of FP |

**No existing fixture depends on the buggy behaviour.** The defect was masked
because no shipped fixture scored an `expected=false + observed=true` cell.

## WU1/WU2 — the fix

Minimal, single-branch correction: the expectation loop now matches on the pair
`(expected_present, observed)` instead of the observation alone. Missing
observations keep the existing documented "treated as absent" semantics.

Nothing else changed: `ScoreMatrix`, `ReplayCaseOutcome`, `FailureRegime`, the
shadow evaluator and the replay engine are untouched.

New truth-table tests (all green):

```text
e82_2_score_truth_table_is_exactly_the_documented_contract
e82_2_missing_observation_is_treated_as_absent
e82_2_unexpected_present_observation_is_a_false_positive
e82_2_truth_table_is_order_invariant
e82_2_duplicate_observation_ids_have_last_wins_semantics
```

## WU3 — `unknown` stays separate

`ScoreMatrix::unknown` is still 0 from `score()`. No unknown counts were
invented. A case with no recorded observation remains e81's
`ReplayCaseOutcome::Incomplete`. If partial completeness needs a richer model
later, that is a separate, documented change.

## WU4 — measurement stack rerun

```text
e76 self_hosting        71 passed / 0 failed   (66 existing + 5 new truth-table tests)
e81 historical_replay   23 passed / 0 failed
e82 shadow_evaluation   20 passed / 0 failed
```

**Zero fixtures changed.** The corrected scorer produced identical results for
every existing fixture, which independently confirms the WU0 classification:
the old expectations were correct under the documented contract, and only the
unexercised `(false, true)` cell was wrong. No historical corpus meaning was
altered to preserve old numbers.

## WU5 — adversarial cases

| Case | Status |
|------|--------|
| A expected absent + observed present → FP, never TP | covered by the truth table |
| B expected present + observed present → TP | covered |
| C expected present + observed absent → FN | covered |
| D expected absent + observed absent → TN | covered |
| E order of expectations/observations → same matrix | `e82_2_truth_table_is_order_invariant` |
| F duplicate observation ids | **characterized**, last-wins, documented as a limitation; not changed (would widen scope) |
| G empty recorded observation at replay level → Incomplete | unchanged, e81 `ReplayCaseOutcome::Incomplete` |
| H shadow comparison reflects corrected scores deterministically | e82 suite (20 tests) unchanged and green |
| I no score outcome can mint `PolicyDecision` | e82 module audit (`r_s_t_shadow_module_has_no_authority_surface`) |
| J no score outcome can mint `PromotionPermit` | same audit |

## WU6 — no policy

No minimum recall, no maximum FP/FN, no promotion tolerances, no candidate
acceptance. The corrected score is still measurement; e83 decides how held-out
evidence is interpreted.

## Verification

| Check | Result |
|-------|--------|
| score truth-table tests | GREEN (5 new) |
| e76 `self_hosting` | 71 passed / 0 failed |
| e81 `historical_replay` | 23 passed / 0 failed |
| e82 `shadow_evaluation` | 20 passed / 0 failed |
| e82.1 architecture self-host | 0 drift |
| e80a `promotion_authority` | 42 passed / 0 failed |
| e80b `application::ai` | 48 passed / 0 failed |
| `cargo test -p cognicode-core --lib` | failure set == `scripts/known_failures.yaml` exactly (41 entries) |
| same, `--features evidence-kernel` | 2576 passed / 2 failed (the classified DEBT-SDDK-006 entries) |
| `cargo build --workspace` | GREEN |
| clippy on the touched surface | no new warnings |

No new failure occurred; nothing needed re-characterization.
