# Delta for moldql

## ADDED Requirements

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
