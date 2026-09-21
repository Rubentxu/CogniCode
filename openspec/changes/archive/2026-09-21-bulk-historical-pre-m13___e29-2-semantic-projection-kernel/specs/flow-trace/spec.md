# Flow Trace Specification

## Purpose
Define ordered, evidence-only traces for vertical-slice, sequence, and data-flow views. Every step MUST carry provenance; a step without provenance MUST be rejected or reported unsupported, never synthesized.

## ADDED Requirements

### Requirement: Ordered evidence trace
A `FlowTrace` MUST preserve the ordered participants and messages established by evidence. Every step MUST carry provenance; a step without provenance MUST be rejected or reported unsupported, never synthesized.

#### Scenario: Ordered flow
- GIVEN evidence records A calls B before B writes D, followed by D read by C
- WHEN a flow trace is projected
- THEN participants and steps retain that exact order and each step exposes its provenance

#### Scenario: Missing ordering evidence
- GIVEN A, B, and C are related but no evidence establishes their execution order
- WHEN a sequence projection is requested
- THEN the projection reports unsupported or insufficient evidence and emits no ordered sequence

### Requirement: Flow view boundaries
Vertical-slice, use-case, and data-flow projections MUST expose only evidenced participants and transitions; they MUST preserve gaps rather than filling them with inferred structure.

#### Scenario: Vertical slice with a gap
- GIVEN an entry point reaches use case U, but no evidenced transition connects U to repository R
- WHEN the vertical slice is projected
- THEN the trace ends at U or marks the gap explicitly, and MUST NOT add U→R

#### Scenario: Empty flow
- GIVEN a valid entry point has no evidenced outgoing flow
- WHEN a flow trace is requested
- THEN it returns an honest empty or unsupported status with provenance metadata

### Requirement: Trace truncation
A truncated trace MUST identify that truncation occurred and the affected limit or boundary; truncation MUST NOT be silent.

#### Scenario: Bounded trace
- GIVEN evidence contains more steps than the configured limit
- WHEN the trace is projected
- THEN returned steps retain order and the envelope reports `truncated=true` with a reason
