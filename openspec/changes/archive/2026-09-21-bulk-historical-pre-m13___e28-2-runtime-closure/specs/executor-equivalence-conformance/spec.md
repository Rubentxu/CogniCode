# Delta for executor-equivalence-conformance

> Change: `e28-2-runtime-closure`. PostgreSQL and snapshot execution are the
> normative differential pair.

## ADDED Requirements

### Requirement: PG-Snapshot Is the Normative Verdict Oracle

The harness MUST treat agreement between `PgGraphExecutor` and
`SnapshotGraphExecutor` as binding. It MUST compare every golden
`(plan, pin, fixture)` triple in CI. A petgraph oracle MAY aid diagnosis but MUST
NOT override PG-to-snapshot agreement.

#### Scenario: CI runs every triple

- GIVEN the golden fixture suite
- WHEN CI runs conformance
- THEN every triple compares both executors' results, errors, and truncation metadata

#### Scenario: Petgraph divergence is non-binding

- GIVEN PostgreSQL and snapshot agree while petgraph differs
- WHEN the harness decides conformance
- THEN the verdict is pass and the petgraph difference is diagnostic only

## MODIFIED Requirements

### Requirement: Truncation Marker Equivalence

When a soft result-row, path-count, or response-byte limit fires, both
executors MUST return `Ok` with identical deterministic result prefixes and
identical `TruncationMetadata { marker, limit, observed, emitted }`. Hard
elapsed-time and visited-node or visited-edge breaches MUST compare as the same
`LimitExceeded` dimension, never as truncation.
(Previously: conformance compared truncation markers for result rows and path
count but did not require full metadata, response-byte parity, or classify
visited limits as hard errors.)

#### Scenario: max_result_rows truncation matches

- GIVEN a query produces 50 rows and `max_result_rows=10`
- WHEN both executors run it
- THEN both return the same first 10 canonical rows
- AND both report matching `ResultRowsLimit` metadata

#### Scenario: max_path_count truncation matches

- GIVEN three canonical paths and `max_path_count=1`
- WHEN both executors run the path plan
- THEN both return the same first path and matching `PathCountLimit` metadata

#### Scenario: max_response_bytes truncation matches

- GIVEN a canonical result exceeds `max_response_bytes`
- WHEN both executors serialize the typed result
- THEN both return the same maximal complete prefix within the budget
- AND both report matching `ResponseBytesLimit` metadata

#### Scenario: Visited-node breach matches as an error

- GIVEN both runs attempt node 101 with `max_visited_nodes=100`
- WHEN the boundary fires
- THEN both return `LimitExceeded { MaxVisitedNodes, observed: 101 }`
- AND neither returns a partial result or truncation metadata

### Requirement: Conformance Failure Is Loud

A conformance failure MUST terminate CI with the `(plan, pin, fixture)` triple,
both outcomes, and the `SemanticsViolation`. Divergence MUST block merge.
(Previously: failures were loud but were not explicitly normative merge gates.)

#### Scenario: Multiset mismatch is reported

- GIVEN PostgreSQL returns `{A,B}` and snapshot returns `{A,C}`
- WHEN conformance runs
- THEN CI fails with both results and `MultisetMismatch`

#### Scenario: Path order mismatch is reported

- GIVEN the backends return different first paths
- WHEN conformance runs
- THEN CI fails with both paths and `PathOrderMismatch`

#### Scenario: Conformance gate blocks merge

- GIVEN any unapproved executor divergence
- WHEN branch CI completes
- THEN it exits non-zero and blocks merge
