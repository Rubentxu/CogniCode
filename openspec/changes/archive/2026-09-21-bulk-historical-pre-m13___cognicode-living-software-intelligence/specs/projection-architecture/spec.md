# Projection Architecture Specification

## Purpose

Make graph and compatibility models reconstructible from canonical facts.

## Requirements

### Requirement: CallGraph projection equivalence

The platform MUST provide a CallGraph projection whose observable output is compared against the legacy CallGraph on golden fixtures before cutover.

#### Scenario: Golden equivalence

- GIVEN a golden repository
- WHEN legacy and projected CallGraphs are built
- THEN their normalized nodes/edges satisfy the declared equivalence contract

### Requirement: Derived projections are rebuildable

Projection data SHALL be disposable and rebuildable from pinned canonical inputs.

#### Scenario: Projection rebuild

- GIVEN projection storage is cleared
- WHEN rebuild is requested for a snapshot
- THEN the resulting projection is equivalent to the prior projection
