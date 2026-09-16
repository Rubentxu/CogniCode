# Proposal — e76 LSI Self-Hosting + Platform Equivalence Harness

> Planned cycle; depends on M9 Trial and e74/e75 portability capabilities.

## Goal

Use CogniCode to evaluate CogniCode while measuring the correctness of CogniCode's predictions against independent ground truth/oracles and across supported platforms.

## In scope

- deterministic self-model baseline;
- sealed prediction record before candidate observation;
- prediction-vs-observation evaluator;
- controlled mutation/fault corpus;
- impact metrics including false negatives and Unknown/fallback rate;
- cross-platform semantic-equivalence harness;
- historical replay bootstrap compatible with future M13 OPTIMIZE/CONFIRM separation;
- integration with existing sandbox/equivalence harnesses without replacing them.

## Out of scope

- autonomous AI code promotion;
- training/fine-tuning loops;
- replacing rustc/cargo/tests/clippy/sandbox as independent oracles;
- declaring self-produced CogniCode scores authoritative by themselves.

## Exit condition

At least one end-to-end self-hosted trial seals a prediction from a base CogniCode snapshot, executes an isolated controlled change, evaluates independent observations, reports impact-quality metrics, and demonstrates semantic equivalence on representative supported platforms or records explicit platform waivers.

## Governing umbrella spec

`self-hosting-validation` in the Living Software Intelligence umbrella.
