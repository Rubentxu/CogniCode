# Investigation Persistence Specification

## Purpose
Backend foundation for ADR-005's `Investigation` entity — a durable, goal-driven knowledge artifact bundling pinned evidence and supporting artifacts. Defines the domain entity, PostgreSQL persistence, REST CRUD, and atomic save semantics. UI is out of scope.

## Requirements

### Requirement: Investigation Domain Entity
The system MUST model `Investigation` with `id`, `workspace_id`, `title`, `goal`, `status`, `owner`, `narrative`, `evidence`, `artifacts`, `created_at`, `updated_at`. `InvestigationStatus` MUST include `Draft` and `Active`.

#### Scenario: Construct a draft investigation
- GIVEN a valid title, goal, workspace_id, and owner
- WHEN an `Investigation` is built with status `Draft`
- THEN it carries empty `evidence` and `artifacts` lists

### Requirement: Investigation CRUD
The system MUST expose create, read-by-id, list-by-workspace, update, and delete operations.

#### Scenario: Create persists and returns the entity
- GIVEN a workspace with no existing investigation of that id
- WHEN create is called with a valid title and owner
- THEN the investigation is persisted and the returned entity matches the input

#### Scenario: List scopes by workspace
- GIVEN workspace `ws:A` has two investigations and `ws:B` has one
- WHEN list is called with workspace_id `ws:A`
- THEN exactly the two investigations from `ws:A` are returned

#### Scenario: Read returns NotFound for missing id
- GIVEN no investigation exists with id `inv:nonexistent`
- WHEN read is called with that id
- THEN it returns `NotFound`

### Requirement: Evidence and Artifact Sub-Resources
The system MUST allow evidence to be pinned to and removed from an investigation, and artifacts to be attached to and removed from it.

#### Scenario: Pinning evidence associates it with the investigation
- GIVEN an existing investigation with no evidence
- WHEN pin-evidence is called with a valid item
- THEN the evidence appears in subsequent reads of that investigation

#### Scenario: Adding an artifact associates it with the investigation
- GIVEN an existing investigation
- WHEN add-artifact is called
- THEN the artifact appears in subsequent reads

#### Scenario: Removing a sub-resource disassociates it
- GIVEN an investigation with pinned evidence and attached artifacts
- WHEN remove-evidence or remove-artifact is called
- THEN the resource no longer appears in subsequent reads

### Requirement: Atomic Investigation Save
The system MUST persist an investigation together with its evidence and artifacts inside a single transaction.

#### Scenario: All three tables commit together on success
- GIVEN a new investigation with two evidence items and one artifact
- WHEN the atomic save is invoked
- THEN all three writes commit

#### Scenario: A failure in any sub-table rolls back the whole save
- GIVEN an investigation save where the evidence insert fails
- WHEN the operation is invoked
- THEN neither the investigation row nor the artifact row is committed

### Requirement: Workspace Scoping
The system MUST scope every investigation read, update, and delete by `workspace_id`.

#### Scenario: Cross-workspace read returns NotFound
- GIVEN investigation `inv:1` belongs to `ws:A`
- WHEN read is called with id `inv:1` and workspace_id `ws:B`
- THEN it returns `NotFound`

### Requirement: REST API Surface
The system MUST expose `/api/investigations` plus sub-resource routes for evidence and artifacts, returning JSON on success and structured errors on failure.

#### Scenario: POST creates an investigation with a stable id
- GIVEN a valid request body
- WHEN POST `/api/investigations` is called
- THEN it returns 200 with the persisted investigation including a stable `id`

#### Scenario: GET returns the investigation with its sub-resources
- GIVEN an existing investigation with pinned evidence and artifacts
- WHEN GET `/api/investigations/:id` is called
- THEN it returns 200 with `evidence` and `artifacts` arrays

#### Scenario: PUT updates mutable fields
- GIVEN an existing investigation
- WHEN PUT `/api/investigations/:id` is called with new title and goal
- THEN those fields are persisted and `updated_at` advances

#### Scenario: DELETE removes the investigation and its sub-resources
- GIVEN an investigation with pinned evidence and attached artifacts
- WHEN DELETE `/api/investigations/:id` is called
- THEN the investigation and all its sub-resource rows are removed

### Requirement: ExplorationSession Optional Investigation Link
`ExplorationSession` MUST accept an optional `investigation_id: Option<String>` that defaults to `None` via `#[serde(default)]`.

#### Scenario: Backward-compatible deserialization without investigation_id
- GIVEN an `ExplorationSession` JSON body that omits `investigation_id`
- WHEN it is deserialized
- THEN the field is `None`

#### Scenario: Round-trip preserves investigation_id
- GIVEN an `ExplorationSession` JSON body with `investigation_id: "inv:1"`
- WHEN it is deserialized and re-serialized
- THEN the field is preserved as `"inv:1"`

### Requirement: Operational Guarantees
The investigation table DDL MUST be idempotent, and the system MUST return a soft `FeatureDisabled` error (not panic) when investigation operations are requested with PostgreSQL persistence disabled.

#### Scenario: Re-applying migration does not raise
- GIVEN the migration has already run once
- WHEN it is applied again
- THEN no error is raised

#### Scenario: PG-disabled call returns FeatureDisabled
- GIVEN the explorer is built without the postgres feature
- WHEN an investigation operation is called
- THEN it returns a `FeatureDisabled` error and the caller can recover