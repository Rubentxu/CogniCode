# Type Hierarchy Projection Specification

## Purpose
Define validated semantic projections for typed inheritance, implementation, and membership relations. Relations whose endpoint or relation kind is not typed evidence MUST be rejected or marked unsupported.

## ADDED Requirements

### Requirement: Typed hierarchy fidelity
A `TypeHierarchyModel` MUST preserve the identity and kind of every evidenced `inherits`, `implements`, and `member` relation. It MUST reject or mark unsupported any relation whose endpoint or relation kind is not typed evidence.

#### Scenario: Complete typed hierarchy
- GIVEN trait T, type A implements T, type B inherits A, and A has member m
- WHEN the hierarchy is projected
- THEN it contains exactly `A implements T`, `B inherits A`, and `A member m` with their original identities and kinds

#### Scenario: Unrelated dependency
- GIVEN A imports module M but no inheritance, implementation, or membership evidence exists
- WHEN a type hierarchy is projected
- THEN the import is excluded and no hierarchy relation is synthesized

### Requirement: Identity and parent fidelity
Hierarchy nodes MUST retain exact source identities and parent relationships; duplicate identities MUST NOT be silently merged when their evidence distinguishes them.

#### Scenario: Same-named types
- GIVEN two types named `User` exist in distinct qualified scopes
- WHEN the hierarchy is projected
- THEN both qualified identities remain distinct and their relations attach to the correct parent

#### Scenario: Missing type endpoint
- GIVEN an implements edge references an unresolved target
- WHEN projection is requested
- THEN the result reports incomplete or unsupported status and MUST NOT invent the missing type

### Requirement: Honest bounded results
Hierarchy confidence, provenance, and truncation MUST be exposed, and a hierarchy with no qualifying typed evidence MUST be distinguishable from an unsupported capability.

#### Scenario: Truncated hierarchy
- GIVEN qualifying relations exceed a configured result limit
- WHEN the hierarchy is projected
- THEN retained relations remain exact and ordered only where evidence orders them, and truncation is explicitly reported
