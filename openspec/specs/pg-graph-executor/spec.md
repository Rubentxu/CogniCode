# PG Graph Executor Specification (OBSOLETE — 2026-08-04)

> **Status: OBSOLETE** — `PgGraphExecutor` was deleted when PostgreSQL
> was removed from the stack (ADR-026, e29-7). The snapshot executor
> (`LadybugGraphExecutor`) is now the only executor. Archive this spec.

## Purpose

The PostgreSQL-backed `GraphExecutor`. Translates a pinned `GraphPlan`
into recursive-CTE SQL over `PostgresRepository::load_call_graph_ws(ws,
rev)` and returns the same `ResultSet` contract as the snapshot
executor. The PG executor is the canonical backend for production
traversals — every `GraphPlan` variant MUST be implemented here.

## Requirements

### Requirement: Construction

`PgGraphExecutor` is constructed from a `PostgresRepository` (or its
`PgPool`). The instance is `Send + Sync + 'static`, holds the pool by
clone, and is side-effect-free until `execute` is called.

#### Scenario: Construct from pool

- GIVEN a `PgPool` connected to a fresh test database
- WHEN `PgGraphExecutor::new(repo)` is called
- THEN the value compiles as `&dyn GraphExecutor`

#### Scenario: Construction is side-effect-free

- GIVEN a `PgPool`
- WHEN `PgGraphExecutor::new` is called 1000 times
- THEN no SQL query runs and no connection is taken

### Requirement: Pin Fails Closed

`execute(&plan, (ws, rev))` MUST pass the pin to `load_call_graph_ws`,
which returns `Err(RepositoryError::UnknownRevision)` for unknown
pairs. The executor MUST translate this to
`Err(ExecutorError::RevisionUnknown("<ws>:<rev>"))`. No silent fallback
to head.

#### Scenario: Unknown revision is rejected

- GIVEN `ws = "ws1"` with no revisions in `graph_revisions`
- WHEN `execute(&plan, ("ws1", 1))` runs
- THEN the result is `Err(ExecutorError::RevisionUnknown("ws1:1"))`
- **(PG-required)**

#### Scenario: Cross-workspace pin is rejected

- GIVEN `ws1` head=3, `ws2` has no revisions
- WHEN `execute(&plan, ("ws2", 3))` runs
- THEN the result is `Err(ExecutorError::RevisionUnknown("ws2:3"))`
- **(PG-required)**

### Requirement: Path Variant Materializes Paths

`GraphPlan::Path { src, dst, max_hops, .. }` MUST execute via a
recursive CTE that walks edges within `max_hops` (≤ 32 to bound
recursion). The executor returns ordered paths in `ResultSet.paths`;
each `Path` carries `PathHop` with `EdgeKind` per hop.

#### Scenario: Shortest path succeeds

- GIVEN fixture graph A→B→C→D with edge A→D direct
- WHEN `execute(Path { src: A, dst: D, max_hops: 3, .. })` runs
- THEN `ResultSet.paths` is non-empty and every path starts at A, ends at D, hop count ≤ 3
- **(PG-required)**

#### Scenario: Unreachable destination returns empty

- GIVEN fixture graph A→B, no path A→Z
- WHEN `execute(Path { src: A, dst: Z, max_hops: 5, .. })` runs
- THEN the result is `Ok(ResultSet { paths: vec![], .. })`
- **(PG-required)**

### Requirement: Neighbors + Subgraph + Cluster + Explain

The PG executor MUST materialize `Neighbors` as `ResultSet.rows` per
neighbor reachable within `depth`, respecting `NeighborKind`. It MUST
materialize `Subgraph` as node + edge unions within `max_depth`.
`Cluster` MUST group nodes by the `by` key and return one row per
group with a count. `Explain` MUST return `ResultSet.scalars` carrying
the inner plan's `PlanMetadata` and MUST NOT execute the inner
traversal.

#### Scenario: Outgoing neighbors at depth 1

- GIVEN fixture graph A→B, A→C, D→A
- WHEN `execute(Neighbors { src: A, kind: Outgoing, depth: 1, .. })` runs
- THEN `ResultSet.rows` contains B and C, NOT D
- **(PG-required)**

#### Scenario: Subgraph returns visited nodes

- GIVEN fixture graph A→B→C→D
- WHEN `execute(Subgraph { nodes: [A], depth: 2, .. })` runs
- THEN `ResultSet.nodes` contains A, B, C and `ResultSet.edges` contains A→B and B→C
- **(PG-required)**

#### Scenario: Cluster by kind

- GIVEN fixture graph with mixed `kind` properties (function, class)
- WHEN `execute(Cluster { by: ["kind"], .. })` runs
- THEN the result has one row per kind with `count` >= 1
- **(PG-required)**

### Requirement: Boolean Composition Typed Multiset

`GraphPlan::BooleanComposition { op, operands, .. }` MUST evaluate
each operand, then combine via `And` (intersection), `Or` (union),
`Not` (complement within the pin's graph). The result is a
`ResultSet` whose `rows` is the typed multiset.

#### Scenario: AND intersection

- GIVEN fixture graph A→{B,C}, B→C
- WHEN `execute(And(Neighbors(A,Out,1), Neighbors(B,Out,1)))` runs
- THEN `ResultSet.rows` contains exactly C
- **(PG-required)**

#### Scenario: NOT complement

- GIVEN fixture graph A→{B,C}, D isolated
- WHEN `execute(Not(Neighbors(A,Out,1)))` runs
- THEN `ResultSet.rows` contains all nodes EXCEPT B and C (including D)
- **(PG-required)**

### Requirement: Plan Limit Enforcement

The PG executor MUST push `PlanLimits` into SQL where possible
(`LIMIT n`, `max_depth` as recursion depth, `max_hops` as path bound)
and enforce in-process for limits SQL cannot express (time, memory,
cancellation).

#### Scenario: max_result_rows truncated at SQL boundary

- GIVEN a Neighbors query that would produce 50 results
- WHEN `execute` runs with `max_result_rows: Some(10)`
- THEN the result is `Ok(ResultSet { rows.len() == 10, truncated: true, truncation: Some(ResultRowsLimit) })`
- **(PG-required)**

#### Scenario: time_ms breach is an error

- GIVEN a slow plan with `time_ms: Some(1)`
- WHEN `execute` runs
- THEN the result is `Err(LimitExceeded { dimension: TimeMs, observed })`
- **(PG-required)**

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Pin to `RevisionId(0)` | `Err(ExecutorError::RevisionUnknown)`; `NONE` is not a valid pin |
| Self-loop edges | Included in path results; the loop `EdgeKind` appears in the hop |
| `BooleanComposition::Not` empty operand | Whole-graph complement; result is everything except the operand's nodes |
| SQL connection failure | `Err(ExecutorError::InternalError("db unreachable"))` |
| Recursive CTE depth > 32 | `Err(ExecutorError::InternalError("max recursion depth exceeded"))` |

## Out of Scope

- Snapshot-backed executor (snapshot-graph-executor)
- Conformance harness (executor-equivalence-conformance)
- Cypher/GQL syntax differences (E28.3)
- Graph analytics registry (E28.4+)

## Dependencies

- `GraphExecutor` (graph-executor-port)
- `PostgresRepository::load_call_graph_ws` (postgres-callgraph-persistence)
- `GraphPlan`, `PlanLimits`, `ResultSet`, `ExecutorError`, `TruncationMarker` (E28.1)
- `WorkspaceId`, `RevisionId` (graph-revisions)
- ADR-014 §4
