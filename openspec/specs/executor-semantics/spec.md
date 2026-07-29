# executor-semantics Specification (NEW)

## Purpose

Normative semantics that both the PostgreSQL and the snapshot
executor MUST satisfy for typed values, multiset identity, ordering,
paths, errors, truncation, provenance, and numeric tolerance
(ADR-014 §4). The contract is captured as value-object types so E28.2
executors can be tested against golden fixtures rather than re-reading
prose.

## Requirements

### Requirement: Typed Value Envelope

`TypedValue` is an enum with variants `Null`, `Bool`, `Int(i64)`,
`Float(f64)`, `String(String)`, `Json(serde_json::Value)`. A missing
property is `TypedValue::Null`. Numeric precision is f64; range
exceeding i64 MUST surface as `Float`.

#### Scenario: Missing property is Null

- GIVEN a node with no `properties.label`
- WHEN the executor reads `label`
- THEN it returns `Ok(TypedValue::Null)` — not an error, not panicking

#### Scenario: Overflow promotes to Float

- GIVEN a node property whose value is `9_007_199_254_740_993` (above i64::MAX)
- WHEN the executor parses it
- THEN the result is `Ok(TypedValue::Float(9_007_199_254_740_993.0))`

### Requirement: Multiset Identity and Ordering

`ResultSet` is a multiset: same value twice is two entries. `ResultSet`
MAY carry an `Ord` marker (`Ordered`, `Unordered`) checked by `assert_equivalent`.

#### Scenario: Unordered multiset equivalence

- GIVEN two `ResultSet`s with the same elements in different order
- WHEN `assert_equivalent(&a, &b)` is called
- THEN the result is `Ok(())`

#### Scenario: Ordered path equivalence

- GIVEN two `ResultSet::Ordered` paths `["a","b","c"]` and `["a","c","b"]`
- WHEN `assert_equivalent(&a, &b)` is called on the path subset
- THEN the result is `Err(SemanticsViolation::PathOrderMismatch)`

### Requirement: Path Node and Edge Sequence

`Path` is `Vec<(NodeId, Vec<EdgeKind>)>` where each hop carries the
edges used. The executor MUST emit paths in traversal order; reordering
is a parity violation.

#### Scenario: Path preserves edge kinds

- GIVEN a 2-hop path `A --calls--> B --imports--> C`
- WHEN the executor returns the path
- THEN `path[0].1 == [EdgeKind::Dependency(Calls)]` and `path[1].1 == [EdgeKind::Dependency(Imports)]`

### Requirement: Error Envelope

`ExecutionError` is an enum with `InternalError(String)`,
`PlanError(PlanError)`, `LimitExceeded { limit: PlanLimit, observed: u64 }`,
`UnsupportedConstruct(UnsupportedConstruct)`, `RevisionUnknown { workspace, revision }`.
The executor MUST raise `UnsupportedConstruct` before any partial
execution; it MUST NOT return an empty success for unsupported syntax.

#### Scenario: Unsupported construct is raised pre-execution

- GIVEN a plan with an unbounded quantifier
- WHEN the executor begins the run
- THEN it returns `Err(UnsupportedConstruct { construct: "UnboundedPath", alternative: "BoundedPath{1..=N}" })`
- AND no rows have been touched

#### Scenario: Limit exceeded is typed

- GIVEN a plan with `path_count_limit = 100` and the executor reaches 101 paths
- WHEN the boundary fires
- THEN the result is `Err(LimitExceeded { limit: PlanLimit::PathCount, observed: 101 })`

### Requirement: Truncation

A `ResultSet` carries a `truncated: TruncationMarker` value. When a
limit permits graceful truncation, the executor returns
`Ok(set.with_truncated(reason))` rather than an error.

#### Scenario: Truncation is explicit

- GIVEN a `result_rows_limit = 50` and a query producing 200 rows
- WHEN the executor reaches the limit
- THEN it returns `Ok(ResultSet { rows: 50, truncated: TruncationMarker::ResultRowsLimit, … })`

#### Scenario: Non-truncatable limit produces error

- GIVEN a `time_limit` and a query that exceeds it
- WHEN the boundary fires
- THEN the result is `Err(LimitExceeded { limit: PlanLimit::Time, observed })` — not a truncated success

### Requirement: Provenance Reporting

Every typed value originating from a graph edge carries a
`ProvenanceSource` (`Lsp`, `TreeSitter`, `Postgres`, …). The executor
MUST preserve the source through the result envelope.

#### Scenario: Edge provenance survives round-trip

- GIVEN an edge with `provenance.source == "lsp"`
- WHEN the executor returns the typed multiset
- THEN every row referencing that edge carries `ProvenanceSource::Lsp`

### Requirement: Numeric Tolerance

Approximate numeric results (PageRank, betweenness, similarities —
E28.4+) MUST carry a `tolerance: f64` on the value object. Two
approximate values compare equal when their absolute difference is
<= `tolerance`.

#### Scenario: Within tolerance

- GIVEN `Float(0.5)` and `Float(0.5000001)` with `tolerance = 1e-6`
- WHEN `assert_approx_equal(&a, &b)`
- THEN the result is `Ok(())`

#### Scenario: Outside tolerance

- GIVEN `Float(0.5)` and `Float(0.51)` with `tolerance = 1e-6`
- WHEN `assert_approx_equal(&a, &b)`
- THEN the result is `Err(SemanticsViolation::ToleranceExceeded { delta: 0.01, tolerance: 1e-6 })`

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| `TypedValue::Float(f64::NAN)` | Rejected at construction with `ValueError::NotFinite` |
| Empty `ResultSet` vs absent path | Different from truncation; `truncated = None` |
| Mixed `ProvenanceSource` on a single node | Node-level source; edges keep their own |
| Path with self-loop | Permitted; the self-loop edge appears in the hop's edges |
| Limit exceeded mid-traversal | Atomic: emit typed error, no partial result |

## Out of Scope

- Concrete executor implementations (E28.2)
- Conformance fixtures (E28.2)

## ADDED Requirements (E28.4 Analytics Registry Cohort 1)

### Requirement: Analytics result envelope reuse

All analytics modes MUST represent values with the existing typed-value envelope
and MUST represent empty results, hard errors, and soft truncation with the
existing result semantics. Streaming MUST expose any final truncation marker;
`stats`, `annotate`, and `persist` MUST NOT convert a typed failure into an empty
success.

#### Scenario: Empty analytics result is successful

- GIVEN an admitted algorithm runs on an empty compatible projection
- WHEN execution completes within limits
- THEN it returns an empty result with no error and no truncation marker

#### Scenario: Streaming retains truncation

- GIVEN a stream reaches its soft row limit after emitting rows
- WHEN the stream terminates
- THEN its terminal outcome carries `ResultRowsLimit`

#### Scenario: Hard error is not an empty result

- GIVEN an analytics run exceeds a hard time limit
- WHEN execution terminates
- THEN it returns typed `LimitExceeded(Time)` and no successful result

## MODIFIED Requirements (E28.4 Analytics Registry Cohort 1)

### Requirement: Numeric Tolerance

Approximate analytics results MUST expose their effective absolute tolerance.
Cohort-1 defaults SHALL be `1e-6` for PageRank scores, `1e-9` for floating-point
bounded-shortest-path costs, and zero for SCC/WCC memberships and shortest-path
node/edge sequences. Two finite approximate values compare equal when their
absolute difference is less than or equal to the effective tolerance; non-finite
values MUST be rejected.

(Previously: Approximate values carried a caller-supplied tolerance, but algorithm-specific defaults were deferred to E28.4+.)

#### Scenario: Within tolerance

- GIVEN PageRank scores `0.5` and `0.5000001` with the default tolerance `1e-6`
- WHEN approximate equivalence is evaluated
- THEN the result is equivalent

#### Scenario: Outside tolerance

- GIVEN PageRank scores `0.5` and `0.51` with the default tolerance `1e-6`
- WHEN approximate equivalence is evaluated
- THEN it returns `ToleranceExceeded` with delta `0.01` and tolerance `1e-6`

#### Scenario: Structural results remain exact

- GIVEN SCC memberships `{A,B},{C}` and `{A},{B,C}`
- WHEN equivalence is evaluated with the cohort default
- THEN the results are not equivalent because membership tolerance is zero

#### Scenario: Shortest-path cost uses its default

- GIVEN path costs `1.0` and `1.0000000005`
- WHEN approximate equivalence is evaluated with default `1e-9`
- THEN the costs are equivalent while their node and edge sequences still require exact equality

## Dependencies

- `MoldPlan` / `GraphPlan` (moldplan-graphplan)
- `PlanLimits` (plan-limits)
- `UnsupportedConstruct` (unsupported-operation-errors)
- `Provenance` (generic-graph-model)
- ADR-014 §4
