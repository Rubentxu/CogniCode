# e82 Characterization — Shadow Evaluation and Failure Regimes

> Cycle: e82-lsi-shadow-evaluation-failure-regimes | Phase: WU0 (characterize) | Date: 2026-09-17
> Characterize before design. No production code changes in WU0.

## Inventory and reuse map

| e81/e76 surface | Location | Decision |
|-----------------|----------|----------|
| `HistoricalReplayPlan` | `application/historical_replay/plan.rs` | **REUSE** (+ additive `run_role`) |
| `HistoricalPredictor` | `historical_replay/replay.rs` | **REUSE** for BOTH analyzer sides |
| `ReplayCaseResult` / `ReplayCaseOutcome` | `historical_replay/replay.rs` | **REUSE** |
| `ReplayReport` / `ReplayReports` | `historical_replay/report.rs` | **REUSE** |
| `DatasetRole` / `DatasetSplit` | `historical_replay/split.rs` | **REUSE** |
| `HistoricalCase` / `HistoricalPredictionInput` | `historical_replay/case.rs` | **REUSE** |
| `ScoreMatrix` + `score()` | `self_hosting/prediction.rs` | **REUSE** (unchanged; only score model) |
| `HistoricalReplay` / `PlatformObservation` / `PlatformKind` | `self_hosting/platform_equivalence.rs` | **REUSE** |
| `compare_platforms` / `Equivalence` / `PlatformNormaliser` / `DefaultNormaliser` | `self_hosting/platform_equivalence.rs` | **REUSE** for `PlatformDivergence` |
| `ArchitectureViolation` | `domain/architecture/violation.rs` | **SEAM** (no per-case adapter exists) |
| `PerWorkReport` / `AffectedWorkPlan` | `application/local_ci`, `change_tracking` | **SEAM** (not attached to a case) |
| provider health / fallback typing | n/a in the replay path | **SEAM** |
| `AnalyzerDescriptor` / `AnalyzerSide` / `AnalyzerUnderTest` | — | **NEW** |
| `ShadowEvaluationPlan` | — | **NEW** |
| `ShadowCaseComparison` / `ScoreDelta` / `ShadowAggregateDelta` | — | **NEW** |
| `FailureRegime` taxonomy + classifier port | — | **NEW** |
| `ShadowRoleReport` / `ShadowEvaluation` / `ShadowEvaluator` | — | **NEW** |

**No forking of the replay engine.** e81 owns replay; e82 composes it through the
newly added `HistoricalReplayPlan::run_role`. `run()` is now compatibility sugar
over `run_role`, with identical semantics.

## Characterization finding: e76 `score()` does not match its own doc

`self_hosting::prediction::score` documents:

```text
TP: expected present + observed present.
FP: expected absent + observed present (unexpected signal).
```

The implementation inverts part of that. For every expectation it does:

```rust
match obs_index.get(exp.id) {
    Some(true)  => matrix.true_positive += 1,   // ignores exp.expected_present
    Some(false) => if exp.expected_present { FN } else { TN },
    None        => if exp.expected_present { FN } else { TN },
}
```

So an expected-absent observation that fires is counted as **TP**, not FP. FP is
only produced by observations that are **absent from the prediction's expected
set entirely**.

Consequences:

* e82 fixtures must express "current misses it" as *absence from the expected
  set*, not as `expected_present: false`.
* The `unknown` counter is still always `0` from `score()` (already recorded by
  e81's WU14 note).

This is **not repaired here**. e81 and e82 both deliberately leave the global
scorer untouched; changing it is a separate decision with its own blast radius.
It is recorded so the semantics are not silently misread.

## Design derived from WU0

```text
application/shadow_evaluation/
  analyzer.rs     AnalyzerDescriptor, AnalyzerSide, AnalyzerUnderTest
  plan.rs         ShadowEvaluationPlan + evaluation_digest
  compare.rs      ScoreDelta, ShadowCaseComparison, pairing, ShadowError
  regime.rs       FailureRegime taxonomy + classifier port
  classifiers.rs  BuiltinFailureRegimeClassifier (only sound classifications)
  report.rs       ShadowRoleReport, ShadowEvaluation, ShadowEvaluator
```

Key decisions:

1. **One predictor contract.** `Current` and `Candidate` are evaluation roles,
   not analyzer types. Both sides are `&dyn HistoricalPredictor`.
2. **`evaluation_digest`** binds corpus + split + both revision digests. No
   authority in e82; it exists so e83 can prove the CONFIRM-passing candidate is
   the one being promoted.
3. **Descriptive deltas only.** Signed `candidate - current` counts; no
   `Improved`/`Regressed`/`Winner`.
4. **Regimes need typed evidence.** `EvidenceIncomplete` and `PlatformDivergence`
   are implemented from real typed sources. `GroundingFailure`, `ImpactMiss`,
   `ArchitectureMiss`, `ProviderDegradation` are contract + adapter seam, because
   no typed per-case signal reaches the replay. `false_negative > 0` is NOT an
   impact miss, and observation strings are never parsed.
5. **Incomplete stays explicit.** A case incomplete on either side has no
   `ScoreDelta`; it never becomes zeros.

## WU0 exit

* Reuse map complete; no engine duplication.
* `run_role` identified as the needed additive seam.
* The e76 `score()` doc/implementation discrepancy characterized and recorded.
* No production code changed in WU0.
