# Delta for generic-graph-projection

> New capability introduced by this change; no existing main spec. Scope: the minimal projection port that derives a generic graph from facts. Umbrella `projection-architecture` covers CallGraph projections; this capability covers the GenericGraph seam (UAT-U11). `GraphNode`/`GraphEdge` type contracts live in main spec `generic-graph-model`.

## ADDED Requirements

### Requirement: Fact-derived generic graph emission

The system MUST expose a projection port that builds a generic graph from the facts of one pinned snapshot, emitting existing `GraphNode`/`GraphEdge` values. Emitted nodes and edges MUST correspond to fact entity values and canonical relations; rebuilding after clearing the projection, from the same pinned facts, MUST yield an equivalent projection (umbrella rebuildability specialized to this port).

#### Scenario: Facts map to nodes and edges

- GIVEN a snapshot whose committed facts use canonical relations
- WHEN the generic projection is built from those facts
- THEN emitted nodes correspond to fact entity values AND emitted edges correspond to fact relations with matching endpoints

#### Scenario: Empty snapshot yields empty projection

- GIVEN a pinned snapshot with no committed facts
- WHEN the projection is built
- THEN it emits no nodes and no edges without error

#### Scenario: Clear and rebuild is equivalent

- GIVEN a projection built from a pinned snapshot
- WHEN the projection storage is cleared and rebuilt from the same pinned facts
- THEN the rebuilt projection is equivalent to the prior one

### Requirement: Consumers remain FactStore-ignorant

The port output MUST consist solely of existing `GraphNode`/`GraphEdge` values, so consumers (e.g., Explorer graph views) can adopt the projection without any FactStore dependency.

#### Scenario: Consumer needs only emitted types

- GIVEN a consumer of the generic projection
- WHEN it consumes the emitted nodes and edges
- THEN it requires no FactStore access AND no types beyond the existing graph model
