# Executable Architecture Knowledge Specification

## Purpose

Define approved architecture constraints derived from knowledge/ADRs.

## Requirements

### Requirement: ADR does not auto-execute

Free-form ADR text MUST NOT automatically become a blocking constraint without an explicit candidate/approval step.

#### Scenario: Candidate constraint approval

- GIVEN an ADR extractor proposes a constraint
- WHEN no approval exists
- THEN it remains non-blocking

### Requirement: Drift links intention to evidence

A drift finding MUST link the approved constraint, source ADR and violating graph evidence.

#### Scenario: Boundary violation trace

- GIVEN an approved boundary rule and violating PR edge
- WHEN architecture analysis runs
- THEN finding references ADR, constraint and edge
