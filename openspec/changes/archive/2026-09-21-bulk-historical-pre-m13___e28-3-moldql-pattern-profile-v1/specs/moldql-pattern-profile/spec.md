# MoldQL Pattern Profile Specification

## Purpose

This profile defines bounded, read-only graph-pattern queries and their observable results. It is inspired by graph query languages but MUST NOT claim Cypher, openCypher, or ISO GQL compatibility.

## Supported-feature matrix

| Construct | v1 status | Constraint |
|---|---|---|
| Typed nodes and edges | Supported | Registered graph types only |
| Incoming, outgoing, both | Supported | Relative to the left binding |
| `*m..n`, `+`, `?` | Bounded | Every effective maximum MUST be finite |
| Property, provenance, confidence predicates | Supported | Typed comparisons; confidence in `[0,1]` |
| Row, node, edge, path projections | Supported | Types and nulls preserved |
| `COUNT`, `ORDER BY`, `LIMIT` | Supported | Order precedes limit |
| `SHORTEST` | Bounded | Finite maximum required |
| Mutations and unbounded paths | Unsupported | Rejected before execution |
| Optional match, subqueries, compatibility modes | Unsupported | No compatibility claim |

## ADDED Requirements

### Requirement: Typed pattern execution

A pattern MUST support typed node and edge bindings, outgoing, incoming, and both directions, and bounded quantifiers. `*m..n` MUST use finite `n`; `?` SHALL mean `0..1`; `+` SHALL mean `1..profile_max_hops`.

#### Scenario: Typed directed bounded pattern
- GIVEN `MATCH (r:Route)-[c:Calls*1..3]->(f:Function) RETURN PATH(r,c,f)`
- WHEN the query executes
- THEN only Route-to-Function paths of one through three Calls edges MUST be returned
- AND each path MUST preserve ordered node and edge identities

#### Scenario: Zero-or-one-hop quantifier
- GIVEN `MATCH (f:Function)-[:Calls?]->(x:Function) RETURN PATH(f,x)` and no matching Calls edge exists
- WHEN the pattern executes
- THEN one zero-hop path containing `f` and no edge MUST be returned
- AND the query MUST NOT acquire optional-match or null-binding semantics

### Requirement: Typed projections and result shaping

Queries MUST project typed rows, nodes, edges, or paths. They MUST support `COUNT`, `ORDER BY`, and `LIMIT`, with ordering applied before limiting.

#### Scenario: Aggregate ordered rows
- GIVEN a pattern returning `f.module, COUNT(c) AS calls ORDER BY calls DESC LIMIT 5`
- WHEN more than five groups match
- THEN exactly the highest five typed rows MUST be returned in descending order

#### Scenario: Missing projected property
- GIVEN a matching node without a projected optional property
- WHEN a row projection is produced
- THEN that field MUST be a typed null and MUST NOT be omitted or stringified

### Requirement: Bounded shortest path

`SHORTEST` MUST select a minimum-hop matching path within an explicit finite bound. Equal-length paths MUST use deterministic ordering.

#### Scenario: Shortest qualifying path
- GIVEN `MATCH SHORTEST (a:Route)-[:Calls*1..6]->(b:Function) RETURN PATH(a,b)`
- WHEN qualifying paths of lengths two and four exist
- THEN the length-two path MUST be returned

#### Scenario: No path within bound
- GIVEN all qualifying paths exceed the declared bound
- WHEN the shortest-path query executes
- THEN an empty typed path result MUST be returned without widening the bound

### Requirement: Unsupported constructs fail safely

Mutation clauses, unbounded paths, and unsupported syntax MUST be rejected as `UnsupportedConstruct` before executor invocation.

#### Scenario: Unbounded pattern rejected
- GIVEN `MATCH (a:Function)-[:Calls*]->(b:Function) RETURN b`
- WHEN the query is submitted
- THEN `UnsupportedConstruct` MUST identify the unbounded quantifier
- AND no executor MUST be invoked

#### Scenario: Mutation rejected
- GIVEN a pattern containing `CREATE`, `DELETE`, `SET`, or `MERGE`
- WHEN the query is submitted
- THEN `UnsupportedConstruct` MUST identify mutation as read-only-profile violation

### Requirement: Published support and surface parity

A supported-feature matrix MUST identify supported, bounded, and unsupported constructs without compatibility claims. REST, MCP, and Explorer MUST accept the same query and expose equivalent typed success, empty, error, provenance, and truncation states.

#### Scenario: Supported-feature matrix
- GIVEN a client requests Pattern Profile capabilities
- WHEN the matrix is returned
- THEN every v1 construct MUST have an explicit support status and constraint
- AND Cypher, openCypher, and ISO GQL compatibility MUST be marked as not claimed

#### Scenario: Cross-surface parity
- GIVEN the same workspace, revision, and pattern query
- WHEN submitted through REST, MCP, and Explorer
- THEN normalized typed results and errors MUST be equivalent
- AND each successful result MUST expose the same provenance and truncation state
