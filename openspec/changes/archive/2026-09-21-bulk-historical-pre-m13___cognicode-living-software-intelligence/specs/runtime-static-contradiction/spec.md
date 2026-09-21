# Runtime Static Contradiction Specification

## Purpose

Define reconciliation of static and observed runtime knowledge.

## Requirements

### Requirement: Contradiction preserved

Runtime observation conflicting with static model MUST create explicit contradiction/diagnostic evidence and MUST NOT silently overwrite the static claim.

#### Scenario: Observed missing edge

- GIVEN runtime trace A->B and no resolved static edge
- WHEN reconciliation runs
- THEN a contradiction/hypothesis identifies possible dynamic-dispatch or model gap
