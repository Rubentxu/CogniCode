# Executor Equivalence Conformance Specification (OBSOLETE — 2026-08-04)

> **Status: OBSOLETE** — PostgreSQL was removed from the stack (ADR-026, e29-7).
> `PgGraphExecutor` was deleted. This spec compared PG vs snapshot executors,
> a comparison that is no longer applicable. The snapshot executor is now
> the only executor. Archive this spec.

## Purpose

## Purpose

The differential conformance harness that proves the PG and snapshot
executors are interchangeable. For every `(plan, pin, fixture)` triple,
both executors MUST return identical `ResultSet` multisets (when
unordered), identical ordered paths, identical error envelopes, and
identical `TruncationMarker` values. A petgraph-internal oracle MAY be
used as a parity reference but is NOT normative — divergence is a
parity violation, never a normative override (ADR-014 §4).

## Requirements

### Requirement: Golden Fixture Suite

A fixed set of `(fixture, plan, expected_result_set)` triples defines
the conformance baseline. Each fixture is a deterministic graph
(seeded by an integer) and each plan is a `GraphPlan` variant with
explicit `PlanLimits`. The suite covers one positive case per variant
(Path, Neighbors, Subgraph, Cluster, Explain, BooleanComposition).

#### Scenario: Every variant has a positive fixture

- GIVEN the conformance suite
- WHEN the suite is enumerated
- THEN there is at least one positive fixture for each of: Path, Neighbors, Subgraph, Cluster, Explain, BooleanComposition

#### Scenario: Fixtures are deterministic

- GIVEN a fixture seeded by integer `7`
- WHEN the fixture is loaded twice
- THEN the two graphs have identical node and edge sets

### Requirement: Typed Multiset Equivalence

For every `(plan, pin)` where the result is unordered (rows, nodes,
edges, scalars), the conformance harness MUST compare PG and snapshot
results via `assert_equivalent` and the result MUST be `Ok(())`.

#### Scenario: Unordered neighbor sets match

- GIVEN fixture graph A→{B,C}, B→C
- WHEN both executors run `Neighbors(A, Out, 2)`
- THEN `assert_equivalent(&pg_result, &snap_result)` is `Ok(())`
- **(PG-required)**

#### Scenario: Subgraph nodes match

- GIVEN fixture graph A→B→C→D
- WHEN both executors run `Subgraph { nodes: [A], depth: 3 }`
- THEN `assert_equivalent(&pg_result, &snap_result)` is `Ok(())` for node and edge multisets
- **(PG-required)**

### Requirement: Ordered Path Equivalence

For every `Path` plan, the path sequences returned by PG and snapshot
MUST be identical in length and in hop order. Divergence is a
`SemanticsViolation::PathOrderMismatch`.

#### Scenario: Path sequences match in order

- GIVEN fixture graph A→B→C→D with edge A→D direct
- WHEN both executors run `Path { src: A, dst: D, max_hops: 3 }`
- THEN `paths.len()` is identical
- AND `paths[i]` is element-wise equal for all `i`
- **(PG-required)**

#### Scenario: BFS ordering matches SQL ordering

- GIVEN fixture graph A→{B,C}, both B and C reach D
- WHEN both executors run `Path(src: A, dst: D, max_hops: 2)`
- THEN the path `[A, B, D]` and `[A, C, D]` appear in the SAME order in both backends
- **(PG-required)**

### Requirement: Error Envelope Equivalence

For every `(plan, pin)` where the result is `Err`, the PG and
snapshot executors MUST return the same `ExecutorError` variant. The
specific observation value MAY differ (e.g., `LimitExceeded { observed }`),
but the `dimension` MUST match.

#### Scenario: Unknown revision matches

- GIVEN pin `(ws1, 99)` where no revision exists
- WHEN both executors run any plan
- THEN both return `Err(ExecutorError::RevisionUnknown("ws1:99"))`
- **(PG-required for PG-side)**

#### Scenario: Unsupported construct matches

- GIVEN a plan carrying an unsupported construct flag
- WHEN both executors run the plan
- THEN both return `Err(ExecutorError::UnsupportedConstruct { .. })`

### Requirement: Truncation Marker Equivalence

When a soft limit fires, both executors MUST return `Ok` with
identical `TruncationMarker` values. The PG executor's SQL `LIMIT`
boundary and the snapshot executor's in-process counter must agree on
whether truncation occurred and on which dimension.

#### Scenario: max_result_rows truncation matches

- GIVEN a Neighbors plan that produces 50 results and `max_result_rows: Some(10)`
- WHEN both executors run the plan
- THEN both return `Ok(ResultSet { rows.len() == 10, truncated: true, truncation: Some(ResultRowsLimit) })`
- **(PG-required)**

#### Scenario: max_path_count truncation matches

- GIVEN a Path plan with `max_path_count: Some(1)` and 3 possible paths
- WHEN both executors run the plan
- THEN both return `Ok(ResultSet { paths.len() == 1, truncated: true, truncation: Some(PathCountLimit) })`
- **(PG-required)**

### Requirement: Provenance Equivalence

For every edge or node returned by both executors, the
`ProvenanceSource` MUST be identical after canonical normalization
(the PG executor surfaces `Postgres`, the snapshot executor surfaces
`Snapshot`; both normalize to a canonical "backend" label).

#### Scenario: Edge provenance matches after normalization

- GIVEN an edge with `Postgres` source on PG side and `Snapshot` source on snapshot side
- WHEN the harness compares the two results
- THEN the edge is considered equivalent

### Requirement: Petgraph Parity Oracle (Non-Normative)

A pure-petgraph reference executor MAY be used as a parity oracle for
investigating divergence. The oracle is NOT normative — if PG and
snapshot agree but the petgraph oracle disagrees, the PG-snapshot
agreement is the conformance verdict.

#### Scenario: Parity oracle divergence is non-binding

- GIVEN a fixture where PG and snapshot agree but the petgraph oracle disagrees
- WHEN the harness runs the conformance suite
- THEN the conformance verdict is `Pass` (PG-snapshot agreement)
- AND the divergence is logged as a parity note for investigation

### Requirement: Conformance Failure Is Loud

A conformance failure MUST terminate the test with a clear diff: the
`(plan, pin, fixture)` triple, the two `ResultSet`s, and the
`SemanticsViolation` variant. Silent PASS is forbidden; a hidden
conformance regression is worse than a loud failure.

#### Scenario: Multiset mismatch is reported

- GIVEN a fixture where PG returns `{A, B}` and snapshot returns `{A, C}`
- WHEN the harness runs the conformance suite
- THEN the test fails with the triple, both `ResultSet`s, and `Err(MultisetMismatch("nodes mismatch"))`

#### Scenario: Path order mismatch is reported

- GIVEN a fixture where PG returns `[A, B, D]` and snapshot returns `[A, C, D]`
- WHEN the harness runs the conformance suite
- THEN the test fails with the triple, both `Path`s, and `Err(PathOrderMismatch("path 0 differs"))`

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Both executors return `Ok(empty)` | Equivalent; no failure |
| Both return `Err(InternalError)` with different messages | Equivalent at variant level; messages NOT compared |
| `time_ms` breach in only one backend | NOT equivalent; executor policy decides the boundary |
| Plan with `max_hops: 0` | Both return `Ok({ paths: vec![] })`; equivalent |
| Plan exceeds PG depth limit (32) | PG returns `Err`; snapshot returns `Ok`; recorded as policy divergence, not conformance failure |

## Out of Scope

- Algorithm-specific numeric tolerance (E28.4+)
- Neo4j oracle wiring (E28.5)
- Performance benchmarking (separate test suite)
- CI vs. local divergence investigation tooling

## Dependencies

- `GraphExecutor` (graph-executor-port)
- `PgGraphExecutor` (pg-graph-executor)
- `SnapshotGraphExecutor` (snapshot-graph-executor)
- `ResultSet`, `Path`, `ExecutorError`, `TruncationMarker`, `assert_equivalent`, `assert_approx_equal` (executor-semantics)
- `PostgresRepository` (postgres-callgraph-persistence)
- `SnapshotProvider` (graph-snapshot-refresh)
- ADR-014 §4
