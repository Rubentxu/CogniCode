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

### Requirement: Cohort catalog and projection contract

The registry MUST admit the cohort-1 algorithms (PageRank, strongly
connected components, weakly connected components, bounded shortest paths),
the cohort-2 algorithms (dominators, articulation points, bridges, k-core),
AND the cohort-3 algorithms (personalized PageRank, conductance, modularity).
Every run MUST use its pinned workspace and revision with the descriptor's
projection assumptions. Dominators MUST be directed and root-parametrized;
articulation points, bridges, and k-core MUST operate on the undirected
projection; personalized PageRank MUST accept an optional personalization
vector; conductance and modularity MUST accept a community assignment.
Incompatible direction, weighting, heterogeneity, edge orientation,
missing personalization vector, or missing community assignment MUST fail
before execution.

#### Scenario: Cohort algorithms honor graph semantics

- GIVEN the directed projection `A↔B`, `B→C`
- WHEN PageRank, SCC, WCC, and shortest path `A→C` with `max_hops=2` run
- THEN PageRank emits finite scores for all nodes, SCC emits `{A,B},{C}`, WCC emits `{A,B,C}`, and the path is `[A,B,C]`

#### Scenario: Non-admitted algorithm is rejected

- GIVEN `betweenness` is not admitted
- WHEN execution is requested
- THEN the registry returns `NotAdmitted` and no run begins

#### Scenario: Projection mismatch fails closed

- GIVEN a descriptor requiring outgoing `calls` edges and a projection that cannot guarantee their orientation
- WHEN execution is requested
- THEN the registry returns `ProjectionMismatch` and produces no result

#### Scenario: Cohort-3 is listed

- GIVEN the production registry
- WHEN its catalog is listed
- THEN personalized PageRank, conductance, and modularity appear with stable, conformant descriptors

#### Scenario: Missing personalization vector is rejected

- GIVEN personalized PageRank without a personalization vector when the descriptor requires one
- WHEN execution is requested
- THEN the registry returns `InvalidParameter(MissingPersonalizationVector)` and no run begins

### Requirement: Personalized PageRank algorithm

The system MUST admit a personalized PageRank descriptor that accepts an
optional personalization vector alongside the standard PageRank parameters.
The descriptor MUST share the cohort-1 PageRank pure function (no duplicated
math), MUST expose its own stable identity `personalized_pagerank@1.0.0`,
MUST preserve `pagerank@1.0.0` backward compatibility (existing callers
SHALL observe identical behavior), and MUST support the same modes as
PageRank.

#### Scenario: Personalized PageRank biases scores by the personalization vector

- GIVEN a connected projection and a personalization vector placing full weight on node `A`
- WHEN personalized PageRank runs in `stream` mode
- THEN `A` receives the highest score and all scores are finite and deterministic

#### Scenario: PageRank backward compatibility is preserved

- GIVEN a cohort-1 caller invoking `pagerank@1.0.0` without a personalization vector
- WHEN PageRank runs
- THEN output is identical to the pre-E28.6 result and the cohort-1 descriptor is unchanged

### Requirement: Conductance algorithm

The system MUST admit a conductance metric descriptor that computes
per-community conductance scores given an adjacency view and a community
assignment. The descriptor MUST support `stats` mode, MUST be deterministic
and sorted by community identifier, and MUST NOT mutate canonical graph
facts.

#### Scenario: Conductance emits per-community scores

- GIVEN an adjacency view and `community_of: Vec<usize>` assigning each node to a community
- WHEN conductance runs in `stats` mode
- THEN one finite score per community is emitted, sorted by community identifier

#### Scenario: Single-community graph yields empty conductance

- GIVEN a graph where every node belongs to a single community
- WHEN conductance runs
- THEN the result is empty, non-truncated, and the run completes without error

### Requirement: Modularity algorithm

The system MUST admit a modularity descriptor that computes a single
modularity score for a graph given a community assignment. The descriptor
MUST support `stats` mode, MUST be deterministic, and MUST NOT mutate
canonical graph facts.

#### Scenario: Modularity is finite and bounded

- GIVEN an adjacency view and a community assignment
- WHEN modularity runs in `stats` mode
- THEN a single finite score in `[-1, 1]` is emitted within the declared tolerance

#### Scenario: Trivial partition yields near-zero modularity

- GIVEN a graph where every node is its own community
- WHEN modularity runs
- THEN the score equals `0.0` within the declared numeric tolerance

### Requirement: RunOutput variants for cohort-3 metrics

The `RunOutput` enum MUST admit new typed variants for conductance and
modularity runs. Each variant MUST be schema-validated against the
corresponding descriptor's output schema. Existing `RunOutput` variants
MUST remain backward compatible.

#### Scenario: Conductance output is schema-valid

- GIVEN a conductance run completes
- WHEN the run record is read
- THEN the `RunOutput` variant carries one typed per-community score set matching the descriptor schema

#### Scenario: Modularity output is schema-valid

- GIVEN a modularity run completes
- WHEN the run record is read
- THEN the `RunOutput` variant carries a single typed modularity score matching the descriptor schema

### Requirement: Production composition root

The system MUST expose a single production composition root that admits the
cohort-1 algorithms (PageRank, SCC, WCC, bounded shortest paths), the
cohort-2 algorithms (dominators, articulation points, bridges, k-core), AND
the cohort-3 algorithms (personalized PageRank, conductance, modularity).
Production builds MUST NOT depend on test-only `.admit()` calls; the
composition root MUST be the single source of the admitted catalog used by
REST, MCP, and Explorer.

#### Scenario: Composition root lists all eleven algorithms

- GIVEN a freshly constructed production composition root
- WHEN its catalog is enumerated
- THEN it lists exactly the four cohort-1, four cohort-2, and three cohort-3 algorithms

#### Scenario: Production registry is non-empty at startup

- GIVEN any REST, MCP, or Explorer service handle
- WHEN its registry is inspected at startup
- THEN it is the non-empty composition-root registry, not the empty default

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
