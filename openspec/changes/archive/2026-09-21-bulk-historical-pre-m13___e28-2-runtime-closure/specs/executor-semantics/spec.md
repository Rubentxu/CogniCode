# Delta for executor-semantics

> Change: `e28-2-runtime-closure`. Typed errors and deterministic truncation
> replace synthetic or ambiguous outcomes.

## ADDED Requirements

### Requirement: No Synthetic Empty Success

No executor, stub, or test double MAY return an empty `ResultSet` to mean that
a plan was unsupported, unpinned, or unhandled. Such cases MUST return a typed
`ExecutorError`.

#### Scenario: StubExecutor refuses empty success

- GIVEN a `StubExecutor` without a real backend
- WHEN `execute` receives a plan
- THEN it returns `UnsupportedConstruct` and never an empty success

#### Scenario: Unhandled variant returns typed error

- GIVEN a not-yet-implemented plan variant
- WHEN `execute` runs
- THEN it returns `InternalError` or `UnsupportedConstruct`

### Requirement: ResultSet Is the Typed Envelope for REST/MCP

REST and MCP graph endpoints MUST serialize the shared `ResultSet` with typed
values, paths, nodes, edges, scalars, provenance, `truncated`, and optional
`truncation` metadata. Untyped row maps are forbidden.

#### Scenario: REST returns typed ResultSet

- GIVEN a successful query
- WHEN REST serializes the response
- THEN typed values, provenance, and truncation fields are present

#### Scenario: MCP returns the same envelope

- GIVEN the same execution result
- WHEN MCP serializes it
- THEN the envelope is schema-equivalent to REST

## MODIFIED Requirements

### Requirement: Error Envelope

`ExecutionError` MUST distinguish internal, plan, unsupported-construct,
unknown-revision, and `LimitExceeded { limit, observed }` failures. Unsupported
constructs MUST fail before partial execution. Elapsed-time and visited-node or
visited-edge breaches MUST be hard `LimitExceeded` errors; path count and
response bytes MUST NOT use this error when deterministic truncation is
possible. Stubs MUST reject real plans rather than return empty success.
(Previously: the error contract did not classify visited limits as hard and
used path count as a hard-error example.)

#### Scenario: Unsupported construct is raised pre-execution

- GIVEN a plan with an unbounded path
- WHEN execution begins
- THEN it returns `UnsupportedConstruct` before touching rows

#### Scenario: Limit exceeded is typed

- GIVEN `max_visited_nodes=100` and traversal attempts node 101
- WHEN the boundary fires
- THEN it returns `LimitExceeded { MaxVisitedNodes, observed: 101 }`

#### Scenario: StubExecutor surfaces UnsupportedConstruct

- GIVEN a `StubExecutor` and any graph plan
- WHEN `execute` runs
- THEN it returns `UnsupportedConstruct`, never an empty `ResultSet`

### Requirement: Truncation

A soft-limited `ResultSet` MUST carry
`truncated=true` and `TruncationMetadata { marker, limit, observed, emitted }`.
Result-row, path-count, and response-byte truncation MUST select a canonical,
complete prefix so identical inputs produce identical output and metadata.
Elapsed-time and visited-node or visited-edge limits MUST return hard errors.
(Previously: truncation carried only a marker and did not define response-byte
or deterministic-prefix semantics.)

#### Scenario: Result-row truncation is explicit

- GIVEN 200 canonical rows and `max_result_rows=50`
- WHEN the row limit fires
- THEN the first 50 rows and `ResultRowsLimit` metadata are returned

#### Scenario: Path-count truncation is deterministic

- GIVEN three canonical paths and `max_path_count=2`
- WHEN the third path is discovered
- THEN the first two paths and `PathCountLimit` metadata are returned

#### Scenario: Response-byte truncation preserves complete values

- GIVEN a typed result exceeds `max_response_bytes`
- WHEN it is materialized
- THEN the maximal complete canonical prefix within the budget is returned
- AND `ResponseBytesLimit` metadata reports limit, observed, and emitted count

#### Scenario: Non-truncatable limit produces error

- GIVEN an elapsed-time budget is exceeded
- WHEN the boundary fires
- THEN it returns `LimitExceeded { Time, observed }`, not truncation
