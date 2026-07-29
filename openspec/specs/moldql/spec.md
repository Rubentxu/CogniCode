# Intent Lowering Specification

## Purpose

The intent lowering layer translates natural-language-style query strings
(lowercase keywords) into the canonical `MoldQLQuery` AST before they
reach the uppercase parser. It sits at the `execute_query` facade
boundary as a non-strict preprocessor: any input outside its grammar
falls through to the canonical `parse()` function, whose contract is
left untouched.

## Requirements

### Requirement: Lower `symbols where` pattern

The lowering function MUST translate the lowercase pattern
`symbols where <condition>` into a `MoldQLQuery::Find` AST whose target
is `TargetType::Symbols` and whose conditions equal the lowered
`<condition>`. The lowered AST MUST NOT be re-parsed by the canonical
`parse()` function.

#### Scenario: symbols where pattern lowers to FIND

- GIVEN a query string `symbols where kind = "function"`
- WHEN `lower_intent` is called
- THEN it returns a `MoldQLQuery::Find` AST
- AND the target is `TargetType::Symbols`
- AND the conditions contain the single predicate `kind = "function"`

#### Scenario: malformed lowercase FIND returns None

- GIVEN a query string `symbols where` (missing condition)
- WHEN `lower_intent` is called
- THEN it returns `None`

### Requirement: Lower `calls from` pattern with optional depth

The lowering function MUST translate `calls from '<id>' [depth N]`
into a `MoldQLQuery::Explore` AST with `direction` set to
`Direction::Callees`. When `depth` is omitted the AST MUST use
`depth = 1`. The `<id>` value MUST appear verbatim in `object_ref`.

#### Scenario: calls from with explicit depth

- GIVEN a query string `calls from 'sym:42' depth 3`
- WHEN `lower_intent` is called
- THEN it returns a `MoldQLQuery::Explore` AST
- AND `object_ref` is `sym:42`
- AND `direction` is `Direction::Callees`
- AND `depth` is `3`

#### Scenario: calls from without depth defaults to 1

- GIVEN a query string `calls from 'sym:42'`
- WHEN `lower_intent` is called
- THEN it returns a `MoldQLQuery::Explore` AST
- AND `depth` is `1`

### Requirement: Fall through for unrecognised patterns

The lowering function MUST return `None` for any input whose leading
keyword is not recognised by the lowering grammar. This MUST leave the
canonical `parse()` function's contract and behavior unchanged for
already-valid uppercase queries.

#### Scenario: uppercase FIND falls through unchanged

- GIVEN a query string `FIND symbols WHERE fan_out > 5`
- WHEN `lower_intent` is called
- THEN it returns `None`
- AND the canonical `parse()` function handles this input without
  modification

### Requirement: Lowering integrates with the execute_query facade

The `execute_query` facade MUST consult `lower_intent` before
delegating to `parse`. If `lower_intent` returns `Some(ast)` the facade
MUST execute that AST directly without invoking `parse`. If it returns
`None` the facade MUST fall back to `parse()` and propagate the
canonical parser's diagnostic on failure.

#### Scenario: lowercase query executes via the lowered AST

- GIVEN the facade is called with `symbols where kind = "function"`
- AND `lower_intent` returns `Some(ast)` for that input
- WHEN the facade processes the query
- THEN execution proceeds with the lowered AST
- AND the canonical `parse()` is not invoked

#### Scenario: input that neither lowers nor parses returns an error

- GIVEN the facade is called with input that does not match the
  lowering grammar and does not parse as uppercase
- WHEN the facade processes the query
- THEN `parse()` returns an error
- AND the facade surfaces a `ResolutionFailed` diagnostic carrying the
  parser message

## ADDED Requirements (E28.3 MoldQL Pattern Profile v1)

### Requirement: Lower lowercase pattern fragments

The intent-lowering layer MUST recognize lowercase `match ... return ...` Pattern Profile fragments and produce the same typed pattern query as canonical uppercase syntax. It MUST preserve quoted values and binding case and MUST NOT reparse a successfully lowered query.

#### Scenario: Lowercase pattern lowers directly

- GIVEN `match (r:Route)-[c:Calls*1..3]->(f:Function) return path(r,c,f)`
- WHEN `lower_intent` is called
- THEN it MUST return the equivalent typed Pattern Profile AST
- AND the canonical parser MUST NOT be invoked

#### Scenario: Mixed unsupported fragment falls through

- GIVEN `match (f:Function) detach delete f`
- WHEN `lower_intent` is called
- THEN it MUST return `None`
- AND canonical parsing MUST surface the unsupported mutation diagnostic

### Requirement: Lowered patterns preserve bounded semantics

Lowering MUST retain node and edge types, direction, predicates, projections, aggregation, ordering, limits, and finite path bounds. `+` MUST lower with the configured finite profile maximum, and `?` MUST lower as `0..1`.

#### Scenario: Lower aggregate and limit

- GIVEN `match (f:Function)-[c:Calls+]->(g:Function) return count(c) as calls order by calls desc limit 5`
- WHEN lowered with profile maximum 4
- THEN the query MUST retain `1..4`, descending order, and limit 5

#### Scenario: Lower optional relationship

- GIVEN `match (f:Function)-[:Calls?]->(g:Function) return node(g)`
- WHEN lowered
- THEN its relationship bound MUST be `0..1`

### Requirement: Pattern lowering failures remain typed

Recognized but unsupported pattern constructs MUST surface `UnsupportedConstruct` before execution. Lowering MUST NOT convert them into empty results or legacy queries.

#### Scenario: Unbounded lowercase path rejected

- GIVEN `match (a:Function)-[:Calls*]->(b:Function) return b`
- WHEN submitted through `execute_query`
- THEN `UnsupportedConstruct` MUST identify the unbounded path
- AND no executor MUST be invoked

#### Scenario: Existing intent lowering is unchanged

- GIVEN `symbols where kind = "function"` or `calls from 'sym:42' depth 3`
- WHEN `lower_intent` is called
- THEN each MUST produce the same existing AST and defaults as before
