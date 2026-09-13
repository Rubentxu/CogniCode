# Change Proposal Authority Specification

## Purpose

Define code authorship without direct authority.

## Requirements

### Requirement: Proposal lifecycle

AI/plugin-generated code, detector, policy or config changes MUST be represented as ChangeProposal before authority-bearing application.

#### Scenario: Bypass rejected

- GIVEN an agent-generated SourcePatch
- WHEN it requests direct application
- THEN policy rejects it unless the explicit promotion contract has been satisfied

### Requirement: Promotion evidence

A promoted proposal MUST reference the trial/gate evidence used for promotion.

#### Scenario: Promotion audit

- GIVEN a proposal is promoted
- WHEN lineage is inspected
- THEN static/sandbox/CI/policy verdicts are linked
