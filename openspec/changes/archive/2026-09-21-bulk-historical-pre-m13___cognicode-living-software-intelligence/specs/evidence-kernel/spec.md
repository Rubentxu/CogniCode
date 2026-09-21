# Evidence Kernel Specification

## Purpose

Define canonical fact, evidence, provenance and snapshot semantics.

## Requirements

### Requirement: Canonical fact provenance

Every persisted Fact MUST reference a Snapshot and ProvenanceRecord. LLM-originated output MUST NOT be persisted as an extracted Fact unless an independent deterministic verifier produces that Fact.

#### Scenario: Fact round-trip preserves provenance

- GIVEN a Fact with typed value and provenance
- WHEN it is persisted and reloaded
- THEN subject, predicate, object, snapshot, producer and provenance class are identical

#### Scenario: LLM output stays a hypothesis

- GIVEN an AgentBehavior produces a semantic claim
- WHEN the output is committed
- THEN it is stored as Hypothesis or AgentEvidence and no extracted Fact is created

### Requirement: Snapshot-pinned reads

All canonical reads SHALL pin one workspace and one Snapshot. Implementations MUST NOT silently mix facts from different snapshots.

#### Scenario: Historical read remains stable

- GIVEN snapshot A and later snapshot B
- WHEN a query is pinned to A after B is published
- THEN it returns the same logical result as before B existed
