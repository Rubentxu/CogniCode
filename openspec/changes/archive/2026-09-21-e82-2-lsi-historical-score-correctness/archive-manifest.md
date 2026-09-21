# Archive Manifest — e82.2 Historical Score Correctness

> Cycle: e82.2-lsi-historical-score-correctness | Phase: archive | Date: 2026-09-17
> Closure pattern: administrative archive (git-level).

## Result

```text
scorer truth table        CORRECT + PROVEN
existing fixtures changed ZERO
unknown semantics         UNCHANGED (still 0; Incomplete lives in e81)
policy / thresholds       NONE ADDED
```

The documented contract `expected=false + observed=true => FP` now holds. It had
been counted as TP because the implementation matched on the observation alone.

## Why it was safe to fix now

* WU0 classified every fixture using `expected_present: false`; none depended on
  the defective cell, so the defect was masked rather than load-bearing.
* The rerun of the whole measurement stack produced **zero** changed fixtures,
  which independently confirms that classification.
* The real exposure was `MutationCorpus::to_sealed_prediction`: a forbidden
  oracle that fired was scored TP instead of FP, hiding a "MUST NOT fire"
  violation.

## Verification

| Check | Result |
|-------|--------|
| new truth-table tests (2x2, missing, unexpected, order, duplicates) | 5 GREEN |
| e76 `self_hosting` | 71 passed / 0 failed |
| e81 `historical_replay` | 23 passed / 0 failed |
| e82 `shadow_evaluation` | 20 passed / 0 failed |
| e82.1 architecture self-host | 0 drift |
| e80a `promotion_authority` | 42 passed / 0 failed |
| e80b `application::ai` | 48 passed / 0 failed |
| `cargo test -p cognicode-core --lib` | failure set == `scripts/known_failures.yaml` exactly (41 entries) |
| same, `--features evidence-kernel` | 2576 passed / 2 failed (DEBT-SDDK-006) |
| `cargo build --workspace` | GREEN |
| clippy on the touched surface | no new warnings |

No new failure occurred.

## Commits

* **Implementation:** `c58f260c` — `fix(e82.2): correct the historical scorer
  truth table`.
* **Archive:** this commit — this manifest + `state.yaml` update.

## State transition

```text
e82.2 CLOSED
historical scoring CORRECT

STOP before e83 (held-out promotion + governed improvement E2E).
```
