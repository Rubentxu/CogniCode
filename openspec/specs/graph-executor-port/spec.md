# Graph Executor Port Specification (NEW)

## Purpose

The backend-neutral `GraphExecutor` trait that consumes a pinned
`GraphPlan` and produces a `ResultSet` (or `ExecutorError`). The trait
is the single contract that the PostgreSQL and snapshot executors BOTH
implement. Executor selection is internal; the call site MUST NOT
observe which backend ran (ADR-014 §4).

## Requirements

### Requirement: GraphExecutor Trait

The `GraphExecutor` trait declares one synchronous method:
`execute(&self, plan: &GraphPlan, pin: (WorkspaceId, RevisionId)) -> Result<ResultSet, ExecutorError>`.
The trait MUST be object-safe (`dyn GraphExecutor`) and the implementor
MUST be `Send + Sync`. A runtime that stores an owned executor across tasks MAY
require a `'static` concrete implementation at its composition boundary; the
trait itself does not declare a `'static` supertrait bound.

#### Scenario: Trait is object-safe

- GIVEN `fn _executor(_: &dyn GraphExecutor) {}`
- WHEN the compiler type-checks the function
- THEN the build succeeds (no `Sized` bound leaks)

#### Scenario: Trait is implementable

- GIVEN a unit struct `StubExecutor`
- WHEN it implements `GraphExecutor` returning `Ok(ResultSet::empty())`
- THEN the implementation compiles

### Requirement: Pinned Plan Input

The executor MUST accept a `GraphPlan` and an explicit `(WorkspaceId,
RevisionId)` pin. The executor MUST NOT read graph state for any
revision other than the supplied pin. The pin is the contract — the
executor MUST NOT silently fall back to head when the pin is unknown.

#### Scenario: Unknown pin fails closed

- GIVEN `(ws = "ws1", rev = 99)` where no revision exists for ws1
- WHEN `execute(&plan, ("ws1", 99))` runs
- THEN the result is `Err(ExecutorError::RevisionUnknown("ws1:99"))`
- AND no rows are returned

#### Scenario: Known pin succeeds

- GIVEN `(ws = "ws1", rev = 3)` where revision 3 exists
- WHEN `execute(&plan, ("ws1", 3))` runs
- THEN the result is `Ok(ResultSet { ... })` or a typed `ExecutorError`

### Requirement: PlanLimits Honored

The executor MUST read `PlanLimits` from `plan.limits()` and enforce
every limit dimension. A breach is either `Err(ExecutorError::LimitExceeded { .. })`
(hard limits: time, memory, cancellation) or `Ok(set.with_truncated(marker))`
(soft limits: rows, paths, visited nodes/edges).

#### Scenario: Soft limit produces truncated success

- GIVEN a `PlanLimits { max_result_rows: Some(3) }` on a plan that finds 10 rows
- WHEN `execute` runs
- THEN the result is `Ok(ResultSet { rows.len() == 3, truncated: true, truncation: Some(ResultRowsLimit) })`

#### Scenario: Hard limit produces typed error

- GIVEN a `PlanLimits { time_ms: Some(1) }` on a slow plan
- WHEN the boundary fires
- THEN the result is `Err(ExecutorError::LimitExceeded { dimension: TimeMs, observed })`

### Requirement: No Empty Success for Unsupported Construct

If the executor detects an unsupported construct at runtime (defense
in depth — the parser normally rejects it), the executor MUST return
`Err(ExecutorError::UnsupportedConstruct(_))`, never an empty
`ResultSet`. The error wraps the `UnsupportedConstruct` from
`unsupported-operation-errors`.

#### Scenario: Unsupported construct is rejected

- GIVEN a `GraphPlan` carrying an unsupported internal flag (defensive)
- WHEN `execute` runs
- THEN the result is `Err(ExecutorError::UnsupportedConstruct(UnsupportedConstruct { .. }))`
- AND no rows are returned

### Requirement: Every GraphPlan Variant Supported

The executor MUST implement every `GraphPlan` variant: `Path`,
`Neighbors`, `Subgraph`, `Cluster`, `Explain`, `BooleanComposition`.
Receiving an unhandled variant is an internal error.

#### Scenario: All six variants dispatch

- GIVEN one `GraphPlan` per variant (Path, Neighbors, Subgraph, Cluster, Explain, BooleanComposition)
- WHEN each is passed to `execute`
- THEN every result is `Ok(ResultSet)` or a typed `ExecutorError`
- AND no variant returns `Err(InternalError("unhandled variant"))`

### Requirement: Provenance Preservation

Typed values originating from graph state MUST carry their
`ProvenanceSource` in the result envelope. The executor MUST NOT
discard provenance during graph-to-result materialization.

#### Scenario: Edge provenance survives

- GIVEN an edge with `ProvenanceSource::StaticAnalysis("calls")`
- WHEN the executor materializes the result
- THEN every `EdgeResult` referencing that edge carries the same source

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Pin is `RevisionId::NONE` | Pre-execution validation rejects; `Err(InvalidPin)` |
| `PlanLimits` is fully `None` | Executor applies internal defaults; no implicit cap |
| Two limits breached simultaneously | First breach wins; executor reports the first detected |
| Plan with `max_hops: 0` | Empty path list; `Ok(ResultSet { paths: vec![] })` |
| Executor panics internally | Caught, returns `Err(InternalError(msg))`; never panics across boundary |

## Out of Scope

- Concrete `GraphExecutor` implementations (pg-graph-executor, snapshot-graph-executor)
- Conformance harness (executor-equivalence-conformance)
- Backend selection policy (deferred to `cognicode-runtime`)
- Analytics admission (E28.4+)

## Dependencies

- `GraphPlan`, `MoldPlan` (moldplan-graphplan)
- `TypedValue`, `ResultSet`, `TruncationMarker`, `Path`, `ExecutorError`, `ProvenanceSource` (executor-semantics)
- `PlanLimits`, `PlanLimit`, `CancellationToken` (plan-limits)
- `UnsupportedConstruct` (unsupported-operation-errors)
- `WorkspaceId`, `RevisionId` (graph-revisions)
- ADR-014 §4
