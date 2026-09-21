# Evidence Bundle Gates Specification

## Purpose

Define evidence-based CI/release decisions.

## Requirements

### Requirement: Evidence bundle

A promotion gate MUST evaluate a snapshot-scoped bundle of available evidence and unresolved uncertainty.

#### Scenario: Missing mandatory evidence blocks

- GIVEN policy requires security grade B and test evidence
- WHEN test evidence is absent
- THEN gate does not pass and reports missing evidence

### Requirement: Legacy job compatibility

Existing CI jobs MAY act as evidence producers during migration.

#### Scenario: Legacy test mapped

- GIVEN existing cargo test job succeeds
- WHEN evidence bundle is built
- THEN its result can be represented as TestResultEvidence
