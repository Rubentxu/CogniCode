# Stable Entity Identity Specification

## Purpose

Define durable entity identity across source occurrences.

## Requirements

### Requirement: Entity and occurrence separation

The platform MUST model logical Entity identity separately from per-snapshot Occurrence location.

#### Scenario: Line shift preserves identity

- GIVEN a function and EntityId
- WHEN unrelated lines are inserted before the function
- THEN the new Occurrence references the same EntityId

#### Scenario: Ambiguous continuity fails closed

- GIVEN two equally plausible rename targets
- WHEN continuity cannot exceed the configured margin
- THEN the matcher returns Ambiguous and creates no forced identity merge
