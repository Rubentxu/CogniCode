# Snapshot Graph Executor Specification (NEW)

## Purpose

The in-memory `GraphExecutor` backed by `SnapshotProvider::snapshot(ws,
rev)`. Walks the petgraph held by the snapshot and materializes the
same `ResultSet` contract as the PG executor. Both executors MUST
produce identical typed multisets, ordered paths, errors, and
truncation for the same pinned plan — see
`executor-equivalence-conformance`.

## Requirements

### Requirement: Construction

`SnapshotGraphExecutor` is constructed from a `&dyn SnapshotProvider`.
The instance is `Send + Sync + 'static` and holds no mutable state
of its own. Construction is side-effect-free.

#### Scenario: Construct from provider

- GIVEN a `SnapshotProviderImpl` over a fresh test pool
- WHEN `SnapshotGraphExecutor::new(provider)` is called
- THEN the value compiles as `&dyn GraphExecutor`

#### Scenario: Construction is side-effect-free

- GIVEN a `SnapshotProvider`
- WHEN `SnapshotGraphExecutor::new` is called 1000 times
- THEN no snapshot read happens and no graph is materialized

### Requirement: Pin Fails Closed

`execute(&plan, (ws, rev))` MUST call `snapshot(ws, rev)` and translate
`SnapshotError::UnknownRevision { ws, rev }` to
`Err(ExecutorError::RevisionUnknown("<ws>:<rev>"))`. No silent
fallback to head.

#### Scenario: Unknown revision is rejected

- GIVEN `ws = "ws1"` with no revisions in `graph_revisions`
- WHEN `execute(&plan, ("ws1", 1))` runs
- THEN the result is `Err(ExecutorError::RevisionUnknown("ws1:1"))`

#### Scenario: Cache hit returns the cached snapshot

- GIVEN `ws = "ws1"` head=3, snapshot already cached
- WHEN `execute(&plan, ("ws1", 3))` runs
- THEN the result is `Ok(ResultSet)` and the cached snapshot is reused

### Requirement: Path Variant Uses BFS

`GraphPlan::Path { src, dst, max_hops, .. }` MUST execute via BFS on
the snapshot's petgraph, bounded by `max_hops`. Paths are returned in
BFS discovery order (shortest first, then by predecessor order).

#### Scenario: Shortest path returns BFS result

- GIVEN fixture graph A→B→C→D with edge A→D direct
- WHEN `execute(Path { src: A, dst: D, max_hops: 3, .. })` runs
- THEN `ResultSet.paths` is non-empty and the shortest path appears before longer paths

#### Scenario: Hop bound respected

- GIVEN fixture graph A→B→C→D with path length 3
- WHEN `execute(Path { src: A, dst: D, max_hops: 2, .. })` runs
- THEN `ResultSet.paths` is empty (3 hops > 2 limit)

### Requirement: Neighbors + Subgraph + Cluster + Explain

The snapshot executor MUST materialize `Neighbors` as unordered
`ResultSet.rows` per reachable neighbor within `depth`, respecting
`NeighborKind`. It MUST materialize `Subgraph` as node + edge unions
within `max_depth` via BFS. `Cluster` MUST group nodes by the `by`
key using a `HashMap<String, usize>` and return one row per group
with a count. `Explain` MUST return `ResultSet.scalars` carrying the
inner plan's `PlanMetadata` and MUST NOT execute the inner traversal.

#### Scenario: Outgoing vs Incoming neighbors

- GIVEN fixture graph A→B, A→C, D→A
- WHEN `execute(Neighbors { src: A, kind: Outgoing, depth: 1, .. })` runs
- THEN `ResultSet.rows` contains B and C, NOT D
- AND when `kind: Incoming` runs, `ResultSet.rows` contains D, NOT B or C

#### Scenario: Subgraph returns visited nodes

- GIVEN fixture graph A→B→C→D
- WHEN `execute(Subgraph { nodes: [A], depth: 2, .. })` runs
- THEN `ResultSet.nodes` contains A, B, C and `ResultSet.edges` contains A→B and B→C

#### Scenario: Cluster by kind

- GIVEN fixture graph with mixed `kind` properties (function, class)
- WHEN `execute(Cluster { by: ["kind"], .. })` runs
- THEN the result has one row per kind with `count` >= 1

### Requirement: Boolean Composition Typed Multiset

`GraphPlan::BooleanComposition { op, operands, .. }` MUST evaluate
each operand independently on the snapshot, then combine via `And`
(intersection), `Or` (union), `Not` (complement within the
snapshot's node universe).

#### Scenario: AND intersection

- GIVEN fixture graph A→{B,C}, B→C
- WHEN `execute(And(Neighbors(A,Out,1), Neighbors(B,Out,1)))` runs
- THEN `ResultSet.rows` contains exactly C

#### Scenario: NOT complement

- GIVEN fixture graph A→{B,C}, D isolated
- WHEN `execute(Not(Neighbors(A,Out,1)))` runs
- THEN `ResultSet.rows` contains all nodes EXCEPT B and C (including D)

### Requirement: Plan Limit Enforcement

The snapshot executor enforces limits in-process: `max_depth`,
`max_hops`, `max_visited_nodes`, `max_visited_edges` are checked
during BFS; `max_result_rows` and `max_path_count` are checked on
result materialization; `time_ms` and `cancellation` are polled at
BFS-frame boundaries.

#### Scenario: max_result_rows truncated

- GIVEN a Neighbors query that would produce 50 results
- WHEN `execute` runs with `max_result_rows: Some(10)`
- THEN the result is `Ok(ResultSet { rows.len() == 10, truncated: true, truncation: Some(ResultRowsLimit) })`

#### Scenario: cancellation aborts

- GIVEN a long-running plan with `cancellation: Some(token)` set externally
- WHEN the executor polls the token mid-BFS
- THEN the result is `Err(LimitExceeded { dimension: Cancellation, observed: 0 })`

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Pin to `RevisionId(0)` | `Err(ExecutorError::RevisionUnknown)`; `NONE` is not a valid pin |
| Self-loop edges | Included in path results; the loop `EdgeKind` appears in the hop |
| `BooleanComposition::Not` empty operand | Whole-graph complement; result is everything except the operand's nodes |
| Snapshot cache eviction | `snapshot(ws, rev)` re-materializes; concurrent reads see the same `Arc<CallGraph>` |
| Disconnected component | BFS visits only the connected component of the source |

## Out of Scope

- PG-backed executor (pg-graph-executor)
- Conformance harness (executor-equivalence-conformance)
- Persistent cache invalidation beyond `SnapshotProvider`'s contract
- Graph analytics registry (E28.4+)

## Dependencies

- `GraphExecutor` (graph-executor-port)
- `SnapshotProvider::snapshot(ws, rev)` (graph-snapshot-refresh)
- `GraphPlan`, `PlanLimits`, `ResultSet`, `ExecutorError`, `TruncationMarker` (E28.1)
- `WorkspaceId`, `RevisionId` (graph-revisions)
- ADR-014 §4
