# Graph Analytics Run Lineage Specification

## Purpose

Define immutable, reproducible, and queryable evidence for every admitted graph
analytics execution and its optional derived result.

## ADDED Requirements

### Requirement: Complete immutable run record

Every admitted execution that begins MUST create a unique run record and MUST
finalize it with workspace, revision, normalized plan hash, algorithm and
implementation versions, normalized parameters, effective seed, mode,
timestamps, status, warnings, and truncation. Finalized records MUST remain
immutable and queryable.

#### Scenario: Successful run records complete lineage

- GIVEN an admitted PageRank run pinned to workspace `w1`, revision `7`
- WHEN execution succeeds
- THEN its queryable record contains every required field and status `succeeded`

#### Scenario: Failed run retains evidence

- GIVEN an admitted run that exceeds a hard memory limit after starting
- WHEN execution terminates
- THEN its record has status `failed` and identifies the typed limit error
- AND it contains no successful result reference

#### Scenario: Truncated run records incomplete scope

- GIVEN a run that reaches a soft result-row or path-count limit
- WHEN partial output is returned
- THEN its record has status `truncated`, the limit dimension, observed value, and affected scope

#### Scenario: Visited-node breach records failure

- GIVEN a run that exceeds the hard visited-node limit
- WHEN execution terminates
- THEN its record has status `failed` and identifies `LimitExceeded(VisitedNodes)`
- AND it contains no partial-success result reference

### Requirement: Reproducible replay lineage

Each descriptor MUST classify its seed as required, defaulted, or not
applicable, and execution MUST resolve the effective seed before a run record is
created. A replay with the same workspace, revision, plan hash, algorithm and
implementation versions, parameters, and effective seed MUST produce an
equivalent result within the descriptor's tolerance. Each replay MUST retain a
separate run identifier.

#### Scenario: Required seed is absent

- GIVEN an admitted descriptor whose seed policy is required
- WHEN a request omits the seed
- THEN execution returns `MissingSeed` and creates no run record

#### Scenario: Deterministic replay matches

- GIVEN a completed run with effective seed `17`
- WHEN its complete replay identity is submitted again
- THEN the new result is equivalent and both independent run records remain queryable

#### Scenario: Replay identity changes

- GIVEN a prior run and a replay using a different revision or seed
- WHEN the replay begins
- THEN the new record exposes the changed field and is not represented as the same replay identity

### Requirement: Idempotent derived-analysis record

Authorized `persist` MUST derive an idempotency identity from the normalized run
identity. Repeated persistence of that identity MUST return one derived-analysis
record; independent run records MAY reference it. Persistence MUST NOT modify
canonical nodes, edges, or revisions.

#### Scenario: Repeated persist reuses derived record

- GIVEN two successful equivalent runs and authorized `persist`
- WHEN both are persisted with the same normalized identity
- THEN both lineage records reference one derived-analysis record and no duplicate exists

#### Scenario: Idempotency-key conflict fails closed

- GIVEN an existing derived record and the same idempotency key with different normalized parameters
- WHEN persistence is requested
- THEN it returns `IdempotencyConflict` and changes neither derived nor canonical records

### Requirement: Authorized lineage queries

Authorized clients MUST be able to query a run by identifier and filter runs by
workspace, revision, algorithm, status, and time range. Filter results MUST use
a stable order. Empty filters MUST return an empty success; inaccessible
workspaces MUST fail without revealing records.

#### Scenario: Query returns matching lineage

- GIVEN three records and a filter for workspace `w1`, algorithm `pagerank`
- WHEN the query runs
- THEN only matching records are returned in stable newest-first order

#### Scenario: Empty query is successful

- GIVEN no records match an authorized filter
- WHEN the query runs
- THEN it returns an empty, non-truncated collection

#### Scenario: Unknown run identifier is an error

- GIVEN no accessible record has run identifier `missing`
- WHEN that identifier is queried
- THEN the result is typed `RunNotFound`

#### Scenario: Query limit is visible

- GIVEN 100 matching records and a result limit of 20
- WHEN the query runs
- THEN 20 records and an explicit `ResultRowsLimit` truncation marker are returned

### Requirement: REST, MCP, and Explorer lineage parity

REST, MCP, and Explorer MUST expose equivalent authorized lineage detail and
query semantics, including empty, error, and truncated outcomes.

#### Scenario: Surface outcomes agree

- GIVEN one success, one empty filter, one unknown identifier, and one truncated query
- WHEN each case is requested through REST, MCP, and Explorer
- THEN all three expose equivalent records, emptiness, typed error, or truncation respectively
