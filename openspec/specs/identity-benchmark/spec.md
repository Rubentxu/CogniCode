# identity-benchmark Specification

## Purpose

The identity benchmark harness: pinned precision, recall, and retention scoring gates over ground-truth fixtures, separate reporting of ambiguous outcomes, and a pinned fingerprint/matcher convention digest independent of the equivalence harness's fact-identity digest. Matcher semantics live in main spec `entity-continuity`.

## Requirements

### Requirement: Pinned scoring gates and fixture coverage

The harness MUST declare pinned constants before scoring — rename precision ≥95%, recall ≥90%, line-shift identity retention 100%, file-move retention ≥99% — scoring ground-truth fixtures covering rename/move with and without edits, colliding names, line shift, and an unchanged control. A run MUST fail when any gate is missed; `Ambiguous` outcomes MUST count as neither match nor miss and be reported separately.

#### Scenario: Missed gate fails the run

- GIVEN one declared gate not met
- WHEN the harness runs
- THEN the run fails, naming the gate and its measured value

#### Scenario: Ambiguous outcome reported separately

- GIVEN the colliding-names fixture expecting ambiguity
- WHEN the harness scores results
- THEN the case is reported as ambiguous, excluded from precision and recall

### Requirement: Separate pinned convention digest

The harness MUST pin its own fingerprint/matcher convention digest, separate from the equivalence harness's fact-identity digest; changing the convention MUST fail the run until explicitly re-pinned.

#### Scenario: Convention change requires explicit re-pin

- GIVEN a change to the fingerprint or matcher convention
- WHEN the harness runs
- THEN it fails until its own digest is explicitly re-pinned
