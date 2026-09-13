# Semantic Provider Pipeline Specification

## Purpose

Define multi-tier semantic observations and graceful fallback.

## Requirements

### Requirement: Provider precision is explicit

Every semantic observation MUST record provider id/version and precision tier.

#### Scenario: Fallback is visible

- GIVEN a higher-tier provider is unavailable
- WHEN a lower-tier fallback returns a result
- THEN the result includes fallback diagnostics and reduced/declared precision

### Requirement: Unknown beats fabricated resolution

A provider SHALL return unresolved/ambiguous when it cannot support a binding; it MUST NOT fabricate a target to satisfy graph completeness.

#### Scenario: Unresolved call

- GIVEN a dynamic call with insufficient evidence
- WHEN resolution runs
- THEN no precise target Fact is emitted and uncertainty is recorded
