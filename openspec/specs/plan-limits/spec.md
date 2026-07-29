# plan-limits Specification (NEW)

## Purpose

Resource governance contract. Every `MoldPlan` and every executor run
declares the applicable limits for time, cancellation, depth, visited
nodes/edges, result rows, path count, and memory. A breach yields a
typed `ExecutionError::LimitExceeded` or an explicit
`ResultSet.truncated` marker — never silent degradation (ADR-014 §7).

## Requirements

### Requirement: PlanLimits Value Object

`PlanLimits` is a Rust struct with optional `Default` fields for each
limit dimension. A `None` value means "no limit declared for this
dimension". The struct MUST derive `Debug, Clone, PartialEq, Serialize, Deserialize`.

```text
PlanLimits {
  time_ms: Option<u64>,
  cancellation: Option<CancellationToken>,
  max_depth: Option<u32>,
  max_visited_nodes: Option<u64>,
  max_visited_edges: Option<u64>,
  max_result_rows: Option<u64>,
  max_path_count: Option<u64>,
  max_memory_bytes: Option<u64>,
}
```

#### Scenario: Default limits are all None

- GIVEN `PlanLimits::default()`
- WHEN each field is observed
- THEN every field is `None` (no implicit cap)

#### Scenario: Custom limits round-trip

- GIVEN `PlanLimits { max_depth: Some(5), max_result_rows: Some(100), .. }`
- WHEN serialized to JSON and deserialized
- THEN the restored struct equals the original (the `None` fields remain `None`)

### Requirement: Every Plan Declares Applicable Limits

A `MoldPlan` MUST carry a `PlanLimits` instance. The compiler
MUST populate at least one limit for plans that can fan out
traversals (e.g., `GraphPlan::Subgraph` MUST set `max_depth`).

#### Scenario: Subgraph requires depth

- GIVEN `GraphPlan::Subgraph { root, depth: 0, … }`
- WHEN the compiler validates the plan
- THEN validation returns `Err(PlanError::MissingLimit(MaxDepth))` when `max_depth` is `None`

#### Scenario: ShortestPath requires a hop bound

- GIVEN `GraphPlan::ShortestPath { from, to, max_hops: None, … }`
- WHEN the compiler validates the plan
- THEN validation returns `Err(PlanError::MissingLimit(MaxHops))`

### Requirement: Breach Produces Typed Error or Explicit Truncation

When the executor exceeds a limit, the boundary is either:
- `Err(ExecutionError::LimitExceeded { limit, observed })` for limits
  that may not be partial (time, memory, cancellation), OR
- `Ok(ResultSet { truncated: TruncationMarker::*, … })` for limits
  that may be partial (result rows, path count, visited nodes).

#### Scenario: Time-limit breach is an error

- GIVEN a plan with `time_ms = 10` and a slow executor
- WHEN the boundary fires
- THEN the result is `Err(LimitExceeded { limit: Time, observed })` — never a truncated success

#### Scenario: Result-row-limit breach is explicit truncation

- GIVEN a plan with `max_result_rows = 50` and execution finds 200 rows
- WHEN the boundary fires
- THEN the result is `Ok(ResultSet { rows: 50, truncated: ResultRowsLimit, … })`

#### Scenario: Memory-limit breach is an error

- GIVEN a plan with `max_memory_bytes = 1_048_576`
- WHEN the executor estimate exceeds 1 MiB
- THEN the result is `Err(LimitExceeded { limit: Memory, observed })`

### Requirement: Cancellation Token

`PlanLimits.cancellation` carries a `CancellationToken`. The executor
MUST observe the token and abort when set. Cancellation produces the
typed `LimitExceeded { limit: Cancellation, observed: 0 }` envelope.

#### Scenario: Cancellation aborts the run

- GIVEN a long-running plan with a cancellation token
- WHEN the token is set by an external signal
- THEN the executor returns `Err(LimitExceeded { limit: Cancellation, observed: 0 })`
- AND no further rows are produced

### Requirement: PlanLimit Enum

`PlanLimit` is an enum with `Time`, `Cancellation`, `MaxDepth`,
`MaxHops`, `MaxVisitedNodes`, `MaxVisitedEdges`, `MaxResultRows`,
`MaxPathCount`, `Memory`. The enum maps 1-to-1 to the `PlanLimits`
fields above.

#### Scenario: Every PlanLimit variant is representable

- GIVEN the `PlanLimit` enum
- WHEN each variant is matched
- THEN there is exactly one variant per `PlanLimits` field

#### Scenario: LimitExceeded identifies the violated dimension

- GIVEN `Err(LimitExceeded { limit: MaxPathCount, observed: 1000 })`
- WHEN the orchestrator inspects the error
- THEN the violated dimension is recoverable via `limit` without parsing strings

### Requirement: TruncationMarker Is Distinct from Error

`TruncationMarker` is an enum with `ResultRowsLimit`, `PathCountLimit`,
`VisitedNodesLimit`, `VisitedEdgesLimit`. A `ResultSet` that carries
`truncated = TruncationMarker::ResultRowsLimit` is a normal success;
orchestrators MUST distinguish it from `Err(LimitExceeded { .. })`.

#### Scenario: Truncation vs error are distinguishable

- GIVEN a `Ok(ResultSet { truncated: Some(ResultRowsLimit), … })` and an `Err(LimitExceeded { limit: MaxPathCount, .. })`
- WHEN an orchestrator matches on the result
- THEN the success path is taken for the truncated result, and the error path for the explicit failure

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| `PlanLimits { ..default() }` | Permitted; runtime limits are caller's responsibility |
| `max_path_count = 0` | Empty result set; not an error |
| Two limits breached simultaneously | First wins; the executor MUST report the first detected breach |
| Cancellation after the executor returns | No-op; the token is advisory, not durable |
| `max_memory_bytes` is a soft estimate | Treated as a hard limit; breach is `Err(LimitExceeded { Memory, .. })` |

## Out of Scope

- Per-backend resource accounting (E28.2)
- Cost-based limit recommendation (future)
- Adaptive limit tuning

## ADDED Requirements (E28.4 Analytics Registry Cohort 1)

### Requirement: Persistent descriptor limit policy

Every admitted analytics descriptor MUST declare its complexity class,
applicable limit dimensions, defaults, hard maxima, and whether each breach is
hard failure or soft truncation. The policy MUST be durably retrievable and
immutable for that descriptor version. A policy change MUST require a new
version and MUST change the normalized plan hash.

#### Scenario: Complete policy survives restart

- GIVEN an admitted descriptor with a complete limit policy
- WHEN the registry restarts and the descriptor version is loaded
- THEN the same defaults, maxima, and breach behaviors are available

#### Scenario: Incomplete policy blocks admission

- GIVEN a descriptor omitting its complexity class or truncation behavior
- WHEN admission is attempted
- THEN `DescriptorIncomplete` lists the omissions and the algorithm is not admitted

#### Scenario: Policy change requires version change

- GIVEN an admitted descriptor version whose hard node maximum changes
- WHEN admission reuses the same descriptor version
- THEN admission is rejected as `DescriptorVersionConflict`

### Requirement: Effective analytics limits

Every analytics run MUST resolve effective limits before execution. A caller MAY
tighten descriptor defaults but MUST NOT exceed hard maxima, remove required
limits, or request an unbounded traversal. Bounded shortest paths MUST have an
effective hop bound.

#### Scenario: Caller tightens a default

- GIVEN a descriptor default of 1,000 result rows and a caller limit of 100
- WHEN limits are resolved
- THEN the effective result-row limit is 100

#### Scenario: Caller attempts to widen a hard maximum

- GIVEN a descriptor hard maximum of 10,000 visited nodes
- WHEN a caller requests 20,000
- THEN validation returns `LimitPolicyViolation` before execution

#### Scenario: Shortest path remains bounded

- GIVEN a bounded-shortest-path request with no caller or descriptor hop bound
- WHEN limits are resolved
- THEN validation returns `MissingLimit(MaxHops)` and no run begins

### Requirement: Analytics limit outcome

Analytics runs MUST reuse the E28.2 typed hard-error and soft-truncation
semantics. Elapsed time, memory, cancellation, visited nodes, and visited edges
are hard boundaries and MUST fail without partial success. Result rows, path
count, and response bytes are soft boundaries and MUST return a deterministic
prefix with the exact truncation dimension. A descriptor MAY make a soft
dimension stricter, but MUST NOT soften a hard E28.2 dimension. Run lineage and
every product surface MUST expose the outcome.

#### Scenario: Soft limit truncates visibly

- GIVEN a run with `max_result_rows=10` that finds 30 rows
- WHEN the soft boundary fires
- THEN 10 rows and `ResultRowsLimit` are returned and recorded in lineage

#### Scenario: Hard limit returns typed error

- GIVEN a run that exceeds its memory maximum
- WHEN the hard boundary fires
- THEN it returns `LimitExceeded(Memory)` with no partial success

#### Scenario: Zero path count is empty

- GIVEN a run with `max_path_count=0`
- WHEN execution begins
- THEN it returns an empty, non-error result rather than an implicit unbounded run

## Dependencies

- `MoldPlan` / `GraphPlan` (moldplan-graphplan)
- `ExecutionError` (executor-semantics)
- ADR-014 §7
