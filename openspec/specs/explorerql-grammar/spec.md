# explorerql-grammar Specification (NEW)

## Purpose

Defines the 5 graph-native clauses that extend MoldQL: `PATH`, `NEIGHBORS`, `SUBGRAPH`, `CLUSTER`, `EXPLAIN`. Each clause is a new top-level variant of `MoldQLQuery` parsed by the recursive-descent parser and dispatched to the correct `ExplorerService` method. ExplorerQL is a STRICT superset of existing MoldQL — every prior query must continue to parse unchanged.

## Requirements

### Requirement: Top-Level Dispatch

The `parse()` entry point MUST accept a leading keyword among `{FIND, EXPLORE, PATH, NEIGHBORS, SUBGRAPH, CLUSTER, EXPLAIN}` (case-insensitive). The parser MUST produce a new `MoldQLQuery` variant for each ExplorerQL keyword (`Path`, `Neighbors`, `Subgraph`, `Cluster`, `Explain`) and MUST continue to produce `Find` / `Explore` for the existing keywords. Unknown leading keywords MUST produce `ParseError` with message `expected FIND, EXPLORE, PATH, NEIGHBORS, SUBGRAPH, CLUSTER, or EXPLAIN`.

#### Scenario: PATH keyword dispatches to Path variant
- GIVEN the input `PATH FROM "parse" TO "render" MAX_HOPS 5`
- WHEN `parse()` is called
- THEN the result MUST be `MoldQLQuery::Path(PathQuery { ... })`
- AND no other variant MUST be returned

#### Scenario: All 5 ExplorerQL keywords parse
- GIVEN valid syntax for each of PATH, NEIGHBORS, SUBGRAPH, CLUSTER, EXPLAIN
- WHEN each is parsed
- THEN each MUST return its own distinct `MoldQLQuery` variant
- AND none MUST collapse to `Find` or `Explore`

#### Scenario: Unknown leading keyword rejected
- GIVEN the input `WIBBLE symbols`
- WHEN `parse()` is called
- THEN it MUST return `Err(ParseError)`
- AND the error message MUST list all 7 valid leading keywords

### Requirement: PATH Syntax

`PATH FROM <object_ref> TO <object_ref> [MAX_HOPS <u32>]` MUST parse to `PathQuery { from, to, max_hops: Option<u32> }`. `MAX_HOPS` defaults to `None` (executor decides the bound). `from` and `to` are quoted symbol ids (`symbol:path:name:line`). Missing `FROM` or `TO` keyword MUST produce `ParseError` mentioning the missing keyword. A non-integer `MAX_HOPS` value MUST produce `ParseError`.

#### Scenario: Minimal PATH with default hops
- GIVEN `PATH FROM "symbol:a.rs:f:1" TO "symbol:b.rs:g:2"`
- WHEN parsed
- THEN `from` MUST equal `"symbol:a.rs:f:1"`
- AND `to` MUST equal `"symbol:b.rs:g:2"`
- AND `max_hops` MUST be `None`

#### Scenario: PATH with explicit MAX_HOPS
- GIVEN `PATH FROM "a" TO "b" MAX_HOPS 7`
- WHEN parsed
- THEN `max_hops` MUST equal `Some(7)`

#### Scenario: Missing TO keyword rejected
- GIVEN `PATH FROM "a"`
- WHEN parsed
- THEN the error MUST mention `expected \`TO\``

#### Scenario: Non-integer MAX_HOPS rejected
- GIVEN `PATH FROM "a" TO "b" MAX_HOPS deep`
- WHEN parsed
- THEN the error MUST mention `MAX_HOPS must be a non-negative integer`

### Requirement: NEIGHBORS Syntax

`NEIGHBORS <object_ref> DEPTH <u32> [DIRECTION (incoming|outgoing|both)]` MUST parse to `NeighborsQuery { root, depth, direction }`. `direction` defaults to `Direction::Both`. DEPTH MUST be a non-negative integer. The direction keywords MUST be case-insensitive. An unknown direction MUST produce `ParseError` listing the three valid values.

#### Scenario: NEIGHBORS with explicit direction
- GIVEN `NEIGHBORS "symbol:src/x.rs:f:1" DEPTH 3 DIRECTION outgoing`
- WHEN parsed
- THEN `direction` MUST equal `Direction::Outgoing`

#### Scenario: NEIGHBORS defaults to both
- GIVEN `NEIGHBORS "symbol:src/x.rs:f:1" DEPTH 2`
- WHEN parsed
- THEN `direction` MUST equal `Direction::Both`

#### Scenario: Unknown direction rejected
- GIVEN `NEIGHBORS "x" DEPTH 1 DIRECTION sideways`
- WHEN parsed
- THEN the error MUST list `incoming`, `outgoing`, `both`

### Requirement: SUBGRAPH Syntax

`SUBGRAPH ROOT <object_ref> [DEPTH <u32>] [DIRECTION (incoming|outgoing|both)]` MUST parse to `SubgraphQuery { root, depth, direction }`. `depth` defaults to `Some(3)` (matching `DEFAULT_SUBGRAPH_DEPTH`). Missing `ROOT` keyword MUST produce `ParseError`. Both clauses are optional and order-independent.

#### Scenario: SUBGRAPH with depth and direction
- GIVEN `SUBGRAPH ROOT "s" DEPTH 5 DIRECTION incoming`
- WHEN parsed
- THEN `depth` MUST be `Some(5)`
- AND `direction` MUST equal `Direction::Incoming`

#### Scenario: SUBGRAPH with bare root uses default depth
- GIVEN `SUBGRAPH ROOT "s"`
- WHEN parsed
- THEN `depth` MUST be `Some(3)`
- AND `direction` MUST equal `Direction::Both`

#### Scenario: Missing ROOT rejected
- GIVEN `SUBGRAPH "s" DEPTH 2`
- WHEN parsed
- THEN the error MUST mention `expected \`ROOT\``

### Requirement: CLUSTER Syntax

`CLUSTER [METHOD (scc|connected)]` MUST parse to `ClusterQuery { method }`. The query is valid with no body — `CLUSTER` alone is legal. `method` defaults to `ClusterMethod::Scc`. Unknown method MUST produce `ParseError`.

#### Scenario: Bare CLUSTER uses scc
- GIVEN `CLUSTER`
- WHEN parsed
- THEN `method` MUST equal `ClusterMethod::Scc`

#### Scenario: CLUSTER with connected method
- GIVEN `CLUSTER METHOD connected`
- WHEN parsed
- THEN `method` MUST equal `ClusterMethod::Connected`

#### Scenario: Unknown cluster method rejected
- GIVEN `CLUSTER METHOD louvain`
- WHEN parsed
- THEN the error MUST mention `expected \`scc\` or \`connected\``

### Requirement: EXPLAIN Syntax

`EXPLAIN FROM <object_ref> TO <object_ref>` MUST parse to `ExplainQuery { from, to }`. Both endpoints are required. Missing `FROM` or `TO` MUST produce `ParseError`. `MAX_HOPS` MUST NOT be accepted on EXPLAIN (graph_explain does not bound depth).

#### Scenario: Minimal EXPLAIN
- GIVEN `EXPLAIN FROM "a" TO "b"`
- WHEN parsed
- THEN `from` MUST equal `"a"`
- AND `to` MUST equal `"b"`

#### Scenario: Missing FROM rejected
- GIVEN `EXPLAIN "a" TO "b"`
- WHEN parsed
- THEN the error MUST mention `expected \`FROM\``

#### Scenario: EXPLAIN rejects MAX_HOPS
- GIVEN `EXPLAIN FROM "a" TO "b" MAX_HOPS 5`
- WHEN parsed
- THEN the error MUST mention `unexpected token` or `MAX_HOPS`

### Requirement: Superset Guarantee

The parser MUST continue to accept every existing MoldQL query (FIND, EXPLORE, all 32 documented variants) without modification. No existing variant, field, or wire format MAY change. The new clauses MUST be added as new top-level keywords only; the existing parser code paths MUST be untouched.

#### Scenario: Existing FIND query still parses
- GIVEN `FIND symbols WHERE kind = "Function" AND fan_in > 3 APPLY complexity`
- WHEN parsed
- THEN the result MUST be `MoldQLQuery::Find(...)` with the same fields as before

#### Scenario: Existing EXPLORE query still parses
- GIVEN `EXPLORE symbol:src/main.rs:main:1 THROUGH callers DEPTH 3`
- WHEN parsed
- THEN the result MUST be `MoldQLQuery::Explore(...)` unchanged

## Edge Cases

| Case | Input | Expected |
|------|-------|----------|
| Empty input | `""` | `ParseError: empty query — expected ...` |
| Whitespace only | `"   "` | `ParseError: empty query` |
| Trailing tokens | `PATH FROM "a" TO "b" FOO` | `ParseError: unexpected trailing input` |
| Mixed case | `path from "a" to "b"` | `Ok` — keywords are case-insensitive |
| Zero depth | `NEIGHBORS "x" DEPTH 0` | `Ok` — 0 is a valid non-negative integer |
| Quoted object with spaces | `NEIGHBORS "my symbol id" DEPTH 1` | `Ok` — quoted strings preserve spaces |
| Unterminated quote | `PATH FROM "a TO "b"` | `ParseError: unterminated string` |

## Out of Scope

- Query planner / cost-based optimization
- Streaming traversal (result must fit in memory)
- GQL/Cypher syntax compatibility beyond the 5 primitives
- Type checking of object_refs against the graph schema
- Variable binding (`$var` placeholders, `WITH` projections)
- Subqueries or nested EXPLORERQL inside another clause
- Comments (`--`, `//`, `/* */`) inside queries

## TDD RED Gate

Before any implementation of the new variants in `moldql/ast.rs` or `moldql/parser.rs`, the following failing tests MUST exist in `moldql/parser.rs` (or a new `moldql/parser_explorerql.rs` test module):

1. One test per `#### Scenario` block in the 7 requirements above (≥ 23 tests).
2. The existing 32 MoldQL test cases MUST still pass without modification (regression gate).
3. The new test module MUST be a separate `#[cfg(test)] mod explorerql_tests` so a failing compile of the new AST does not block the legacy suite.

The RED gate fails if any of the 23+ new tests passes before the corresponding parser code is written, or if any of the 32 legacy tests is broken by the AST additions.
