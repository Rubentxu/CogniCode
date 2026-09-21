# Delta for explorerql-targets

## ADDED Requirements

### Requirement: Typed node anchors

Pattern node labels MUST resolve to registered `NodeKind` names and constrain anchor and traversed nodes by that type. Labels MUST be case-insensitive while preserving canonical result types. An omitted label MAY match any node type.

#### Scenario: Typed anchors constrain endpoints
- GIVEN `MATCH (r:Route)-[:Calls*1..3]->(f:Function) RETURN r,f`
- WHEN the pattern executes
- THEN `r` MUST bind only Route nodes
- AND `f` MUST bind only Function nodes

#### Scenario: Unknown node label rejected
- GIVEN `MATCH (x:Widget)-[:Calls]->(f:Function) RETURN x`
- WHEN the query is lowered
- THEN an unsupported-construct diagnostic MUST identify `Widget` as an unknown node label

### Requirement: Typed edge anchors

Pattern edge labels MUST resolve to registered `EdgeKind` names and constrain traversed relationships by that type. An omitted edge label MAY match any supported read-only relationship.

#### Scenario: Edge type is enforced
- GIVEN a graph containing Calls and Imports edges between matching nodes
- WHEN `MATCH (a:Function)-[:Calls]->(b:Function) RETURN EDGE(a,b)` executes
- THEN only Calls edges MUST be returned

#### Scenario: Unknown edge label rejected
- GIVEN `MATCH (a:Function)-[:Teleports]->(b:Function) RETURN b`
- WHEN the query is lowered
- THEN an unsupported-construct diagnostic MUST identify `Teleports` as unknown

### Requirement: Direction applies to typed anchors

Outgoing, incoming, and both-direction patterns MUST constrain traversal relative to the left-hand binding without changing node or edge types.

#### Scenario: Incoming direction
- GIVEN `MATCH (callee:Function)<-[:Calls]-(caller:Function) RETURN caller`
- WHEN the pattern executes
- THEN only Functions with a Calls edge into `callee` MUST be returned

#### Scenario: Both direction
- GIVEN `MATCH (a:Function)-[:Calls]-(b:Function) RETURN b`
- WHEN the pattern executes
- THEN matching Calls edges in either direction MUST qualify
- AND duplicate bindings MUST follow typed multiset semantics
