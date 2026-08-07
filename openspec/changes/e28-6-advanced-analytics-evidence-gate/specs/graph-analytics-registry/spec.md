# Delta for graph-analytics-registry

> Change: `e28-6-advanced-analytics-evidence-gate`
> Modifies: `openspec/specs/graph-analytics-registry/spec.md`
> Depends on: `e28-5-structural-analytics-cohort-2` (pending archive)

## ADDED Requirements

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

## MODIFIED Requirements

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
(Previously: admitted cohort-1 + cohort-2 — eight algorithms total.)

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

### Requirement: Production composition root

The system MUST expose a single production composition root that admits the
cohort-1 algorithms (PageRank, SCC, WCC, bounded shortest paths), the
cohort-2 algorithms (dominators, articulation points, bridges, k-core), AND
the cohort-3 algorithms (personalized PageRank, conductance, modularity).
Production builds MUST NOT depend on test-only `.admit()` calls; the
composition root MUST be the single source of the admitted catalog used by
REST, MCP, and Explorer.
(Previously: admitted cohort-1 + cohort-2 — eight algorithms total.)

#### Scenario: Composition root lists all eleven algorithms

- GIVEN a freshly constructed production composition root
- WHEN its catalog is enumerated
- THEN it lists exactly the four cohort-1, four cohort-2, and three cohort-3 algorithms

#### Scenario: Production registry is non-empty at startup

- GIVEN any REST, MCP, or Explorer service handle
- WHEN its registry is inspected at startup
- THEN it is the non-empty composition-root registry, not the empty default