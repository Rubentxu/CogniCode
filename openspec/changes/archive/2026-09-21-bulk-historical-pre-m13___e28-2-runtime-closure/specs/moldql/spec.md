# Delta for moldql

> Change: `e28-2-runtime-closure`. `compile_to_plan` is the sole normal
> production entry; legacy compilation exists only for incident rollback.

## ADDED Requirements

### Requirement: `compile_to_plan` is the sole compilation entry

`compile_to_plan(query, pin)` MUST be the default and sole normal production
entry into a backend-neutral, pinned `MoldPlan`. REST and MCP callers MUST NOT
select a backend through `CompileTarget`.

#### Scenario: compile_to_plan returns a pinned MoldPlan

- GIVEN a path query and pin `(ws1, 3)`
- WHEN `compile_to_plan` runs
- THEN it returns a graph plan carrying that pin

#### Scenario: Unsupported construct fails before dispatch

- GIVEN an unbounded construct
- WHEN `compile_to_plan` runs
- THEN it returns `UnsupportedConstruct` and no executor is called

### Requirement: Legacy `compile()` is a temporary rollback facade

Legacy `compile(query, target)` MAY remain linked only for an explicit,
temporary incident rollback and migration tests. Rollback mode MUST be disabled
by default, MUST be observable, and MUST NOT be selected by normal REST or MCP
requests. When rollback mode is active, the facade MUST delegate through the
compatibility adapter and emit a deprecation warning.

#### Scenario: Normal production cannot select legacy compile

- GIVEN rollback mode is disabled
- WHEN REST or MCP executes a graph query
- THEN it uses `compile_to_plan` and never calls `compile()`

#### Scenario: Explicit rollback is temporary

- GIVEN an operator explicitly enables incident rollback mode
- WHEN a query uses the legacy facade
- THEN the request records rollback metadata and emits deprecation
- AND disabling the override restores the sole normal plan path

## MODIFIED Requirements

### Requirement: Lowering integrates with the execute_query facade

The `execute_query` facade MUST consult `lower_intent` before `parse`. A lowered
AST MUST bypass parsing; otherwise parse diagnostics MUST propagate unchanged.
Every successful AST MUST then flow through `compile_to_plan` with an explicit
pin and MUST NOT flow through legacy `compile()` during normal operation.
(Previously: the facade selected lowering or parsing but did not require the
sole pinned plan-compilation route.)

#### Scenario: Lowercase query executes via the lowered AST

- GIVEN `symbols where kind = "function"` lowers successfully
- WHEN the facade processes it
- THEN parsing is skipped and the AST flows through `compile_to_plan`

#### Scenario: Input that neither lowers nor parses returns an error

- GIVEN input matches neither grammar
- WHEN the facade processes it
- THEN the canonical parser diagnostic is surfaced as `ResolutionFailed`

#### Scenario: Parsed AST flows through compile_to_plan

- GIVEN an uppercase path query parses successfully
- WHEN the facade resolves it
- THEN it calls `compile_to_plan(ast, pin)`, never legacy `compile()`
- AND the executor receives the resulting plan
