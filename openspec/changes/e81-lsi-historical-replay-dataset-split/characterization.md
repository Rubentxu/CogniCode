# e81 Characterization — Historical Replay Dataset Split

> Cycle: e81-lsi-historical-replay-dataset-split | Phase: WU0 (characterize) | Date: 2026-09-17
> Characterize before design. No production code changes in WU0.

## Existing e76 foundation

| Symbol | Location | Shape |
|--------|----------|-------|
| `HistoricalReplay` | `application/self_hosting/platform_equivalence.rs:190` | `snapshot_digest: String`, `observations: Vec<PlatformObservation>`, `sealed_prediction: Option<SealedPrediction>` |
| `SealedPrediction` | `application/self_hosting/prediction.rs:67` | `label`, `expected: Vec<ExpectedObservation>`, `seal_digest`; `seal()` sorts by id and hashes |
| `ExpectedObservation` | `prediction.rs:48` | `id: String`, `expected_present: bool` |
| `Observation` | `prediction.rs:117` | `id: String`, `present: bool` |
| `ScoreMatrix` | `prediction.rs:142` | `true_positive`, `false_positive`, `false_negative`, `true_negative`, `unknown`; `precision()`, `recall()`, `unknown_rate()` |
| `score()` | `prediction.rs:206` | compares a `SealedPrediction` to `&[Observation]` |
| `PlatformObservation` | `platform_equivalence.rs:62` | `platform`, `id`, `raw: Vec<u8>` |
| `MutationCorpus` | `mutation_corpus.rs:135` | `mutations: Vec<ControlledMutation>`, each with `expected_oracle_ids` / `forbidden_oracle_ids`; `to_sealed_prediction()` |
| self-hosting acceptance suites | `*_acceptance.rs`, `*_edge_cases.rs` | deterministic in-memory + real-source acceptance tests |

### The e76 contract that must not weaken

From `prediction.rs` and the umbrella `self-hosting-validation` spec:

```text
prediction is sealed BEFORE the observation is revealed
observations come from independent oracles (CogniCode is not its own sole oracle)
historical replay starts from the base state
a successor outcome must not leak into the prediction
```

## WU0 decision: REUSE / ADAPT / WRAP

```text
HistoricalReplay   REUSE  (unchanged) — the historical observation payload,
                          carried INSIDE a HistoricalCase. It stays an
                          observation container; it does NOT gain dataset
                          policy, corpus identity, or split membership.
SealedPrediction   REUSE  (unchanged) — the prediction seal.
Observation        REUSE  (unchanged) — the scoring input.
ScoreMatrix+score  REUSE  (unchanged) — the only score model; no competing one.
MutationCorpus     ADAPT  (shape only) — a precedent for "immutable corpus with
                          pre-declared expectations", but its element type is
                          oracle-id based, not a historical case. The corpus
                          shape is re-created; the type is not reused.
```

`HistoricalReplay` is NOT redefined into `HistoricalCase`. e76's container is
useful as-is; e81 adds a different semantic layer: case identity, explicit
base/successor separation, and split membership.

```text
HistoricalCase
 ├── id              (e81)
 ├── base_ref        (e81 — what the predictor may start from)
 ├── outcome_ref     (e81 — the successor; never shown to the predictor)
 └── replay          (e76 HistoricalReplay — observations + optional seal)
```

## Two gaps found in the existing surface

1. **`score()` cannot express "no observation recorded at all".**
   `score()` sets `matrix.unknown = 0` unconditionally and treats a missing
   observation as `FN`/`TN`. That is correct for a *present* observation set with
   gaps, but it means a case with **no recorded observations** would be scored as
   a normal comparison, silently implying "safe". WU14/WU10-O require an explicit
   incomplete representation. Decision: introduce a **replay-level** outcome
   (`ReplayCaseOutcome::Scored | Incomplete`), and do NOT change the global
   `score()` semantics.

2. **No corpus/split layer exists.** Nothing enforces disjoint immutable
   OPTIMIZE/CONFIRM sets. The umbrella spec
   (`specs/historical-replay-promotion/spec.md`) already requires it, and
   `tasks.md` 13.1/13.2 are the two items e81 implements.

## Design derived from WU0

```text
application/historical_replay/
  case.rs    HistoricalCaseId, ContentRef, HistoricalCase,
             HistoricalPredictionInput (base-side only)
  corpus.rs  HistoricalCorpus (unique ids, canonical order, digest)
  split.rs   DatasetRole, DatasetSplit (disjoint, immutable, role-encoded digest)
  plan.rs    HistoricalReplayPlan::prepare (bind + freeze) and run()
  replay.rs  HistoricalPredictor port, ReplayCaseResult, ReplayCaseOutcome, errors
  report.rs  ReplayReport (role-tagged), ReplayReports { optimize, confirm }
```

Reuse: `crate::application::portable_execution::content_digest` for every
digest (the same helper `SealedPrediction::seal` uses), so the corpus/split
digests compose with the existing sealing scheme.

### Structural no-leak (WU2)

The predictor receives `HistoricalPredictionInput { case_id, base_snapshot,
base_source_ref }`. It has no field that could carry a successor digest, an
observation, a ground-truth outcome, a score, or a confirm result. The secret
side (`outcome_ref`, `replay.observations`, the sealed prediction) stays on the
`HistoricalCase`, which is never handed to the predictor. This turns e76's
"predict first, observe after" from a convention into an API boundary.

### Disjointness is a construction invariant (WU4/WU5)

`DatasetSplit::try_new` rejects duplicates and overlap, so a valid split cannot
be produced at all. `HistoricalReplayPlan::prepare` then re-validates corpus
membership and split invariants, and only a successful plan can execute.
Therefore an overlapping or unknown-id configuration yields **zero predictor
invocations** because it cannot reach a runnable plan.

## WU0 exit

* e76 surface inventoried; REUSE/ADAPT/WRAP decided explicitly.
* `HistoricalReplay` is not repurposed into `HistoricalCase`.
* Two gaps recorded: replay-level incompleteness, and the missing corpus/split.
* No production code changed yet.
