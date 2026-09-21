# Ingest Pipeline Integrity Specification

## Purpose

Define workspace isolation, change notification, and progress guarantees for
streaming ingest.

## ADDED Requirements

### Requirement: Workspace-scoped upsert identity

Node upserts MUST identify an existing node by
`(workspace_id, id, kind)`. Edge upserts MUST identify an existing edge by
`(workspace_id, source_id, source_kind, target_id, target_kind, kind)`. An
upsert in one workspace MUST NOT conflict with or modify graph data in another
workspace.

#### Scenario: Equal identities coexist across workspaces

- GIVEN two workspaces ingest nodes and edges with equal local identities
- WHEN both ingests complete
- THEN each workspace retains its own nodes and edges
- AND neither workspace's values overwrite the other's values

#### Scenario: Repeated identity updates within one workspace

- GIVEN one workspace already contains a node and edge matching their upsert identities
- WHEN ingest submits both identities again with changed values
- THEN both existing workspace-local graph facts are updated
- AND no duplicate fact or uniqueness failure is produced

### Requirement: Entity-specific change notifications

Node and edge notification triggers MUST have separate entity contracts. Each
successful mutation MUST publish a distinguishable notification for its entity
class and workspace. Each trigger MUST use only data available on its mutated
entity and MUST NOT invalidate an otherwise valid mutation.

#### Scenario: Node and edge changes are distinguishable

- GIVEN a workspace receives one successful node mutation and one edge mutation
- WHEN change notifications are observed
- THEN one node notification and one edge notification identify that workspace
- AND consumers can distinguish the affected entity classes

#### Scenario: Edge notification needs no node-only metadata

- GIVEN a valid edge mutation contains no node-specific path metadata
- WHEN the edge is persisted
- THEN persistence succeeds and an edge change notification is emitted
- AND no notification-field error occurs

### Requirement: Streaming ingest makes progress under backpressure

Ingest MUST complete a stream larger than its in-flight capacity without
deadlock, unbounded buffering, or loss of accepted results. If downstream
persistence fails, the ingest service MUST return a typed failure within the
configured timeout instead of hanging. Durable Job terminal-state semantics are
owned by E29.4.

#### Scenario: More than ten source files complete

- GIVEN eleven extractable source files produce more results than the in-flight capacity, with a known expected count
- WHEN ingest runs against a healthy database
- THEN its Job reaches success within the configured timeout
- AND the persisted result count equals the fixture's expected result count

#### Scenario: A slow consumer applies backpressure

- GIVEN extraction temporarily produces results faster than persistence accepts them
- WHEN the in-flight capacity is reached
- THEN ingest limits queued work and resumes as capacity becomes available
- AND the Job completes without dropping accepted results

#### Scenario: Persistence failure returns without hanging

- GIVEN persistence rejects an accepted ingest result
- WHEN the streaming ingest processes that result
- THEN the ingest service returns a typed persistence failure within the configured timeout
- AND the pipeline does not remain blocked or continue accepting results
