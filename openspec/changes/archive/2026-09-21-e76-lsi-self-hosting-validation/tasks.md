# Tasks — e76 LSI Self-Hosting + Platform Equivalence Harness

> Planned decomposition. Refine during e76 design.

## WU1 — Self-model baseline

- run CogniCode on CogniCode using canonical production ingestion;
- freeze normalized semantic digests and deterministic baseline checks;
- compose existing graph/equivalence/sandbox evidence rather than duplicating harnesses.

## WU2 — Prediction vs observation

- seal affected-work/architecture/finding predictions from the base world;
- run controlled candidate through SoftwareWorld/ChangeProposal/Trial;
- collect independent compiler/test/lint/sandbox observations;
- score TP/FP/FN, precision, recall, Unknown/fallback and explanation coverage.

## WU3 — Controlled mutation corpus

- add explicit mutations for grounding mismatch, semantic-call change, architecture boundary violation and at least one scheduling/read-set case;
- every mutation declares expected outcomes before execution;
- false negatives must remain visible and cannot be hidden by aggregate precision.

## WU4 — Platform equivalence + historical bootstrap

- run platform-neutral self corpus on representative Linux/macOS/Windows environments;
- normalize legitimate platform mechanics and compare canonical/derived semantics;
- replay selected historical changes without leaking successor outcomes into prediction;
- emit evidence suitable for the strategic post-e76 review and future M13 datasets.

## Closure gate

CogniCode can self-evaluate without circular self-judgement, and OS differences do not silently become semantic knowledge differences.
