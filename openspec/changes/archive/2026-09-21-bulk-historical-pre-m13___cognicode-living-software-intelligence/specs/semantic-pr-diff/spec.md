# Semantic PR Diff Specification

## Purpose

Define knowledge-level comparison of a PR world against base.

## Requirements

### Requirement: Multi-dimensional diff

Semantic PR Diff MUST report entity/relation changes and SHOULD report architecture/security/test/dependency impacts where capability data exists.

#### Scenario: Architecture edge appears

- GIVEN a PR introduces a cross-component dependency
- WHEN semantic diff runs
- THEN the new dependency edge and affected components are reported

### Requirement: Uncertainty is explicit

Missing providers/data SHALL produce an uncertainty section instead of silently reporting no impact.

#### Scenario: Missing runtime evidence

- GIVEN no runtime evidence exists
- WHEN PR diff is generated
- THEN runtime section reports unavailable/unknown rather than safe
