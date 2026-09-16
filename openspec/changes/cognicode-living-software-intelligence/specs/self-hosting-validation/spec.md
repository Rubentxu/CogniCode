# Self-Hosting Validation Harness Specification

## Purpose

Make CogniCode evaluate CogniCode while preventing circular self-validation. The harness measures whether CogniCode's predictions about its own code agree with declared ground truth and independent external observations.

## Requirements

### Requirement SHV-001: Prediction is sealed before observation

The harness MUST persist/identify the CogniCode prediction from the base snapshot before the candidate change/trial is observed.

#### Scenario: Impact prediction cannot be rewritten after tests

- GIVEN a base snapshot and proposed controlled mutation
- WHEN CogniCode predicts affected work
- THEN the prediction record is fixed before executing the mutation/trial
- AND later observations cannot mutate the original prediction.

### Requirement SHV-002: CogniCode is not its own sole oracle

A self-hosting evaluation MUST distinguish CogniCode prediction, declared scenario ground truth where available, and independent observations such as compiler/tests/linters/sandbox.

#### Scenario: CogniCode misses a compiler-visible defect

- GIVEN CogniCode predicts a change safe
- AND rustc/cargo tests observe a contract-breaking failure
- WHEN the run is scored
- THEN the miss is recorded as a false negative/regression
- AND CogniCode's own safe prediction cannot override the independent observation.

### Requirement SHV-003: Self-model determinism

Repeated analysis of the same CogniCode source/config snapshot MUST produce equivalent canonical knowledge after normalizing legitimate runtime metadata.

#### Scenario: Repeated self scan

- GIVEN unchanged CogniCode source/config
- WHEN two self-host scans run
- THEN canonical Fact semantic digests and derived semantic views are equivalent
- AND differences in allocation/order/timestamps do not become semantic drift.

### Requirement SHV-004: Controlled mutation corpus

The harness MUST support explicit mutations/fault scenarios with expected outcomes for architecture, grounding, semantic deltas, affected work and/or policy decisions.

#### Scenario: Cross-layer violation

- GIVEN a mutation that makes domain code depend on infrastructure
- WHEN the self-hosting scenario executes
- THEN its ground-truth contract declares an architecture violation
- AND CogniCode detection is scored against that declaration and external build/test observations.

### Requirement SHV-005: Impact quality metrics

The harness MUST report at least true positives, false positives, false negatives, precision, recall, Unknown/fallback rate and explanation coverage for affected-work predictions where ground truth is available.

#### Scenario: False negative is visible

- GIVEN a test/work item actually affected by a controlled change
- AND CogniCode predicted it Unaffected
- WHEN results are evaluated
- THEN false-negative count increments
- AND the run cannot present scheduling precision alone as successful.

### Requirement SHV-006: Cross-platform semantic equivalence

For a platform-neutral corpus, Linux/macOS/Windows runs MUST compare normalized semantic outputs so OS mechanics do not silently alter canonical knowledge.

#### Scenario: Windows path representation

- GIVEN the same logical source tree analysed on Windows and Linux
- WHEN platform-equivalence comparison runs
- THEN path separator/drive representation is normalized at the platform boundary
- AND equivalent source produces equivalent canonical Fact semantics and derived decisions unless a declared platform-specific rule applies.

### Requirement SHV-007: Self-hosting composes M9 trials

Controlled candidate changes SHOULD execute through SoftwareWorld/ChangeProposal/Trial once M9 is available instead of mutating the developer workspace directly.

#### Scenario: Mutation isolation

- GIVEN a self-hosting mutation scenario
- WHEN the experiment executes
- THEN the base world remains unchanged
- AND observations/evidence are attached to the trial/candidate world.

### Requirement SHV-008: Existing external sandbox remains independent

Self-hosting MUST complement, not replace, validation against pinned external repositories. Release readiness may consume evidence from both.

### Requirement SHV-009: Historical replay bootstrap

Self-hosting SHOULD be able to replay selected historical CogniCode changes as prediction-vs-observation cases, with future OPTIMIZE/CONFIRM separation delegated to the M13 historical-replay contract.

#### Scenario: Historical change

- GIVEN a base commit and successor change
- WHEN a replay case is constructed
- THEN prediction is made from the base state
- AND observation is collected from the successor/trial without leaking successor outcomes into prediction.
