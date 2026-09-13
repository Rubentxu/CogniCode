# Execution Read Sets Specification

## Purpose

Define observed dependencies of analyses and behaviors.

## Requirements

### Requirement: Read dependency recording

Executions configured for read tracing MUST persist ordered/deduplicated references to canonical knowledge consumed, with an explicit truncation marker if bounded.

#### Scenario: Changed unread fact does not invalidate

- GIVEN execution E read facts A and B only
- WHEN unrelated fact C changes
- THEN read-set invalidation does not mark E stale solely because C changed
