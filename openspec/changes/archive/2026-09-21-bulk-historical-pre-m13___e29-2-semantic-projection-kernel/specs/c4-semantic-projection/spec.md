# C4 Semantic Projection Specification

## Purpose
Define evidence-grounded Context, Container, Component, Code, and actor identities without allowing renderers to invent architecture. Sources MAY include manifests, IaC, and routes, but an identity MUST NOT be emitted solely because a renderer expects a C4 level.

## ADDED Requirements

### Requirement: C4 identity projection
The C4 projection MUST preserve evidenced identities for actors, systems, containers, components, and code elements. Sources MAY include manifests, IaC, and routes, but an identity MUST NOT be emitted solely because a renderer expects a C4 level.

#### Scenario: Evidence-backed levels
- GIVEN a route identifies actor A and system S, IaC identifies container C, and code identifies component P and code element F
- WHEN C4 is projected
- THEN each identity is emitted at its evidenced level with its supporting provenance

#### Scenario: Missing level evidence
- GIVEN a system has no evidenced component decomposition
- WHEN a component projection is requested
- THEN the capability reports unsupported or unavailable and emits no fabricated components

### Requirement: Human override precedence
A valid human override MUST be represented as explicit evidence and MUST take precedence over conflicting inferred classification while preserving the underlying provenance and conflict visibility.

#### Scenario: Override classification
- GIVEN code evidence classifies X as a component and a human override classifies X as a container
- WHEN C4 is projected
- THEN X is presented as the overridden container and the projection retains both override and source provenance

#### Scenario: Invalid override
- GIVEN an override refers to an unknown identity
- WHEN projection is requested
- THEN the override is rejected or reported invalid and MUST NOT create an identity

### Requirement: Confidence, provenance, and truncation
Every C4 result MUST expose confidence in `[0,1]`, provenance for each identity or relation, and explicit truncation. The projection MUST NOT apply a silent fixed cap.

#### Scenario: Large result
- GIVEN more C4 identities exist than the requested limit
- WHEN projection is performed
- THEN the result reports truncation and its reason, while retained identities keep exact evidence

#### Scenario: Low confidence
- GIVEN an identity is supported only by weak evidence
- WHEN C4 is projected
- THEN its bounded confidence and provenance are exposed; it MUST NOT be presented as certain
