# explorerql-filters Specification (NEW)

## Purpose

Defines the `WHERE` filter syntax for ExplorerQL clauses. Filters restrict which nodes/edges are kept in the result based on provenance and confidence fields. The grammar is a strict superset of the existing MoldQL `WHERE` (which already supports `field op value` with `AND` chaining). ExplorerQL adds provenance-prefixed fields and confidence interval filters without changing the existing Condition AST shape.

## Requirements

### Requirement: Provenance Field Filter

The parser MUST accept a WHERE condition of the form `provenance.<source> = "<value>"` where `<source>` is one of `lsp`, `tree_sitter`, `postgres`, `runtime`, `manual`. The dotted field MUST be stored as `Field::dotted("provenance", source)`. Unknown provenance sources MUST produce `ParseError` listing the 5 valid sources.

#### Scenario: LSP provenance filter
- GIVEN `PATH FROM "a" TO "b" WHERE provenance.lsp = "go_to_definition"`
- WHEN parsed
- THEN the `Field` MUST equal `Field::dotted("provenance", "lsp")`
- AND the value MUST be `Value::String("go_to_definition")`
- AND the operator MUST be `Op::Eq`

#### Scenario: Multiple provenance sources
- GIVEN `SUBGRAPH ROOT "a" WHERE provenance.postgres = "lsp_proxy" AND provenance.lsp = "go_to_definition"`
- WHEN parsed
- THEN the conditions vector MUST contain 2 elements
- AND each MUST have a `Field` whose `head()` is `"provenance"`

#### Scenario: Unknown provenance source rejected
- GIVEN `PATH FROM "a" TO "b" WHERE provenance.magic = "x"`
- WHEN parsed
- THEN the error MUST list the 5 valid sources

### Requirement: Confidence Interval Filter

The parser MUST accept `confidence >= <f64>`, `confidence <= <f64>`, `confidence > <f64>`, `confidence < <f64>`, and `confidence == <f64>`. The value MUST be in the closed interval `[0.0, 1.0]`. A value outside that range MUST produce `ParseError` mentioning the valid range. The `Field` MUST be stored as `Field::single("confidence")`.

#### Scenario: Lower bound confidence filter
- GIVEN `NEIGHBORS "a" DEPTH 3 WHERE confidence >= 0.5`
- WHEN parsed
- THEN the condition MUST be `confidence >= 0.5`
- AND `value` MUST equal `Value::Number(0.5)`

#### Scenario: Upper bound confidence filter
- GIVEN `SUBGRAPH ROOT "a" WHERE confidence < 0.9`
- WHEN parsed
- THEN `value` MUST equal `Value::Number(0.9)`

#### Scenario: Confidence outside [0,1] rejected
- GIVEN `PATH FROM "a" TO "b" WHERE confidence > 1.5`
- WHEN parsed
- THEN the error MUST mention `confidence must be in [0.0, 1.0]`

#### Scenario: Negative confidence rejected
- GIVEN `PATH FROM "a" TO "b" WHERE confidence >= -0.1`
- WHEN parsed
- THEN the error MUST mention `confidence must be in [0.0, 1.0]`

### Requirement: Combined Provenance and Confidence

A single WHERE clause MAY combine provenance and confidence conditions with AND (inherited from MoldQL's existing AND-chaining). The order of conditions is preserved in the AST's `Vec<Condition>`.

#### Scenario: Provenance AND confidence
- GIVEN `PATH FROM "a" TO "b" WHERE provenance.lsp = "go_to_definition" AND confidence >= 0.7`
- WHEN parsed
- THEN the conditions vector MUST have exactly 2 entries
- AND the first MUST be the provenance condition
- AND the second MUST be the confidence condition

#### Scenario: Three-way AND chain
- GIVEN `SUBGRAPH ROOT "a" WHERE provenance.postgres = "x" AND provenance.lsp = "y" AND confidence > 0.5`
- WHEN parsed
- THEN the conditions vector MUST have exactly 3 entries in order

### Requirement: WHERE on Every ExplorerQL Primitive

The parser MUST accept a `WHERE` clause on PATH, NEIGHBORS, SUBGRAPH, CLUSTER, and EXPLAIN. The clause is optional on every primitive except that the syntax MUST be positionally consistent: WHERE appears after the primitive's required tokens and before any future boolean combinator (`AND/OR/NOT` between primitives, see `explorerql-boolean`).

#### Scenario: WHERE on PATH
- GIVEN `PATH FROM "a" TO "b" MAX_HOPS 5 WHERE confidence > 0.5`
- WHEN parsed
- THEN the path MUST have `max_hops = Some(5)`
- AND the conditions vector MUST contain the confidence condition

#### Scenario: WHERE on CLUSTER
- GIVEN `CLUSTER METHOD scc WHERE confidence >= 0.8`
- WHEN parsed
- THEN the method MUST be `Scc`
- AND the conditions MUST be present

#### Scenario: WHERE absent is legal
- GIVEN `CLUSTER`
- WHEN parsed
- THEN `conditions` MUST be empty (or `None` — see TDD gate)

## Edge Cases

| Case | Input | Expected |
|------|-------|----------|
| Confidence exactly 0.0 | `... WHERE confidence >= 0` | `Ok` — boundary inclusive |
| Confidence exactly 1.0 | `... WHERE confidence <= 1.0` | `Ok` — boundary inclusive |
| Provenance quoted with case | `... WHERE provenance.LSP = "x"` | Source MUST be normalized to lowercase or error |
| Empty WHERE value | `PATH FROM "a" TO "b" WHERE confidence >=` | `ParseError: expected value after operator` |
| Confidence as integer | `... WHERE confidence >= 1` | `Ok` — must coerce to `1.0` |

## Out of Scope

- Per-edge-kind filters (e.g. `kind = "import"`) — these belong to graph_cluster/subgraph, not the WHERE clause
- Confidence distribution histograms
- Re-weighting confidence at query time
- Soft filters (probabilistic predicates)
- `MATCH ... WHERE` SQL-style patterns
- `provenance` with no value (`WHERE provenance`) — must always be a comparison

## TDD RED Gate

Before any implementation:

1. The existing `Condition`, `Field`, `Op`, `Value` AST types MUST NOT be modified.
2. New tests in `moldql/parser_explorerql_filters.rs` (or `#[cfg(test)] mod explorerql_filters_tests`) MUST cover all 11 scenarios above.
3. A `trybuild` or compile-fail test MUST assert that `provenance.foo` (unknown source) fails parsing.
4. A fuzz-style property test SHOULD assert that `parse(s).is_ok()` for 50 randomly generated valid filters.

The RED gate fails if any new condition passes the parser before the source-list constant or the range-check is implemented.
