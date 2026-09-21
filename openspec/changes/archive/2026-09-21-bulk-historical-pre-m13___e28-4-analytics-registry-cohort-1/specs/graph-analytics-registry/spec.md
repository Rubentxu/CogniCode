# Graph Analytics Registry Specification

## Purpose

Define governed admission and user-facing execution for cohort-1 graph analytics
without permitting analytics to alter canonical graph facts.

## ADDED Requirements

### Requirement: Descriptor-driven admission

The registry MUST admit an algorithm only when its descriptor declares a stable
identity, version, maturity, determinism and seed policy, directed/weighted/
heterogeneous graph traits, projection assumptions, parameter and output
schemas, supported modes, complexity and limit/truncation policies, and
conformance fixtures. Admission MUST be atomic.

#### Scenario: Complete descriptor is admitted

- GIVEN a cohort-1 descriptor containing every required declaration
- WHEN registration is requested
- THEN the registry admits that exact descriptor version

#### Scenario: Incomplete descriptor is rejected

- GIVEN a descriptor missing its output schema and resource limits
- WHEN registration is requested
- THEN admission returns a structured `DescriptorIncomplete` error listing both fields
- AND the algorithm cannot be listed or executed

### Requirement: Cohort-1 catalog and projection contract

The registry MUST admit only PageRank, strongly connected components (SCC),
weakly connected components (WCC), and bounded shortest paths for cohort 1.
Every run MUST use its pinned workspace and revision with the descriptor's
projection assumptions. Incompatible direction, weighting, heterogeneity, or
edge orientation MUST fail before execution.

#### Scenario: Cohort algorithms honor graph semantics

- GIVEN the directed projection `A↔B`, `B→C`
- WHEN PageRank, SCC, WCC, and shortest path `A→C` with `max_hops=2` run
- THEN PageRank emits finite scores for all nodes, SCC emits `{A,B},{C}`, WCC emits `{A,B,C}`, and the path is `[A,B,C]`

#### Scenario: Non-admitted cohort is rejected

- GIVEN `betweenness` is not admitted in cohort 1
- WHEN execution is requested
- THEN the registry returns `NotAdmitted` and no run begins

#### Scenario: Projection mismatch fails closed

- GIVEN a descriptor requiring outgoing `calls` edges and a projection that cannot guarantee their orientation
- WHEN execution is requested
- THEN the registry returns `ProjectionMismatch` and produces no result

### Requirement: Explicit modes and canonical safety

The platform SHALL support `stream`, `stats`, `annotate`, and separately
authorized `persist`. A request MUST select a mode supported by the descriptor.
No mode MAY mutate canonical nodes, edges, or revisions. `persist` MUST create
only an idempotent derived-analysis record, and derived relationships MUST enter
as relation candidates.

#### Scenario: Stream and stats succeed

- GIVEN an admitted algorithm supporting `stream` and `stats`
- WHEN each mode is requested
- THEN `stream` emits schema-valid typed rows and `stats` emits its schema-valid summary

#### Scenario: Annotation remains ephemeral

- GIVEN an admitted algorithm supporting `annotate`
- WHEN annotation completes
- THEN an overlay is returned and the pinned canonical revision is unchanged

#### Scenario: Authorized persist is derived only

- GIVEN a caller authorized for `persist`
- WHEN a successful run is persisted
- THEN a derived-analysis record is returned and canonical graph facts are unchanged

#### Scenario: Persist authorization fails closed

- GIVEN a caller without `persist` authorization
- WHEN `persist` is requested
- THEN an authorization error is returned and no derived record is created

#### Scenario: Canonical write attempt is rejected

- GIVEN any analytics mode attempts a canonical graph write
- WHEN the execution boundary observes it
- THEN the run fails with `CanonicalWriteViolation` and the revision remains unchanged

### Requirement: REST, MCP, and Explorer parity

REST, MCP, and Explorer MUST expose the same admitted user-facing catalog and
normalized execution outcomes. Explorer MUST provide an entry and inspectable
result for each admitted algorithm.

#### Scenario: Happy-path parity

- GIVEN the same admitted PageRank request through REST, MCP, and Explorer
- WHEN all three complete
- THEN they expose equivalent scores, status, lineage identifier, and tolerance

#### Scenario: Empty-result parity

- GIVEN an empty compatible projection
- WHEN the same cohort-1 request uses all three surfaces
- THEN each reports successful empty output with no truncation

#### Scenario: Error parity

- GIVEN the same non-admitted algorithm request through all three surfaces
- WHEN validation occurs
- THEN each exposes `NotAdmitted` and no execution result

#### Scenario: Truncation parity

- GIVEN the same run reaches a soft result limit
- WHEN observed through all three surfaces
- THEN each visibly exposes the same truncation dimension and incomplete scope
