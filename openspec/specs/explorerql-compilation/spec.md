# explorerql-compilation Specification (NEW)

## Purpose

Defines how an `MoldQLQuery` is compiled into one of two execution plans: a PostgreSQL SQL query (for the persistent `edges` / `symbols` tables) or a petgraph dispatch (for the in-memory call graph). The compiler lives in a new `moldql/compile.rs` module and produces a `CompiledQuery` enum that the executor dispatches without further branching on the AST.

## Requirements

### Requirement: Compilation Entry Point

A new public function `compile(query: &MoldQLQuery, target: CompileTarget) -> Result<CompiledQuery, CompileError>` MUST exist in `moldql/compile.rs`. `CompileTarget` is an enum with `Postgres` and `Petgraph` variants. The function MUST be deterministic — same input produces byte-identical output.

#### Scenario: Compile PATH to petgraph
- GIVEN a parsed `PathQuery { from: "a", to: "b", max_hops: None }`
- WHEN compiled with `CompileTarget::Petgraph`
- THEN the result MUST be `CompiledQuery::Petgraph(PetgraphPlan::ShortestPath { from: "a", to: "b", max_hops: None })`

#### Scenario: Compile PATH to postgres
- GIVEN the same `PathQuery`
- WHEN compiled with `CompileTarget::Postgres`
- THEN the result MUST be `CompiledQuery::Postgres(PostgresPlan { sql: "...", params: ["a", "b"] })`
- AND the SQL MUST contain a recursive CTE walking the `edges` table

#### Scenario: Unknown AST variant rejected
- GIVEN a future `MoldQLQuery` variant the compiler does not know
- WHEN compiled
- THEN the result MUST be `Err(CompileError::UnsupportedVariant)`

### Requirement: PostgreSQL Compilation

For each ExplorerQL primitive, the Postgres compiler MUST emit a single `sqlx::query` plan with positional `$1..$N` parameters (no string interpolation of user data — SQL injection safe). The emitted SQL MUST be valid PostgreSQL 14+ syntax and MUST use the existing `symbols` and `edges` schema.

| Primitive | SQL strategy |
|-----------|--------------|
| `PATH` | Recursive CTE on `edges` bounded by `max_hops` |
| `NEIGHBORS` | Single-level JOIN on `edges` with depth filter |
| `SUBGRAPH` | Recursive CTE with `direction` case-switch |
| `CLUSTER` | `SELECT ... FROM find_scc(...)` (existing PG function) for `scc`; `WITH RECURSIVE` for `connected` |
| `EXPLAIN` | JOIN of shortest path + edge metadata |

#### Scenario: PATH with MAX_HOPS bounds the CTE
- GIVEN `PATH FROM "a" TO "b" MAX_HOPS 4`
- WHEN compiled to Postgres
- THEN the SQL MUST contain a `WHERE depth < 4` or equivalent bound
- AND the bound MUST be a parameter, not a string literal

#### Scenario: NEIGHBORS compiles to a single JOIN
- GIVEN `NEIGHBORS "a" DEPTH 1 DIRECTION outgoing`
- WHEN compiled to Postgres
- THEN the SQL MUST be a non-recursive `SELECT ... FROM edges WHERE source = $1`
- AND there MUST be exactly 1 placeholder

#### Scenario: SUBGRAPH outgoing is recursive
- GIVEN `SUBGRAPH ROOT "a" DEPTH 3 DIRECTION outgoing`
- WHEN compiled to Postgres
- THEN the SQL MUST use `WITH RECURSIVE` (or equivalent)
- AND the recursive term MUST join on `target = source`

#### Scenario: CLUSTER scc uses existing function
- GIVEN `CLUSTER METHOD scc`
- WHEN compiled to Postgres
- THEN the SQL MUST be `SELECT * FROM find_scc()` (or whatever the existing scc function is named)
- AND no parameters MUST be present

#### Scenario: EXPLAIN joins edges metadata
- GIVEN `EXPLAIN FROM "a" TO "b"`
- WHEN compiled to Postgres
- THEN the SQL MUST select edge kind, source, target, and confidence
- AND it MUST be a single statement

### Requirement: PostgreSQL Parameterization

The Postgres compiler MUST accept user-supplied values (symbol ids, MAX_HOPS, direction) only as `$N` placeholders. No `format!`, `concat!`, or string concatenation of untrusted data into the SQL string is permitted. WHERE-clause filters (provenance, confidence) MUST be applied as SQL `AND` predicates using placeholders.

#### Scenario: Confidence filter is parameterized
- GIVEN `PATH FROM "a" TO "b" WHERE confidence > 0.5`
- WHEN compiled to Postgres
- THEN the SQL MUST contain `confidence > $N` (not `confidence > 0.5` inline)
- AND the params vector MUST contain `0.5`

#### Scenario: Provenance filter is parameterized
- GIVEN `PATH FROM "a" TO "b" WHERE provenance.lsp = "go_to_definition"`
- WHEN compiled to Postgres
- THEN the SQL MUST contain `provenance->>'source' = $N` (or the existing column-equivalent)
- AND `go_to_definition` MUST be a param, not inline

#### Scenario: No string interpolation in SQL
- GIVEN any ExplorerQL query with a string literal
- WHEN compiled to Postgres
- THEN the SQL string MUST NOT contain the literal's value verbatim (use `$N`)
- AND a `cargo audit`-style linter test MUST scan the output

### Requirement: petgraph Compilation

For each ExplorerQL primitive, the petgraph compiler MUST emit a `PetgraphPlan` enum that the existing `ExplorerService` graph methods accept. The compilation MUST be a pure data translation — no `petgraph::Graph` construction happens in the compiler, only in the executor.

| Primitive | PetgraphPlan variant |
|-----------|----------------------|
| `PATH` | `ShortestPath { from, to, max_hops }` |
| `NEIGHBORS` | `ForwardRadius { root, depth }` (with direction as a flag) |
| `SUBGRAPH` | `Subgraph { root, depth, direction }` |
| `CLUSTER` | `Cluster { method }` |
| `EXPLAIN` | `Explain { from, to }` |

#### Scenario: NEIGHBORS incoming maps to backward radius
- GIVEN `NEIGHBORS "a" DEPTH 3 DIRECTION incoming`
- WHEN compiled to petgraph
- THEN the result MUST be `PetgraphPlan::BackwardRadius { root: "a", depth: 3 }`

#### Scenario: NEIGHBORS both maps to both radii
- GIVEN `NEIGHBORS "a" DEPTH 2 DIRECTION both`
- WHEN compiled to petgraph
- THEN the result MUST be a `Both` plan (or two `PetgraphPlan`s) — not a single direction

#### Scenario: SUBGRAPH outgoing maps to subgraph
- GIVEN `SUBGRAPH ROOT "a" DEPTH 2 DIRECTION outgoing`
- WHEN compiled to petgraph
- THEN the plan MUST be `Subgraph { root: "a", depth: 2, direction: Outgoing }`

### Requirement: WHERE Filters Compile to Both Targets

A WHERE clause MUST compile to BOTH a Postgres predicate AND a petgraph post-filter. The petgraph post-filter is a closure (`Arc<dyn Fn(&EdgeData) -> bool + Send + Sync>`) that the executor applies after traversal. The two targets MUST be semantically equivalent — given the same graph state, both must return the same filtered set.

#### Scenario: Confidence filter compiles to both
- GIVEN `... WHERE confidence > 0.5`
- WHEN compiled to both targets
- THEN Postgres MUST get `confidence > $N` in the SQL
- AND petgraph MUST get a closure testing `edge.confidence > 0.5`

#### Scenario: Provenance filter compiles to both
- GIVEN `... WHERE provenance.lsp = "go_to_definition"`
- WHEN compiled to both targets
- THEN Postgres MUST get `provenance->>'source' = $N`
- AND petgraph MUST get a closure testing `edge.provenance.source == "go_to_definition"`

### Requirement: Boolean Composition Compilation

A boolean `MoldQLQuery::Boolean` MUST compile to a `CompiledQuery::Composed(Vec<CompiledQuery>, BooleanOp)`. The executor walks the vec in order and combines results according to `BooleanOp`. Each sub-query compiles independently. No cross-sub-query optimization is required (out of scope).

#### Scenario: AND compiles to two compiled plans
- GIVEN `PATH FROM "a" TO "b" AND NEIGHBORS "c" DEPTH 2`
- WHEN compiled to Postgres
- THEN the result MUST be `CompiledQuery::Composed(vec![left_pg, right_pg], BooleanOp::And)`
- AND `left_pg` MUST be a `PATH` SQL
- AND `right_pg` MUST be a `NEIGHBORS` SQL

#### Scenario: NOT compiles to a single inverted plan
- GIVEN `NOT SUBGRAPH ROOT "a" DEPTH 2`
- WHEN compiled to petgraph
- THEN the result MUST be `CompiledQuery::Composed(vec![inner], BooleanOp::Not)`

### Requirement: Test Parity

The Postgres and petgraph compilers MUST share a test suite that asserts they produce semantically equivalent plans for the same AST. The test MUST run against a fixture graph (at least 10 nodes, 15 edges, 3 distinct provenance sources, 5 confidence values) and assert both backends return the same node ids and the same edge counts for every ExplorerQL primitive + every filter combination.

#### Scenario: PATH parity
- GIVEN the fixture graph and `PATH FROM "x" TO "y"`
- WHEN compiled to both targets and executed
- THEN both MUST return the same `path: [..]` and `length`

#### Scenario: NEIGHBORS parity with WHERE parity
- GIVEN the fixture and `NEIGHBORS "x" DEPTH 2 WHERE confidence > 0.5`
- WHEN both backends execute
- THEN the returned node id sets MUST be equal (set equality, not list equality — order may differ)

#### Scenario: SUBGRAPH parity with provenance filter
- GIVEN the fixture and `SUBGRAPH ROOT "x" DEPTH 3 WHERE provenance.lsp = "go_to_definition"`
- WHEN both backends execute
- THEN the returned node and edge sets MUST be equal

## Edge Cases

| Case | Input | Expected |
|------|-------|----------|
| Empty WHERE | `CLUSTER` (no WHERE) | Empty `params` vector in Postgres; identity closure in petgraph |
| Subgraph with depth 0 | `SUBGRAPH ROOT "a" DEPTH 0` | Returns the root only (or `[]` — see TDD gate) |
| MAX_HOPS 0 on PATH | `PATH FROM "a" TO "b" MAX_HOPS 0` | Self-only check — returns `None` or empty path |
| Direction both with depth 1 | `NEIGHBORS "a" DEPTH 1 DIRECTION both` | Returns direct callers AND direct callees |
| CLUSTER with WHERE on a disconnected graph | `CLUSTER METHOD connected WHERE confidence > 0.9` | Returns components whose members all satisfy the filter |

## Out of Scope

- Cost-based choice between Postgres and petgraph at runtime (target is caller-specified)
- Query result caching / memoization
- Index hints (`/*+ IndexScan */`)
- Cross-target result merging
- SQL prepared statement caching
- Schema migration of `edges` / `symbols` tables
- Optimizer rewrites (e.g. pushing filters into the recursive CTE)

## TDD RED Gate

Before any implementation of `moldql/compile.rs`:

1. The `CompiledQuery`, `CompileTarget`, `PostgresPlan`, `PetgraphPlan`, and `BooleanOp` types MUST be defined and exported.
2. A `compile_fixtures.rs` test module MUST contain ≥ 20 tests:
   - 5 primitives × 2 targets = 10 (one per (primitive, target) pair)
   - 4 WHERE filters × 2 targets = 8 (provenance, confidence, both, none)
   - 2 boolean tests (AND, NOT) × 2 targets = 4 (overlap allowed; total ≥ 20)
3. A parity test (`compile_parity.rs`) MUST run the fixture graph through both backends and assert set equality for ≥ 8 query variants.
4. A static-analysis test MUST scan the output of every `compile(...)` call and assert no user-supplied string literal appears in the SQL body.

The RED gate fails if any compile test passes before the corresponding primitive's emit function is implemented, or if a parity test diverges between the two backends for an equivalent AST.
