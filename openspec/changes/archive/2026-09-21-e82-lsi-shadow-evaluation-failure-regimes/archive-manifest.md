# Archive Manifest — e82 Shadow Evaluation and Failure Regimes

> Cycle: e82-lsi-shadow-evaluation-failure-regimes | Phase: archive | Date: 2026-09-17
> Closure pattern: **administrative archive** (git-level), consistent with
> e79/e79.1/e80a/e80b/e81.

## Cycle summary

| Phase | Status | Artifact |
|-------|--------|----------|
| WU0 characterize | DONE | `characterization.md` (reuse map + the e76 `score()` finding) |
| WU1 role-specific seam | DONE | `HistoricalReplayPlan::run_role` (additive) |
| WU2 analyzer identity | DONE | `analyzer.rs` (`AnalyzerDescriptor`, `AnalyzerSide`) |
| WU3 shadow plan | DONE | `plan.rs` (`ShadowEvaluationPlan`, `evaluation_digest`) |
| WU4 predictor reuse | DONE | `AnalyzerUnderTest` over `&dyn HistoricalPredictor` |
| WU5 role execution | DONE | `ShadowEvaluator::evaluate_role` / `evaluate_all` |
| WU6 per-case comparison | DONE | `compare.rs` (`pair_case_results`) |
| WU7 descriptive deltas | DONE | `ScoreDelta`, `ShadowAggregateDelta` |
| WU8 taxonomy | DONE | `regime.rs` (`FailureRegime`) |
| WU9 typed evidence | DONE | `FailureRegimeEvidence`, `FailureRegimeOccurrence` |
| WU10 sound classifications | DONE | `classifiers.rs` (EvidenceIncomplete, PlatformDivergence) |
| WU11 classifier seam | DONE | `FailureRegimeClassifier` port |
| WU12 incomplete stays explicit | DONE | no `ScoreDelta` when either side is incomplete |
| WU13 role report | DONE | `ShadowRoleReport` (no selection fields) |
| WU14 no hidden metric policy | DONE | audited; no thresholds |
| WU15 execution isolation | DONE | each side runs independently |
| WU16 determinism | DONE | canonical ordering, no clock |
| WU17 control experiment | DONE | identical revisions allowed |
| WU18 adversarial tests | DONE | cases A-T |
| WU19 OPTIMIZE/CONFIRM future-proofing | DONE | `evaluate_role` per role |
| WU20 P1.4 boundary | DONE | untouched; no evaluator wired into authority |
| WU21 authority audit | DONE | module source audit |
| WU22 vertical UAT | DONE | end-to-end shadow evaluation test |
| archive | DONE | this document + `state.yaml` update |

## Exit statement

```text
AnalyzerCurrent identity          CLOSED
AnalyzerCandidate identity        CLOSED
role-specific replay              CLOSED
shadow comparison                 CLOSED
FailureRegime taxonomy            CLOSED

current/candidate same input       PROVEN
per-case pairing                   PROVEN
raw metric deltas                  PROVEN
incomplete remains explicit        PROVEN
failure regimes evidence-backed    PROVEN
shadow evaluation deterministic    PROVEN

candidate selection                NOT IMPLEMENTED
promotion policy                   NOT IMPLEMENTED
held-out gate                      NOT IMPLEMENTED / e83
self-improvement                   NOT IMPLEMENTED / e83

P1.4                              STILL EXPLICIT
DEBT-SDDK-006                     STILL EXPLICIT
```

## What is implemented for real, and what is a seam

```text
REAL   EvidenceIncomplete   from e81 ReplayCaseOutcome::Incomplete
REAL   PlatformDivergence   from e76 compare_platforms, >= 2 distinct platforms only

SEAM   GroundingFailure     no typed per-case grounding signal reaches the replay
SEAM   ImpactMiss           the replay scorer does not know the prediction domain
SEAM   ArchitectureMiss     ArchitectureViolation exists, but no per-case adapter
SEAM   ProviderDegradation  no typed provider-health signal reaches the replay
```

The four seams are a contract plus a `FailureRegimeClassifier` adapter (a
deterministic example lives in the test suite). They are NOT guessed:
`false_negative > 0` is not an impact miss, and observation strings are never
parsed.

## Characterization finding (recorded, not repaired)

`self_hosting::prediction::score` documents "FP: expected absent + observed
present", but its implementation counts a firing observation as **TP** whenever
it appears in the prediction's expected set, regardless of `expected_present`.
FP arises only from observations absent from the expected set. e82's fixtures
follow the implementation; the global scorer is left untouched, as in e81. Full
detail in `characterization.md`.

## e82 vs e83

```text
e82  measurement + diagnosis; structural dataset separation (e81)
e83  held-out promotion policy + governed improvement E2E
```

e82 does NOT guarantee that nobody inspected CONFIRM while developing a
candidate. `ShadowEvaluationPlan::evaluation_digest` binds corpus + split + both
revisions so e83 can prove the CONFIRM-passing candidate is the candidate being
promoted. e82 does not consume that digest as authority.

## Verification

| Check | Result |
|-------|--------|
| e82 `shadow_evaluation` (WU18 A-T + WU22) | 20 passed / 0 failed |
| e81 `historical_replay` (run_role regression) | 23 passed / 0 failed |
| e76 `self_hosting` | 66 passed / 0 failed |
| e80b `application::ai` | 48 passed / 0 failed |
| e80a `promotion_authority` | 42 passed / 0 failed |
| e77.1 canonical grounding | 10 passed / 0 failed |
| `cargo test -p cognicode-core --lib` | terminates; failure set == `scripts/known_failures.yaml` exactly (41 entries) |
| same, `--features evidence-kernel` | 2571 passed / 2 failed (the characterized DEBT-SDDK-006 entries) |
| `cargo build --workspace` | GREEN |
| clippy on the touched surface | no new warnings |

Three e82 tests failed during development and were characterized, not absorbed:
two were fixtures that had assumed the documented (not implemented) `score()`
FP semantics, and one asserted a delta where the absolute count was meant. All
three were corrected against the implementation; no production behaviour changed
to make them pass.

## Explicit non-goals (honored)

No `AnalyzerCurrent` vs `AnalyzerCandidate` trait split, no held-out promotion
gate, no candidate-superiority decision, no promotion thresholds or tolerances,
no latency/fallback metrics invented, no FixAgent invocation, no
PromotionAuthorization/PromotionPermit, no ChangeProposal creation, no source
mutation, no Backstage/Control Plane, no e78, no P1.4 repair, no DEBT-SDDK-006
repair.

## Debts

| Debt | Status |
|------|--------|
| DEBT-SDDK-002 | RESOLVED (e79.1) |
| DEBT-SDDK-005 | RESOLVED (e79.1) |
| DEBT-SDDK-004 | PARTIAL: 3/4 repaired; P1.4 deferred with architectural trigger |
| DEBT-SDDK-006 | OPEN: `interproc_summary` feature-gating (two baseline entries) |

`known_failures.yaml` was NOT modified.

## Commits

* **Implementation:** `654e5ab1` — `feat(e82): analyzer shadow evaluation and
  failure-regime diagnosis`.
* **Archive:** this commit — this manifest + `state.yaml` umbrella update.

## State transition

```text
e82 CLOSED
M13 13.3 (FailureRegime taxonomy contract) SATISFIED
M13 13.4 (analyzer shadow comparison) SATISFIED

NEXT: e82.1 (P1.4 CausalRecorder architecture boundary)
THEN: e83 (held-out promotion + governed improvement E2E)
```

## STOP

Architectural stop after e82, for review. Before e83, the P1.4 architecture
boundary is resolved so that executable-architecture evidence can enter the
governed promotion loop on a clean substrate. e82 itself did NOT repair P1.4 and
did NOT wire the e77 evaluator into any authority-bearing gate.
