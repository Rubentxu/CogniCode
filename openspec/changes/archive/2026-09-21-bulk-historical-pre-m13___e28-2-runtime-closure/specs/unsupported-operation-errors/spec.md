# Delta for unsupported-operation-errors

> Change: `e28-2-runtime-closure`. Runtime dispatch completes the fail-closed
> rejection chain.

## ADDED Requirements

### Requirement: Unsupported Constructs Reject Before Dispatch

The runtime composition root MUST reject any plan carrying an unsupported
construct before dispatching to `GraphExecutor`. It MUST return a typed error,
never empty success or a mid-traversal fallback.

#### Scenario: Dispatch guard catches a defensive regression

- GIVEN an unsupported construct bypasses parser and lowering
- WHEN runtime dispatch validates the plan
- THEN it returns `UnsupportedConstruct` and does not call an executor

#### Scenario: Rejection avoids backend initialization

- GIVEN the same invalid plan
- WHEN the dispatch guard rejects it
- THEN no PostgreSQL query or snapshot load is started

## MODIFIED Requirements

### Requirement: Raised Before Execution

An unsupported construct MUST be rejected by parsing, lowering, plan
validation, or the runtime dispatch guard before traversal. The runtime guard
MUST be the final fail-closed layer.
(Previously: rejection ended at parsing, lowering, or plan validation.)

#### Scenario: Parser rejects unbounded quantifier

- GIVEN an unbounded path constructed through a raw AST
- WHEN plan validation runs
- THEN it returns `UnsupportedConstruct::UnboundedPath`

#### Scenario: Mutating clause is rejected

- GIVEN a query contains `DELETE`
- WHEN it is parsed
- THEN it returns `UnsupportedConstruct::MutatingClause`

#### Scenario: Dispatch guard rejects slipped-through constructs

- GIVEN an unsupported flag bypasses earlier validation
- WHEN runtime dispatch evaluates it
- THEN it returns `UnsupportedConstruct` and never invokes an executor
