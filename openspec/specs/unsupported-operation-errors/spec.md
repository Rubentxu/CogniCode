# unsupported-operation-errors Specification (NEW)

## Purpose

Structured error emitted by the parser, lowering, or plan-checker when
a query references a construct the v1 contract does not support. The
error MUST identify the construct, describe the supported bounded
alternative, and never surface as an empty success (ADR-014 §4;
graph-query-execution "Unsupported constructs fail before execution").

## Requirements

### Requirement: UnsupportedConstruct Error

`UnsupportedConstruct` is a struct with `construct: ConstructId`,
`message: String`, `supported_alternative: Option<String>`,
`location: Option<SourceLocation>`. The type implements `std::error::Error`
and `Display`.

#### Scenario: Error carries construct id and alternative

- GIVEN an `UnsupportedConstruct { construct: "UnboundedPath", message: "…", supported_alternative: Some("BoundedPath{1..=N}"), location: None }`
- WHEN `Display` is called
- THEN the output includes the construct id, the message, and the suggested alternative

#### Scenario: ConstructId is exhaustive

- GIVEN the `ConstructId` enum
- WHEN each variant is enumerated
- THEN the set covers: `UnboundedPath`, `UnboundedQuantifier`, `UnboundedRecursion`, `MutatingClause`, `PatternProfileFeature`, `GraphAnalyticsFeature`, `Other(String)`

### Requirement: Raised Before Execution

A plan that contains an unsupported construct MUST be rejected by
parsing, lowering, or `PlanLimits::validate` — never by the executor
mid-traversal. The check is exhaustive: every AST variant rejected by
the parser must surface as `UnsupportedConstruct`, not a generic
`CompileError`.

#### Scenario: Parser rejects unbounded quantifier

- GIVEN `MoldQLQuery::Path { max_hops: None, … }` constructed via raw AST
- WHEN the plan-checker validates the plan
- THEN the result is `Err(UnsupportedConstruct { construct: UnboundedPath, … })`

#### Scenario: Mutating clause is rejected

- GIVEN a query containing a `DELETE` clause (future feature)
- WHEN parsed
- THEN the parser returns `Err(UnsupportedConstruct { construct: MutatingClause, supported_alternative: None, … })`

### Requirement: Identifies the Supported Alternative

When a supported alternative exists, the error MUST include it in
`supported_alternative`. The alternative is a short pre-formatted hint
suitable for surfacing to an MCP caller or the Explorer UX.

#### Scenario: Suggests bounded alternative

- GIVEN an unbounded quantifier
- WHEN the error is constructed
- THEN `supported_alternative == Some("Use a bounded quantifier `1..=N`")`

#### Scenario: No alternative for mutating clauses

- GIVEN a mutating clause
- WHEN the error is constructed
- THEN `supported_alternative == None` (mutations are out of v1 scope — no bounded variant)

### Requirement: Source Location

When the parser can locate the unsupported construct, the error MUST
include a `SourceLocation { line, column, byte_offset }`. The location
enables the UI to highlight the offending token.

#### Scenario: Location precision

- GIVEN a query string `PATH FROM a TO b *` where `*` is the unsupported token
- WHEN parsed
- THEN the error's `location` is `Some(SourceLocation { line: 1, column: 20, byte_offset: 19 })`

#### Scenario: Lowering without a source location

- GIVEN an unsupported construct created during lowering (no source text)
- WHEN the error is constructed
- THEN `location == None` is permitted

### Requirement: Distinct from CompileError

`UnsupportedConstruct` is its own error path. The legacy
`CompileError::UnsupportedVariant` is the bridge for the pre-E28.1
caller surface; new code MUST emit `UnsupportedConstruct` instead.

#### Scenario: Bridge mapping

- GIVEN a legacy caller receiving `CompileError::UnsupportedVariant("…")`
- WHEN the bridge translates the error
- THEN the new caller-facing surface is `MoldError::UnsupportedConstruct { … }`
- AND the original construct id is preserved

### Requirement: No Empty Success for Unsupported Syntax

The executor MUST NOT return `Ok(ResultSet { rows: 0, … })` for an
unsupported construct. The unsupported construct is a precondition
failure; the result is always `Err(UnsupportedConstruct { .. })`.

#### Scenario: Executor refuses to swallow the error

- GIVEN a plan that slipped through validation (defensive regression gate)
- WHEN the executor detects the unsupported construct at runtime
- THEN it returns `Err(UnsupportedConstruct { .. })` — never an empty success

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Construct id is not in the predefined set | `ConstructId::Other("graph.cycles")` — allowed for forward compat |
| User-supplied alternative text overflows | Truncated to 256 chars; original kept in logs |
| Error serialised for MCP | JSON shape: `{ construct, message, supported_alternative, location }` |
| Multiple unsupported constructs in one query | First one wins; orchestrator may retry after fixing it |

## Out of Scope

- User-facing remediation flows (UX lives in the Explorer UI)
- Auto-downgrade to a "naive" alternative (the contract is fail-closed)
- Translation table for legacy `CompileError` variants other than `UnsupportedVariant`

## Dependencies

- `GraphPlan` (moldplan-graphplan)
- `ExecutionError` (executor-semantics)
- Legacy `CompileError` (kept behind a bridge — explorerql-compilation delta)
- ADR-014 §4; graph-query-execution "Unsupported constructs fail before execution"
