# Delta for graph-analytics-registry

> Change: `e28-5-structural-analytics-cohort-2`
> Modifies: `openspec/specs/graph-analytics-registry/spec.md`
> Depends on: `e28-4-analytics-registry-cohort-1` (shipped)

## ADDED Requirements

### Requirement: Dominators algorithm

The system MUST admit a directed, root-parametrized dominators algorithm.
Its output schema MUST expose `node_id`, `immediate_dominator`, and `depth`.
It MUST support `stream`, `stats`, and `annotate` modes and MUST NOT mutate
canonical graph facts. Output MUST be deterministic once `root` is fixed and
MUST be sorted by `node_id`. If `root` is absent from the projection,
`execute()` MUST return a typed `InvalidParameter` error before any run
record is created.

#### Scenario: Dominators ranks reachable nodes from root

- GIVEN the directed projection `A→B`, `A→C`, `B→C` and `root=A`
- WHEN dominators runs in `stats` mode
- THEN rows expose `A` as self-dominator and `B`, `C` with `A` as immediate dominator
- AND output is sorted by `node_id` and `depth`

#### Scenario: Dominators rejects unknown root

- GIVEN the directed projection `A→B` and `root=missing`
- WHEN dominators runs
- THEN execution returns `InvalidParameter(RootNotInGraph)` and creates no record

### Requirement: Articulation Points algorithm

The system MUST admit an undirected articulation-points algorithm. Its output
schema MUST expose `node_id` and `cut_vertices_count`. It MUST support
`stream`, `stats`, and `annotate` modes and MUST operate on the undirected
projection of the input. Output MUST be sorted by `node_id` and MUST be
deterministic.

#### Scenario: Articulation points identify cut vertices

- GIVEN the undirected projection `{A-B, B-C, C-D, B-D, B-E}`
- WHEN articulation points runs in `stream` mode
- THEN `B` and `E` are emitted with their cut-vertices count
- AND output is sorted by `node_id`

#### Scenario: Articulation points returns empty on a 2-connected graph

- GIVEN the undirected cycle `{A-B, B-C, C-A}`
- WHEN articulation points runs
- THEN the result is empty and non-truncated

### Requirement: Bridges algorithm

The system MUST admit an undirected bridges algorithm. Its output schema MUST
expose `edge_source` and `edge_target`. It MUST support `stream`, `stats`,
and `annotate` modes and MUST operate on the undirected projection of the
input. Output MUST be sorted lexicographically by `(edge_source, edge_target)`
and MUST be deterministic.

#### Scenario: Bridges identify cut edges

- GIVEN the undirected path `{A-B, B-C, C-D}`
- WHEN bridges runs in `stream` mode
- THEN `(A,B)`, `(B,C)`, `(C,D)` are emitted as bridges
- AND output is sorted lexicographically

#### Scenario: Bridges returns empty on a 2-edge-connected graph

- GIVEN the undirected cycle `{A-B, B-C, C-A}`
- WHEN bridges runs
- THEN the result is empty and non-truncated

### Requirement: K-Core algorithm

The system MUST admit an undirected, k-parametrized k-core algorithm. Its
output schema MUST expose `node_id` and `core_number`. It MUST support
`stream`, `stats`, and `annotate` modes. Output MUST be sorted by `node_id`
and MUST be deterministic. `k=0` MUST return every node with its observed
degree as core number.

#### Scenario: K-core peels low-degree nodes

- GIVEN the undirected projection `{A-B, B-C, C-D, C-E, D-E}` and `k=2`
- WHEN k-core runs in `stream` mode
- THEN only `B`, `C`, `D`, `E` are emitted (with core number ≥ 2)
- AND output is sorted by `node_id`

#### Scenario: K-core with k=0 is exhaustive

- GIVEN the same undirected projection and `k=0`
- WHEN k-core runs
- THEN every node is emitted with its degree as core number

### Requirement: Production composition root

The system MUST expose a `default_analytics_registry()` composition root that
admits the cohort-1 algorithms (PageRank, SCC, WCC, bounded shortest paths)
AND the cohort-2 algorithms (dominators, articulation points, bridges,
k-core). Production builds MUST NOT depend on test-only `.admit()` calls; the
composition root MUST be the single source of the admitted catalog used by
REST, MCP, and Explorer.

#### Scenario: Composition root lists all eight algorithms

- GIVEN a freshly constructed `default_analytics_registry()`
- WHEN its catalog is enumerated
- THEN it lists exactly the four cohort-1 and four cohort-2 algorithms

#### Scenario: Production registry is non-empty at startup

- GIVEN any REST, MCP, or Explorer service handle
- WHEN its registry is inspected at startup
- THEN it is the non-empty composition-root registry, not the empty default

## MODIFIED Requirements

### Requirement: Cohort catalog and projection contract

The registry MUST admit the cohort-1 algorithms (PageRank, strongly connected
components, weakly connected components, bounded shortest paths) and the
cohort-2 algorithms (dominators, articulation points, bridges, k-core).
Every run MUST use its pinned workspace and revision with the descriptor's
projection assumptions. Dominators is directed and root-parametrized;
articulation points, bridges, and k-core operate on the undirected projection.
Incompatible direction, weighting, heterogeneity, or edge orientation MUST
fail before execution.
(Previously: admitted only the four cohort-1 algorithms.)

#### Scenario: Cohort algorithms honor graph semantics

- GIVEN the directed projection `A↔B`, `B→C`
- WHEN PageRank, SCC, WCC, and shortest path `A→C` with `max_hops=2` run
- THEN PageRank emits finite scores for all nodes, SCC emits `{A,B},{C}`, WCC emits `{A,B,C}`, and the path is `[A,B,C]`

#### Scenario: Non-admitted algorithm is rejected

- GIVEN `betweenness` is not admitted in any cohort
- WHEN execution is requested
- THEN the registry returns `NotAdmitted` and no run begins

#### Scenario: Projection mismatch fails closed

- GIVEN a descriptor requiring outgoing `calls` edges and a projection that cannot guarantee their orientation
- WHEN execution is requested
- THEN the registry returns `ProjectionMismatch` and produces no result

#### Scenario: Cohort-2 algorithms are admitted alongside cohort-1

- GIVEN the composition-root registry
- WHEN dominators, articulation points, bridges, and k-core are listed
- THEN all four appear in the catalog with stable identities and conformant descriptors
