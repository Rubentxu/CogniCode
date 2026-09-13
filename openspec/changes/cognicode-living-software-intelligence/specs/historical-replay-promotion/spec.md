# Historical Replay Promotion Specification

## Purpose

Define OPTIMIZE/CONFIRM discipline for self-improving analyzers/CI.

## Requirements

### Requirement: Dataset separation

OPTIMIZE and CONFIRM sets MUST be disjoint and immutable for an evaluation run.

#### Scenario: Overlap rejected

- GIVEN overlapping case ids
- WHEN evaluation starts
- THEN configuration error is returned before candidate evaluation

### Requirement: Held-out regression blocks promotion

A candidate MUST NOT be promoted when confirm policy shows unacceptable regression even if optimize improved.

#### Scenario: Optimize win confirm loss

- GIVEN positive optimize delta and negative confirm delta beyond tolerance
- WHEN promotion gate runs
- THEN candidate is rejected
