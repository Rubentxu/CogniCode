# Graph Runtime Composition Specification

## Purpose

The composition root provides one normal production path from MoldQL to a
typed `ResultSet`: `compile_to_plan` followed by a pinned `GraphExecutor`.

## ADDED Requirements

### Requirement: Composition Root Selects and Injects the Backend

The runtime MUST construct a `GraphExecutorRegistry` with
`PgGraphExecutor` as canonical and `SnapshotGraphExecutor` as the differential
oracle. It MUST inject the registry and `GraphQueryPort` into REST and MCP
handlers. Callers MUST observe `dyn GraphExecutor`, never a concrete backend.

#### Scenario: Registry dispatches the canonical executor

- GIVEN the runtime composition root is initialized
- WHEN a graph plan is dispatched
- THEN the registry invokes `PgGraphExecutor` through `dyn GraphExecutor`
- AND the snapshot executor remains available to conformance checks

### Requirement: Plan Execution Is the Sole Normal Production Route

Normal production queries MUST use `compile_to_plan` and `GraphExecutor`.
Legacy `compile()` and `CompileTarget` MAY be activated only as an explicit,
temporary rollback during an incident; rollback mode MUST be disabled by
default and observable. No default-off flag MAY make the legacy route normal.

#### Scenario: Normal startup selects the plan path

- GIVEN the runtime starts without an incident rollback override
- WHEN REST or MCP executes a graph query
- THEN it uses `compile_to_plan` and the canonical executor
- AND no legacy compile route is reachable from the request

#### Scenario: Temporary rollback is explicit

- GIVEN an operator explicitly activates rollback mode for an incident
- WHEN a graph query executes during that bounded rollback window
- THEN the legacy adapter MAY serve the request and emits rollback metadata
- AND removing the override restores the sole normal plan path

### Requirement: Limit Outcomes Are Coherent

Executors MUST enforce elapsed time and visited-node or visited-edge limits
during traversal as hard `LimitExceeded` errors with no partial result. Path
count and response-byte limits MUST return a deterministic complete prefix with
explicit truncation metadata containing the dimension, limit, observed value,
and emitted count. Identical inputs MUST yield identical prefixes and metadata.

#### Scenario: Visited-node limit is a hard error

- GIVEN `max_visited_nodes=5`
- WHEN traversal attempts to visit node 6
- THEN execution returns `Err(LimitExceeded { MaxVisitedNodes, observed: 6 })`

#### Scenario: Path count truncates deterministically

- GIVEN three canonically ordered paths and `max_path_count=2`
- WHEN execution discovers the third path
- THEN it returns the first two paths with `PathCountLimit` metadata

#### Scenario: Response bytes truncate at a complete item

- GIVEN a canonically ordered result exceeds `max_response_bytes`
- WHEN the response is materialized
- THEN it returns the maximal complete prefix within the budget
- AND `ResponseBytesLimit` metadata reports limit, observed, and emitted count

### Requirement: Canonical Backend-Neutral PlanHash

`PlanHash` MUST be the SHA-256 of normalized plan serialization and MUST ignore
backend identity.

#### Scenario: Equivalent plans hash identically

- GIVEN logically identical plans for PostgreSQL and snapshot execution
- WHEN each plan is hashed
- THEN both hashes are equal

### Requirement: Typed ResultSet Over REST and MCP

REST and MCP graph endpoints MUST serialize the shared typed `ResultSet`,
including results, truncation metadata, and provenance. Untyped row maps are
forbidden.

#### Scenario: Both transports return the same envelope

- GIVEN a successful `Neighbors` execution
- WHEN REST and MCP serialize the result
- THEN both envelopes have equivalent typed values, provenance, and truncation metadata

### Requirement: PG-Snapshot Conformance Is Normative

Every golden `(plan, pin, fixture)` triple MUST compare PostgreSQL and snapshot
results, hard errors, and soft truncation metadata. Divergence MUST fail CI.

#### Scenario: Conformance divergence blocks CI

- GIVEN the backends return different results for one golden triple
- WHEN the conformance suite runs
- THEN CI exits non-zero with the triple and semantic difference

### Requirement: No Synthetic Empty Success

Unsupported or unhandled plans MUST return a typed error. No executor, stub, or
dispatch guard MAY represent unsupported behavior as an empty success.

#### Scenario: Unsupported plan fails before execution

- GIVEN a plan carrying an unsupported construct
- WHEN the runtime dispatch guard evaluates it
- THEN it returns `UnsupportedConstruct` and does not call an executor
