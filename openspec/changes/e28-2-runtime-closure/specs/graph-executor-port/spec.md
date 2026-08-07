# Delta for graph-executor-port

> Change: `e28-2-runtime-closure`. Runtime-injected limits and pinned execution
> are enforced through the backend-neutral executor port.

## ADDED Requirements

### Requirement: Executors Receive Injected PlanLimits

Each executor MUST merge runtime limits with plan limits, selecting the tighter
bound for every dimension. Every traversal and result-materialization boundary
MUST consult the merged limits.

#### Scenario: Injected elapsed-time budget tightens the run

- GIVEN a plan has no `time_ms` and runtime injects `time_ms=1`
- WHEN execution exceeds one millisecond
- THEN it returns `LimitExceeded { Time, observed }`

#### Scenario: Injected response budget overrides the plan

- GIVEN a plan allows 2 MiB and runtime injects `max_response_bytes=512 KiB`
- WHEN the result exceeds 512 KiB
- THEN the response uses the 512 KiB soft limit and reports truncation metadata

### Requirement: Pin-Fails-Closed Enforced at Runtime

The runtime MUST reject an absent workspace or revision pin before executor
dispatch. It MUST NOT fall back to head or return empty success.

#### Scenario: Unpinned plan is rejected

- GIVEN a graph plan without a pin
- WHEN runtime dispatch validates it
- THEN it returns `RevisionUnknown` without calling an executor

#### Scenario: NONE revision is rejected

- GIVEN a plan pinned to `(ws1, RevisionId::NONE)`
- WHEN runtime dispatch validates it
- THEN it returns `RevisionUnknown("ws1:0")`

## MODIFIED Requirements

### Requirement: PlanLimits Honored

The executor MUST enforce every effective limit during traversal or result
materialization. Elapsed time, visited nodes, and visited edges MUST be hard
`LimitExceeded` errors with no partial result. Memory, cancellation, and
structural depth bounds remain hard. Result rows, path count, and response bytes
MUST return a deterministic complete prefix with
`TruncationMetadata { marker, limit, observed, emitted }`. Post-walk detection
is forbidden when the boundary can be checked earlier.
(Previously: visited limits were soft truncations, response-byte limits were
absent, and some enforcement occurred after traversal.)

#### Scenario: Soft result-row limit produces truncation

- GIVEN a plan finds 10 rows with `max_result_rows=3`
- WHEN execution materializes the result
- THEN it returns the first three canonical rows with `ResultRowsLimit` metadata

#### Scenario: Hard elapsed-time limit produces an error

- GIVEN a slow plan with `time_ms=1`
- WHEN elapsed time exceeds the limit
- THEN it returns `LimitExceeded { Time, observed }`

#### Scenario: Visited-node cap aborts mid-walk

- GIVEN `max_visited_nodes=5`
- WHEN traversal attempts node 6
- THEN it returns `LimitExceeded { MaxVisitedNodes, observed: 6 }`
- AND returns no partial result

#### Scenario: Visited-edge cap aborts mid-walk

- GIVEN `max_visited_edges=100`
- WHEN traversal attempts edge 101
- THEN it returns `LimitExceeded { MaxVisitedEdges, observed: 101 }`

#### Scenario: Path count returns a deterministic prefix

- GIVEN three canonical paths and `max_path_count=2`
- WHEN the third path is discovered
- THEN it returns the first two paths with `PathCountLimit` metadata

#### Scenario: Response-byte limit returns a complete prefix

- GIVEN a canonical result exceeds `max_response_bytes`
- WHEN the result is serialized
- THEN it returns the maximal complete prefix within the budget
- AND reports `ResponseBytesLimit` metadata
