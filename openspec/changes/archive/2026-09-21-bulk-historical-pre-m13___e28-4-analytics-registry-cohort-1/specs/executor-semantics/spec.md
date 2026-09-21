# Delta for executor-semantics

## ADDED Requirements

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

## MODIFIED Requirements

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
