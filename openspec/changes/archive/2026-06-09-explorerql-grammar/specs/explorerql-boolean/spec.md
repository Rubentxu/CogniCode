# explorerql-boolean Specification (NEW)

## Purpose

Defines how ExplorerQL clauses compose with `AND`, `OR`, `NOT` to express multi-step graph reasoning. A composed query is either (a) a top-level boolean wrapper joining two or more sub-queries, or (b) a `WHERE`-level expression reusing MoldQL's AND with new OR / NOT. Composition is a STRICT additive layer — it never alters an existing primitive's required tokens.

## Requirements

### Requirement: WHERE-Level Boolean Composition

Within a `WHERE` clause, the parser MUST accept `OR` and `NOT` in addition to the existing `AND`. Operator precedence MUST be `NOT > AND > OR` (highest to lowest). Each side of an operator MUST be a `<field> <op> <value>` predicate. Parentheses MUST be supported for explicit grouping.

#### Scenario: AND binds tighter than OR
- GIVEN `PATH FROM "a" TO "b" WHERE confidence > 0.5 OR provenance.lsp = "x" AND kind = "Function"`
- WHEN parsed
- THEN the AST MUST represent it as `OR(confidence > 0.5, AND(provenance.lsp = "x", kind = "Function"))`
- AND NOT as `AND(OR(...), kind = "Function")`

#### Scenario: NOT inverts a single condition
- GIVEN `PATH FROM "a" TO "b" WHERE NOT provenance.lsp = "manual"`
- WHEN parsed
- THEN the conditions vector MUST contain a `NOT` node wrapping the provenance condition
- AND the inner condition's field MUST be `provenance.manual`

#### Scenario: Parentheses override precedence
- GIVEN `PATH FROM "a" TO "b" WHERE (confidence > 0.5 OR provenance.lsp = "x") AND kind = "Function"`
- WHEN parsed
- THEN the AND MUST apply to the parenthesized OR
- AND the OR MUST contain exactly the two inner conditions

#### Scenario: Bare AND still works
- GIVEN `FIND symbols WHERE kind = "Function" AND fan_in > 3`
- WHEN parsed
- THEN the result MUST be `MoldQLQuery::Find` with the same 2 AND-chained conditions
- AND no `Or`/`Not` node MUST be present (regression of existing behavior)

### Requirement: Top-Level Boolean Composition

A query MAY be one of three top-level forms: a single primitive (`PATH ...`), a binary boolean `(<query> AND|OR <query>)`, or a unary `NOT <query>`. The parser MUST accept these forms and produce a new `MoldQLQuery::Boolean(BooleanQuery)` variant. The keyword introducing the top-level form is the FIRST keyword in the input.

#### Scenario: AND of two PATHs
- GIVEN `PATH FROM "a" TO "b" MAX_HOPS 3 AND PATH FROM "c" TO "d" MAX_HOPS 5`
- WHEN parsed
- THEN the result MUST be `MoldQLQuery::Boolean(BooleanQuery::And(lhs, rhs))`
- AND `lhs` and `rhs` MUST each be a `MoldQLQuery::Path(...)`

#### Scenario: OR of PATH and NEIGHBORS
- GIVEN `PATH FROM "a" TO "b" OR NEIGHBORS "c" DEPTH 2`
- WHEN parsed
- THEN the result MUST be `MoldQLQuery::Boolean(BooleanQuery::Or(...))`
- AND one child MUST be `Path`, the other MUST be `Neighbors`

#### Scenario: NOT of a SUBGRAPH
- GIVEN `NOT SUBGRAPH ROOT "a" DEPTH 2`
- WHEN parsed
- THEN the result MUST be `MoldQLQuery::Boolean(BooleanQuery::Not(inner))`
- AND `inner` MUST be a `Subgraph`

#### Scenario: Single primitive is not wrapped
- GIVEN `PATH FROM "a" TO "b"`
- WHEN parsed
- THEN the result MUST be `MoldQLQuery::Path(...)` (NOT a Boolean wrapper)

### Requirement: Mixed Boolean and WHERE

A sub-query in a top-level boolean expression MAY have its own `WHERE` clause. The parser MUST scope each sub-query's WHERE to that sub-query only — filters do not bleed across boolean boundaries.

#### Scenario: WHERE inside AND left side
- GIVEN `PATH FROM "a" TO "b" WHERE confidence > 0.5 AND PATH FROM "c" TO "d" WHERE kind = "Function"`
- WHEN parsed
- THEN the left `Path` MUST have its own conditions vector
- AND the right `Path` MUST have its own conditions vector
- AND they MUST NOT share state

#### Scenario: WHERE inside NOT
- GIVEN `NOT PATH FROM "a" TO "b" WHERE provenance.lsp = "manual"`
- WHEN parsed
- THEN the inner `Path` MUST carry the `provenance.lsp` condition
- AND the `Not` wrapper MUST NOT strip the condition

### Requirement: Disambiguation with Parentheses

A top-level boolean expression MAY be parenthesized: `(PATH FROM "a" TO "b") AND (NEIGHBORS "c" DEPTH 2)`. Parentheses MUST NOT appear around a single primitive without an operator — `(PATH FROM "a" TO "b")` alone is treated as just `PATH FROM "a" TO "b"`.

#### Scenario: Parens force left-associativity
- GIVEN `(PATH FROM "a" TO "b" OR NEIGHBORS "c" DEPTH 1) AND CLUSTER`
- WHEN parsed
- THEN the OR MUST be the left child of the AND
- AND the CLUSTER MUST be the right child

#### Scenario: Bare-paren primitive is unwrapped
- GIVEN `(PATH FROM "a" TO "b")`
- WHEN parsed
- THEN the result MUST be `MoldQLQuery::Path(...)` without a Boolean wrapper

## Edge Cases

| Case | Input | Expected |
|------|-------|----------|
| Double NOT | `NOT NOT PATH FROM "a" TO "b"` | `Ok` — `Not(Not(Path))` |
| AND at EOF | `PATH FROM "a" TO "b" AND` | `ParseError: expected query after AND` |
| Empty parens | `() AND PATH FROM "a" TO "b"` | `ParseError: empty group` |
| NOT without operand | `NOT` | `ParseError: expected query after NOT` |
| Mixed precedence | `A AND B OR C AND D` | `Ok` — parses as `(A AND B) OR (C AND D)` |
| Boolean at WHERE level with no value | `WHERE AND` | `ParseError: expected condition after AND` |

## Out of Scope

- XOR (exclusive or)
- Implication (`A -> B`)
- Quantifiers (`FORALL`, `EXISTS`)
- Short-circuit evaluation semantics (the spec is purely syntactic; executor semantics are a separate concern)
- `UNION` / `INTERSECT` set operators (use OR/AND at the top level)
- Boolean composition across separate MCP tool invocations — composition is single-statement only

## TDD RED Gate

Before any implementation:

1. The new `BooleanQuery` AST type MUST be a distinct variant of `MoldQLQuery` (no in-place modification of `Find`/`Explore`/`Path`).
2. Tests MUST include 12 scenarios covering: AND-precedence, OR-precedence, NOT-inversion, paren-overrides, mixed WHERE-scoping, bare-paren-unwrap, and double-NOT.
3. A regression test MUST assert that every existing 32-test MoldQL corpus still parses identically.
4. A precedence-climbing test (`A AND B OR C AND D`) MUST assert the exact AST tree shape (snapshot test on the `Debug` output of the parsed AST).

The RED gate fails if the precedence snapshot changes after AST edits, or if a new boolean keyword is accepted without an explicit entry in the precedence table.
