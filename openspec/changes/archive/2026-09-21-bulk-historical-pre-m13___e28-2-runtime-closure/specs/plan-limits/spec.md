# Delta for plan-limits

> Change: `e28-2-runtime-closure`. Limits are enforced in-walk with one
> coherent hard-error versus deterministic-truncation policy.

## ADDED Requirements

### Requirement: Production execution resolves finite effective limits

The runtime MUST resolve `PlanLimits` against a versioned operation-class policy
before production dispatch. Every operation MUST have finite elapsed-time,
visited-node, visited-edge, response-byte, and memory bounds. Path operations
MUST additionally have finite hops and path-count bounds; row-producing
operations MUST have a finite result-row bound. A cancellation token MUST always
be present. If any applicable effective bound remains absent, dispatch MUST
return `MissingLimit(dimension)` before executor invocation. `PlanLimits::default()`
MAY remain an all-`None` construction value for tests and builders, but MUST NOT
be the final production policy.

#### Scenario: Default plan receives runtime policy

- GIVEN `PlanLimits::default()` and a production path operation
- WHEN effective limits are resolved
- THEN finite time, visited-node, visited-edge, response-byte, memory, hop, and path-count bounds are present
- AND execution receives a cancellation token

#### Scenario: Missing required policy fails before dispatch

- GIVEN a production operation whose effective memory bound remains absent
- WHEN dispatch is attempted
- THEN it returns `MissingLimit(Memory)`
- AND no executor is invoked

### Requirement: Per-Hop Counters Enforce Limits During Traversal

Executors MUST check elapsed time and visited-node and visited-edge counters
before the next traversal step. Breaches MUST abort with a hard typed error.
Path-count limits MUST stop at a canonical path boundary and return a soft,
deterministic prefix. Response-byte limits MUST stop at a complete serialized
result item.

#### Scenario: Visited-node cap is hard

- GIVEN `max_visited_nodes=5`
- WHEN traversal attempts node 6
- THEN it returns `LimitExceeded { MaxVisitedNodes, observed: 6 }`

#### Scenario: Visited-edge cap is hard

- GIVEN `max_visited_edges=100`
- WHEN traversal attempts edge 101
- THEN it returns `LimitExceeded { MaxVisitedEdges, observed: 101 }`

#### Scenario: Path count truncates at canonical order

- GIVEN three canonical paths and `max_path_count=2`
- WHEN the third path is discovered
- THEN the first two paths are returned with `PathCountLimit` metadata

## MODIFIED Requirements

### Requirement: PlanLimits Value Object

`PlanLimits` MUST carry optional bounds for elapsed time, cancellation, depth,
visited nodes, visited edges, result rows, path count, response bytes, and
memory. `None` MUST mean no declared bound. It MUST remain serializable and
comparable.

```text
PlanLimits {
  time_ms, cancellation, max_depth,
  max_visited_nodes, max_visited_edges,
  max_result_rows, max_path_count,
  max_response_bytes, max_memory_bytes
}
```

(Previously: the value object had no `max_response_bytes` field.)

#### Scenario: Default limits are absent

- GIVEN `PlanLimits::default()`
- WHEN its fields are observed
- THEN every field, including `max_response_bytes`, is `None`

#### Scenario: Custom limits round-trip

- GIVEN path-count and response-byte bounds
- WHEN limits serialize and deserialize
- THEN the restored value equals the original

### Requirement: Breach Produces Typed Error or Explicit Truncation

Elapsed time, visited nodes, visited edges, memory, cancellation, and structural
bounds MUST produce `LimitExceeded` with no partial result. Result rows, path
count, and response bytes MUST produce `truncated=true` plus
`TruncationMetadata { marker, limit, observed, emitted }`. Soft truncation MUST
use canonical ordering and complete items so output is deterministic.
(Previously: visited nodes were soft, response bytes were absent, and soft
results carried less metadata.)

#### Scenario: Elapsed-time breach is an error

- GIVEN `time_ms=10`
- WHEN elapsed time exceeds 10 ms
- THEN it returns `LimitExceeded { Time, observed }`, never truncation

#### Scenario: Result-row breach is explicit truncation

- GIVEN 200 canonical rows and `max_result_rows=50`
- WHEN the boundary fires
- THEN the first 50 rows and `ResultRowsLimit` metadata are returned

#### Scenario: Response-byte breach is explicit truncation

- GIVEN a canonical result exceeds `max_response_bytes`
- WHEN it is materialized
- THEN the maximal complete prefix within the budget is returned
- AND `ResponseBytesLimit` metadata reports limit, observed, and emitted count

#### Scenario: Memory-limit breach is an error

- GIVEN `max_memory_bytes=1_048_576`
- WHEN the executor estimate exceeds 1 MiB
- THEN it returns `LimitExceeded { Memory, observed }`

### Requirement: PlanLimit Enum

`PlanLimit` MUST distinguish `Time`, `Cancellation`, `MaxDepth`, `MaxHops`,
`MaxVisitedNodes`, `MaxVisitedEdges`, `MaxResultRows`, `MaxPathCount`,
`MaxResponseBytes`, and `Memory`.
(Previously: `MaxResponseBytes` was absent.)

#### Scenario: Every limit dimension is representable

- GIVEN all declared plan and graph bounds
- WHEN each is mapped to `PlanLimit`
- THEN each has one unambiguous variant

#### Scenario: LimitExceeded identifies a hard dimension

- GIVEN `LimitExceeded { MaxVisitedEdges, observed: 101 }`
- WHEN a caller inspects it
- THEN no string parsing is required to identify the limit

### Requirement: TruncationMarker Is Distinct from Error

`TruncationMarker` MUST contain `ResultRowsLimit`, `PathCountLimit`, and
`ResponseBytesLimit`. `VisitedNodesLimit` and `VisitedEdgesLimit` MUST NOT be
truncation markers. A truncated result MUST include metadata with marker, limit,
observed, and emitted values.
(Previously: visited-node and visited-edge markers existed and response-byte
truncation did not.)

#### Scenario: Truncation and hard error are distinguishable

- GIVEN a `PathCountLimit` result and a `MaxVisitedNodes` error
- WHEN a caller matches each outcome
- THEN the first takes the success path and the second takes the error path

#### Scenario: Response truncation metadata is explicit

- GIVEN `max_response_bytes=1024` and a larger result
- WHEN a complete prefix is returned
- THEN metadata uses `ResponseBytesLimit` and reports the byte bound
