# Delta for explorerql-grammar

## ADDED Requirements

### Requirement: Pattern syntax

`MATCH <pattern> [WHERE ...] RETURN <projection> [ORDER BY ...] [LIMIT n]` MUST parse as the Pattern Profile query variant. Patterns MUST support typed node forms `(binding:NodeType)`, typed edge forms `[binding:EdgeType]`, outgoing `->`, incoming `<-`, and both-direction `-` relationships.

#### Scenario: Typed outgoing pattern parses
- GIVEN `MATCH (r:Route)-[c:Calls*1..3]->(f:Function) RETURN PATH(r,c,f)`
- WHEN `parse()` is called
- THEN the AST MUST retain bindings, types, outgoing direction, and bounds `1..3`

#### Scenario: Unbounded syntax is rejected
- GIVEN `MATCH (a:Function)-[:Calls*]->(b:Function) RETURN b`
- WHEN `parse()` is called
- THEN it MUST return an unsupported-construct diagnostic for `*`

## MODIFIED Requirements

### Requirement: Top-Level Dispatch

The `parse()` entry point MUST accept a leading keyword among `{FIND, EXPLORE, PATH, NEIGHBORS, SUBGRAPH, CLUSTER, EXPLAIN, MATCH}` case-insensitively. It MUST produce the corresponding distinct query variant. Unknown leading keywords MUST produce `ParseError` listing all eight valid keywords.

(Previously: Dispatch accepted seven keywords and had no Pattern Profile entry.)

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
- AND the error message MUST list all eight valid leading keywords

#### Scenario: MATCH dispatches to pattern variant
- GIVEN `MATCH (f:Function) RETURN NODE(f)`
- WHEN `parse()` is called
- THEN the result MUST be the distinct Pattern Profile query variant

### Requirement: Superset Guarantee

The parser MUST continue accepting every existing MoldQL and ExplorerQL query without changing its variant, fields, or wire format. Pattern syntax MUST be additive.

(Previously: The guarantee covered FIND, EXPLORE, and 32 documented MoldQL variants before MATCH existed.)

#### Scenario: Existing FIND query still parses
- GIVEN `FIND symbols WHERE kind = "Function" AND fan_in > 3 APPLY complexity`
- WHEN parsed
- THEN the result MUST be `MoldQLQuery::Find(...)` with the same fields as before

#### Scenario: Existing EXPLORE query still parses
- GIVEN `EXPLORE symbol:src/main.rs:main:1 THROUGH callers DEPTH 3`
- WHEN parsed
- THEN the result MUST be `MoldQLQuery::Explore(...)` unchanged
