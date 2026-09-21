# Archive Manifest — e81 Historical Replay Dataset Split

> Cycle: e81-lsi-historical-replay-dataset-split | Phase: archive | Date: 2026-09-17
> Closure pattern: **administrative archive** (git-level), consistent with
> e79/e79.1/e80a/e80b.

## Cycle summary

| Phase | Status | Artifact |
|-------|--------|----------|
| WU0 characterize | DONE | `characterization.md` (inventory + REUSE/ADAPT/WRAP + two gaps) |
| WU1 case identity | DONE | `case.rs` (`HistoricalCaseId`, `ContentRef`, `HistoricalCase`) |
| WU2 no-leak input | DONE | `HistoricalPredictionInput` (base-side only) |
| WU3 corpus | DONE | `corpus.rs` (unique ids, canonical order, digest) |
| WU4 split | DONE | `split.rs` (`DatasetRole`, `DatasetSplit`, role-encoded digest) |
| WU5 bind + validate | DONE | `HistoricalReplayPlan::prepare` |
| WU6 frozen run | DONE | the plan owns corpus + id vectors |
| WU7 replay execution | DONE | `replay.rs` (`HistoricalPredictor`, `ReplayCaseResult`) |
| WU8 reports | DONE | `report.rs` (`ReplayReport`, `ReplayReports`) |
| WU9 held-out discipline | DONE | OPTIMIZE and CONFIRM never merged; no combined score |
| WU10 adversarial tests | DONE | cases A-O |
| WU11 e76 integration UAT | DONE | vertical replay test |
| WU12 no random split | DONE | module audit |
| WU13 no CONFIRM leakage | DONE | OPTIMIZE report usable independently; documented e83 split |
| WU14 unknown semantics | DONE | `ReplayCaseOutcome::Incomplete` (no faked unknown counts) |
| WU15 authority audit | DONE | module source audit |
| archive | DONE | this document + `state.yaml` update |

## Exit statement

```text
HistoricalCase model             CLOSED
HistoricalCorpus                 CLOSED
DatasetSplit                     CLOSED

OPTIMIZE/CONFIRM disjoint        PROVEN
OPTIMIZE/CONFIRM immutable       PROVEN
overlap pre-execution rejection  PROVEN
base/outcome leakage             PREVENTED BY API
replay deterministic             PROVEN

candidate comparison             NOT STARTED / e82
FailureRegime                    NOT STARTED / e82
held-out promotion               NOT STARTED / e83
self-improvement                 NOT STARTED / e83
```

### How each property is enforced

| Property | Mechanism |
|----------|-----------|
| disjoint | `DatasetSplit::try_new` rejects overlap; `prepare` re-checks |
| immutable | private fields, canonical sorted ids, no mutation API; the plan owns a frozen copy |
| overlap pre-execution rejection | no runnable plan exists without a valid split, so zero predictor invocations |
| leakage prevented by API | `HistoricalPredictionInput` has only the three base-side fields |
| deterministic | canonical ordering + digest; identical plan + predictor yields identical reports |

## e81 vs e82 vs e83

```text
e81  structural dataset separation + deterministic replay measurement
e82  AnalyzerCurrent vs AnalyzerCandidate + FailureRegime + shadow evaluation
e83  promotion-time held-out discipline + governed improvement E2E
```

e81 does not decide what is "good enough". It introduces no thresholds
(no `recall >= x`, no `FN <= y`).

## Reuse of e76 (unchanged)

`SealedPrediction`, `Observation`, `ScoreMatrix` and `score()` are the only
score model. `HistoricalReplay` remains an observation container; it did **not**
become `HistoricalCase`, and it gained no dataset policy. The e76 contract is
preserved: the prediction is sealed before observation, and observations cannot
retro-fit a prediction.

## Verification

| Check | Result |
|-------|--------|
| e81 `historical_replay` (WU10 A-O + WU11 + audits) | 23 passed / 0 failed |
| e76 `self_hosting` | 66 passed / 0 failed |
| e80b `application::ai` | 48 passed / 0 failed |
| e80a `promotion_authority` | 42 passed / 0 failed |
| e69 `policy_gate` | 14 passed / 0 failed |
| e77.1 `architecture_e77_1_wu3` canonical grounding | 10 passed / 0 failed |
| `cargo test -p cognicode-core --lib` | terminates; failure set == `scripts/known_failures.yaml` exactly (41 entries) |
| same, `--features evidence-kernel` | 2551 passed / 2 failed (the classified DEBT-SDDK-006 entries) |
| `cargo build --workspace` | GREEN |
| clippy on the touched surface | no new warnings |

No new failure was encountered; nothing needed re-characterization.

## Explicit non-goals (honored)

No `AnalyzerCurrent` vs `AnalyzerCandidate`, no FailureRegime taxonomy, no shadow
evaluation, no candidate-superiority decision, no held-out promotion gate, no
promotion thresholds or tolerances, no automatic improvement, no FixAgent
invocation, no automatic patch generation, no automatic trial, no Backstage or
Control Plane, no e78 Packs, no P1.4 repair, no DEBT-SDDK-006 repair.

## Debts

| Debt | Status |
|------|--------|
| DEBT-SDDK-002 | RESOLVED (e79.1) |
| DEBT-SDDK-005 | RESOLVED (e79.1) |
| DEBT-SDDK-004 | PARTIAL: 3/4 repaired; P1.4 deferred with architectural trigger |
| DEBT-SDDK-006 | OPEN: `interproc_summary` feature-gating (two baseline entries) |

`known_failures.yaml` was NOT modified.

## Commits

* **Implementation:** `c7086edc` — `feat(e81): historical replay with an
  immutable OPTIMIZE/CONFIRM split`.
* **Archive:** this commit — this manifest + `state.yaml` umbrella update.

## Files

New:
* `application/historical_replay/{mod,case,corpus,split,plan,replay,report}.rs`
* `application/historical_replay/historical_replay_tests.rs`

Changed:
* `application/mod.rs` (wire the gated module)

## State transition

```text
e81 CLOSED
M13 13.1 (historical replay dataset format) SATISFIED
M13 13.2 (OPTIMIZE/CONFIRM disjoint sets) SATISFIED

NEXT: e82 (AnalyzerCurrent vs AnalyzerCandidate + FailureRegime + shadow evaluation)
```

## STOP

Architectural stop after e81, for review before e82. Historical replay now
measures; it does not compare, classify, or promote.
