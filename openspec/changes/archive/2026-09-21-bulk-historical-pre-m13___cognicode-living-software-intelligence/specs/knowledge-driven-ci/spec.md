# Knowledge Driven CI Specification

## Purpose

Define affected-work planning from semantic changes.

## Requirements

### Requirement: Explainable scheduling

Every scheduled analysis/test job MUST expose why it was selected and which changed/read dependencies caused it.

#### Scenario: Why scheduled

- GIVEN a selected test job
- WHEN explanation is requested
- THEN at least one semantic/change dependency path justifies selection

### Requirement: Conservative fallback

When selection confidence is below policy threshold the planner MUST schedule the configured conservative fallback.

#### Scenario: Low confidence full suite

- GIVEN semantic provider degradation
- WHEN confidence falls below threshold
- THEN required full suite/fallback jobs are selected
