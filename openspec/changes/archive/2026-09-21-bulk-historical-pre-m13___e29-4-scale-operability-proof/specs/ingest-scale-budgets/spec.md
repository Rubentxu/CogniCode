# Ingest Scale Budgets Specification

## Purpose
Define deterministic ingest and graph query/render performance proof at representative repository sizes.

## ADDED Requirements

### Requirement: Deterministic fixture budgets
The system MUST evaluate seeded, reproducible fixtures of 10 MB and 100 MB in the required CI lane, and MAY evaluate a generated 1 GB fixture in an optional lane. Full ingest and incremental ingest MUST publish wall-time and p99 measurements against declared budgets.

#### Scenario: CI fixture sizes
- GIVEN the scale lane is run with the prescribed seed
- WHEN 10 MB and 100 MB fixtures are generated
- THEN identical seed and parameters produce identical file manifests and graph facts
- AND both runs report pass/fail against wall-time and p99 budgets

#### Scenario: Optional large fixture
- GIVEN the optional 1 GB lane is enabled
- WHEN its generated fixture is executed
- THEN the fixture is created on demand, is not required in the repository, and emits the same measurements

### Requirement: Incremental change-ratio budgets
Incremental ingest MUST be measured for 1%, 10%, and 50% changed-file ratios on each required fixture, with each result compared to its declared delta budget.

#### Scenario: Incremental ratios
- GIVEN a deterministic baseline fixture
- WHEN exactly 1%, 10%, or 50% of its files are changed
- THEN the resulting ingest completes and records the selected ratio and budget verdict

#### Scenario: No-op baseline
- GIVEN no fixture files changed
- WHEN incremental ingest runs
- THEN it reports a successful no-op without exceeding the incremental budget

### Requirement: Query and render budgets
Graph query and render operations MUST meet declared budgets for 1,000-node and 5,000-node graphs, including wall-time and p99 evidence.

#### Scenario: Supported graph sizes
- GIVEN a graph containing 1,000 or 5,000 nodes
- WHEN the standard query and render workload runs
- THEN each workload reports measurements and passes only when its budget is met

#### Scenario: Budget breach
- GIVEN a workload exceeds a declared wall-time or p99 budget
- WHEN the scale proof completes
- THEN the run is marked failed and identifies the breached size and metric
