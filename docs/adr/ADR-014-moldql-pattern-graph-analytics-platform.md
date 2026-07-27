# ADR-014: MoldQL Pattern Profile and graph analytics platform

**Status**: PROPOSED  
**Date**: 2026-07-27  
**Deciders**: User, OpenCode orchestrator  

## Context

CogniCode already contains a promising graph-query and analytics substrate, but
its capabilities are fragmented.

MoldQL defines object selection and graph primitives. The graph primitives can
compile to PostgreSQL or Petgraph representations, but their execution is
stubbed or semantically divergent. The production MoldQL service does not wire
the graph navigation port. Existing tests mostly verify plan shape rather than
equal results.

`cognicode-graph-algos` and `CallGraphProjection` provide useful algorithms,
but the catalog has no admission contract, run lineage or common result modes.
Some implementations duplicate Petgraph, and projection defects can invalidate
new analytics.

Research into Cypher, ISO GQL and Neo4j GDS confirms that CogniCode benefits
from graph-pattern expressiveness and selected analytics. Adopting Neo4j as a
required second database would conflict with PostgreSQL canonicality and create
replication, licensing and operational costs.

## Decision

CogniCode will evolve MoldQL into its single product query language and add a
selective graph analytics platform. PostgreSQL remains the sole canonical graph
store.

### 1. Canonical state and execution scope

Every graph query and analytics run must be scoped to one workspace and one
immutable graph revision. In-memory graphs and external engines are derived
projections and cannot accept authoritative writes.

Identity, workspace isolation, typed property round-trip and snapshot refresh
tests are prerequisites for Pattern Profile delivery.

### 2. MoldQL remains the product language

MoldQL gains a read-only **Pattern Profile** inspired by GQL and openCypher.
CogniCode will not claim Cypher, openCypher or GQL compatibility without a
published conformance score and explicit supported-feature matrix.

Pattern Profile v1 includes:

- typed node and relationship patterns;
- direction;
- bounded path quantifiers;
- property, provenance and confidence predicates;
- typed row, node, edge and path results;
- aggregation, ordering and limits;
- bounded shortest paths.

Mutations and unbounded variable-length paths are excluded from v1.

### 3. `MoldPlan` and `GraphPlan` are separate contracts

Not all MoldQL operations are graph operations. The normalized plan algebra is:

```text
MoldPlan
  ├── Graph(GraphPlan)
  ├── ObjectSelection
  ├── Quality
  ├── Lens
  └── ViewExecution
```

Only graph-selecting syntax lowers to `GraphPlan`. The plan is versioned,
backend-neutral and contains no SQL, Petgraph, MCP or React types.

`cognicode-core` owns plan and result value objects, executor ports, execution
policy and analytics run lineage. `cognicode-graph-algos` owns pure algorithm
implementations. `cognicode-explorer` owns syntax adaptation, ViewSpec
integration and presentation. `cognicode-runtime` selects executors.

### 4. Executor semantics are normative

PostgreSQL and snapshot executors must implement the same semantics for:

- typed values and missing properties;
- multiset identity and ordering;
- path node and edge sequence;
- errors;
- truncation;
- provenance;
- approximate numeric tolerance.

Unsupported operations fail before execution. An executor must never return an
empty success for unsupported behavior. Backend choice is internal and is only
exposed in diagnostics and conformance testing.

### 5. Query and ViewSpec evaluation are read-only

MoldQL and ViewSpec evaluation cannot mutate canonical graph facts.

Analytics modes are:

| Mode | Meaning |
|---|---|
| `stream` | Return derived values |
| `stats` | Return execution summaries |
| `annotate` | Create an ephemeral view overlay |
| `persist` | Invoke an authorized command that stores derived analysis |

Each algorithm descriptor declares its supported modes. `persist` stores a
separate, idempotent derived-analysis record with lineage. It does not update
canonical `GraphNode` properties or create canonical edges. Derived relations
use the existing `RelationCandidate` promotion workflow.

### 6. Algorithms enter through an admission contract

An admitted algorithm must declare:

- identity and version;
- maturity;
- deterministic behavior and seed rules;
- directed, weighted and heterogeneous graph support;
- input projection assumptions;
- parameters and output schema;
- supported modes;
- complexity and resource limits;
- truncation behavior;
- conformance fixtures.

The delivery cohorts are:

1. Stabilize PageRank, SCC, WCC and bounded shortest paths.
2. Add dominators, articulation points, bridges and k-core.
3. Add betweenness, k-shortest paths, multi-source reachability and
   personalized PageRank.
4. Add Leiden, conductance, modularity and node similarity.

Product concepts such as god nodes and surprising connections should compose
primitive results rather than duplicate commodity algorithms.

### 7. Execution governance is mandatory

Every plan and algorithm run declares limits for time, cancellation, depth,
visited nodes, visited edges, result rows, path count and memory where
applicable. A limit breach produces a typed error or an explicitly truncated
result. It cannot silently degrade.

Every run records workspace, graph revision, normalized plan version and hash,
algorithm and implementation versions, parameters, deterministic seed,
timestamps, status, warnings and truncation.

### 8. Neo4j is an optional CI oracle

Neo4j GDS may validate overlapping query semantics and algorithm outputs in CI.
It is not a production dependency, query router or source of truth in E28.

A production Neo4j sidecar requires a separate ADR triggered by measured
latency, scale or query-expression thresholds.

### 9. UI-visible completion follows ADR-012

Internal foundation slices may ship without direct UI. Every user-facing clause
or admitted user-facing algorithm must define:

- a discoverable Explorer entry point;
- an inspectable result pane;
- REST and MCP access where applicable;
- interaction tests for happy, empty, error and truncation states.

WASM remains an optional optimization, not a completion requirement.

## Alternatives considered

### Adopt Neo4j and Cypher as mandatory infrastructure

Rejected. It provides the fastest feature breadth, but adds a second canonical
candidate, synchronization, licensing and operational coupling.

### Implement complete openCypher compatibility

Deferred. The language semantics and TCK surface are too large for the current
execution foundation. A read-only compatibility profile may be proposed later
with measured conformance.

### Continue extending the existing target-specific compiler

Rejected. It would preserve duplicated semantics and make executor divergence
harder to detect.

### Copy the Neo4j GDS catalog

Rejected. Algorithm count is not product value. CogniCode admits only
capabilities tied to Explorer questions, compositions and views.

### Expose PostgreSQL, Petgraph and Neo4j selection to users

Rejected. Backend selection is infrastructure policy and must not leak into
saved queries or product semantics.

## Consequences

### Positive

- MoldQL can grow without coupling syntax to one backend.
- Query results become reproducible and comparable.
- Analytics stay selective, explainable and resource-governed.
- PostgreSQL canonicality remains intact.
- Explorer receives capabilities tied to visible user questions.
- Neo4j can add validation value without becoming lock-in.

### Negative

- Requires foundational work before visible syntax expansion.
- Typed values, revisions and differential conformance add significant scope.
- Two executors multiply the verification burden.
- Persisted analytics require lifecycle and invalidation policy.

### Mitigations

- Execute E28 in dependency order.
- Freeze new grammar until current primitives execute correctly.
- Begin with fixed golden graphs and strict resource limits.
- Keep the algorithm registry small and cohort-gated.
- Require separate decisions for production sidecars or graph-store migration.

## Out of scope

- Replacing PostgreSQL.
- Production Neo4j replication.
- Full Cypher or ISO GQL compatibility.
- Graph mutation from MoldQL.
- New UML, C4, use-case or state-machine models.
- New `GraphTopology` or `FlowTrace` domain contracts.
- Browser WASM as the canonical executor.

## References

- [Graph stack assessment](../analysis/cognicode-graph-stack-assessment.md)
- [Cypher and GDS fit assessment](../analysis/cognicode-cypher-gds-fit-assessment.md)
- [Graph query execution specification](../specs/graph-query-execution.md)
- [Graph analytics execution specification](../specs/graph-analytics-execution.md)
- [E28 roadmap](../ROADMAP.md#graph-query--analytics-platform-e28)
- [ADR-002](./ADR-002-moldable-exploration-parity-program.md)
- [ADR-006](./ADR-006-functional-gtoolkit-parity.md)
- [ADR-007](./ADR-007-node-properties-graph-query-port.md)
- [ADR-010](./ADR-010-diagram-artifacts-as-persistent-views.md)
- [ADR-012](./ADR-012-ui-visible-capability-contract.md)
- [ADR-013](./ADR-013-progressive-moldable-workbench-shell.md)
- [Neo4j GDS algorithms](https://neo4j.com/docs/graph-data-science/current/algorithms/)
- [openCypher TCK](https://github.com/opencypher/openCypher/tree/main/tck)
