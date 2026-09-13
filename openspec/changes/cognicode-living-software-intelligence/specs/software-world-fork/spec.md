# Software World Fork Specification

## Purpose

Define isolated counterfactual worlds and promotion.

## Requirements

### Requirement: Fork isolation

A fork MUST share immutable ancestry but SHALL NOT mutate parent state until promotion.

#### Scenario: Fork change isolated

- GIVEN parent P and fork F
- WHEN F applies a candidate semantic model
- THEN P remains unchanged

### Requirement: Three-way promotion

Promotion MUST compare fork base, parent current and fork current and MUST fail closed on conflicting changes.

#### Scenario: Conflict rejects atomic promotion

- GIVEN parent and fork changed same governed entity
- WHEN promote is attempted
- THEN no partial changes are applied and a conflict result is recorded
