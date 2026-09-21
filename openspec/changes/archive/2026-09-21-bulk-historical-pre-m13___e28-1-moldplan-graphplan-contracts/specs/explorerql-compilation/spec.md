# Delta for explorerql-compilation

> Backend-neutral compilation target. The legacy `compile()` →
> `CompiledQuery` / `CompileTarget` surface stays alive as a bridge;
> the canonical target for new code is the versioned `MoldPlan` /
> `GraphPlan` (ADR-014 §3 / §4).

## MODIFIED Requirements

### Requirement: Compilation Entry Point

A new public function `compile_to_plan(query: &MoldQLQuery, limits: &PlanLimits, pin: (WorkspaceId, RevisionId)) -> Result<MoldPlan, MoldError>` MUST exist in `moldql/compile.rs`. The legacy `compile(query, target: CompileTarget) -> Result<CompiledQuery, CompileError>` entry point MUST remain for backwards compatibility and MUST bridge to `compile_to_plan` internally.
(Previously: `compile` produced `CompiledQuery::Postgres(String)` or `CompiledQuery::Petgraph(PetgraphPlan)` keyed by a caller-supplied `CompileTarget` — backend specifics were in the public type.)

#### Scenario: compile_to_plan returns a versioned MoldPlan

- GIVEN a parsed `PathQuery { from: "a", to: "b", max_hops: Some(3) }`
- WHEN `compile_to_plan(&q, &PlanLimits::default(), (ws1, rev=5))` is called
- THEN the result is `Ok(MoldPlan::Graph(GraphPlan::ShortestPath { …, max_hops: Some(3), filters: [] }))`
- AND the returned `MoldPlan` carries `PlanVersion` and `PlanHash`

#### Scenario: compile_to_plan pins workspace + revision

- GIVEN any parsed query
- WHEN `compile_to_plan` is called with `(ws1, rev=5)`
- THEN the resulting `MoldPlan.pin → (ws1, rev=5)` — no later mutation possible

#### Scenario: Legacy compile bridges to compile_to_plan

- GIVEN the legacy `compile(&q, CompileTarget::Petgraph)` entry point
- WHEN called with the same `PathQuery`
- THEN it delegates to `compile_to_plan` internally
- AND the output is wrapped back into `CompiledQuery::Petgraph(…)` for the legacy caller

#### Scenario: Determinism

- GIVEN a fixed query, limits, and pin
- WHEN `compile_to_plan` is called twice
- THEN both results are equal (same `PlanVersion`, same `PlanHash`, deep `PartialEq`)

#### Scenario: Unknown AST variant rejected

- GIVEN a future `MoldQLQuery` variant the compiler does not know
- WHEN `compile_to_plan` is called
- THEN the result is `Err(MoldError::UnsupportedConstruct { construct: Other("…"), … })`

### Requirement: Plan-Level Compilation (PostgreSQL & Petgraph lowering as executor-internal)

The `MoldPlan`/`GraphPlan` produced by `compile_to_plan` is the canonical
compilation target — it carries no SQL, no Petgraph tokens, no MCP, and
no React types. Backend-specific lowering (`PostgresPlan` and
`PetgraphPlan`) becomes an EXECUTOR concern. The compiler MUST NOT
emit `CompiledQuery::Postgres(String)` or `CompiledQuery::Petgraph(…)`
in the new entry point.
(Previously: the compiler produced `CompiledQuery::Postgres(String)` containing a hand-emitted SQL string and `CompiledQuery::Petgraph(PetgraphPlan)` typed by backend.)

#### Scenario: GraphPlan is backend-neutral

- GIVEN a `MoldPlan::Graph(GraphPlan::ShortestPath { from, to, max_hops, filters })`
- WHEN the compiler inspects the variant
- THEN the payload contains no `String` SQL, no `petgraph::Graph`, no `tokio` futures, no `serde_json::Value` query shapes

#### Scenario: PG SQL safety is preserved at the executor boundary

- GIVEN a `GraphPlan::ShortestPath` with `filters: [Filter::Confidence { op: Gt, value: 0.5 }]`
- WHEN the executor lowers it to SQL
- THEN the SQL contains `confidence > $N` (not `confidence > 0.5` inline)
- AND `0.5` is a bound parameter, not a string concatenation

#### Scenario: Bound string values are never inlined into SQL

- GIVEN a `GraphPlan::ShortestPath { from: "alpha' OR 1=1; --", … }`
- WHEN the executor lowers it to SQL
- THEN the SQL string MUST NOT contain the user's literal value verbatim

#### Scenario: NEIGHBORS subgraph lowering preserves both-direction semantics

- GIVEN `GraphPlan::Neighbors { root: "a", depth: 2, direction: Both, … }`
- WHEN the executor lowers the plan
- THEN the relational algebra (or graph walk) covers incoming AND outgoing edges
- AND the typed output reports results from both sides

### Requirement: Filter Encoding on the Plan

WHERE filters lower to `PlanFilter` values on the `GraphPlan` (not
SQL strings). The executor lowers each `PlanFilter` to a SQL predicate
AND to a petgraph post-filter closure. The two lowerings MUST be
semantically equivalent given the same source graph.
(Previously: WHERE compiled directly to a SQL string in the PG branch and a closure in the Petgraph branch; the two branches were not derived from a shared representation.)

#### Scenario: Confidence filter lowers to a typed PlanFilter

- GIVEN a query with `WHERE confidence > 0.5`
- WHEN `compile_to_plan` is called
- THEN the resulting `GraphPlan.shortest_path.filters` contains `PlanFilter::Confidence { op: Gt, value: 0.5 }`

#### Scenario: Provenance filter lowers to a typed PlanFilter

- GIVEN a query with `WHERE provenance.lsp = "go_to_definition"`
- WHEN `compile_to_plan` is called
- THEN the resulting `GraphPlan.shortest_path.filters` contains `PlanFilter::Provenance { key: "lsp", value: "go_to_definition" }`

#### Scenario: Filter equivalence PG vs petgraph

- GIVEN a `GraphPlan` with `PlanFilter::Confidence { op: Gt, value: 0.5 }`
- WHEN both executors run against the same fixture graph
- THEN the returned node sets are equal (set equality, not list equality)

### Requirement: Boolean Composition Compilation

A boolean `MoldQLQuery::Boolean` MUST compile to a
`MoldPlan::Graph(GraphPlan::BooleanComposition { op, operands })`. Each
operand recurses through `compile_to_plan` first; no cross-operand
optimization is required (out of scope). The legacy `CompiledQuery::Composed`
remains a bridge shim.
(Previously: `CompiledQuery::Composed(Vec<CompiledQuery>, BooleanOp)` carried arbitrary backend targets and the executor walked each child.)

#### Scenario: AND composes sub-plans

- GIVEN `MoldQLQuery::Boolean { op: And, operands: [path(a, b), neighbors(c, 2)] }`
- WHEN `compile_to_plan` is called
- THEN the result is `MoldPlan::Graph(GraphPlan::BooleanComposition { op: And, operands: [<plan a>, <plan c>] })`

#### Scenario: NOT composes with a single operand

- GIVEN `MoldQLQuery::Boolean { op: Not, operands: [subgraph(a, 2)] }`
- WHEN `compile_to_plan` is called
- THEN the result is `MoldPlan::Graph(GraphPlan::BooleanComposition { op: Not, operands: [<plan a>] })`

#### Scenario: Recursive operands preserve the plan algebra

- GIVEN a boolean whose operands are themselves booleans
- WHEN `compile_to_plan` is called
- THEN the result is a 3-level `BooleanComposition` — no executor tree node

### Requirement: Test Parity

The Postgres and petgraph executors MUST share a test suite that
asserts they produce semantically equivalent `ResultSet`s for the same
`MoldPlan` / `GraphPlan` against the same `(workspace, revision)` pair.
The shared fixture graph is at least 10 nodes, 15 edges, with 3 distinct
provenance sources and 5 confidence values. Parity is asserted over
ordered paths (exact sequence) and unordered node sets (multiset).
(Previously: parity was asserted on the legacy `CompiledQuery` shape; suite covered 5 primitives × 2 targets + 8 filter combinations + 4 boolean cases.)

#### Scenario: PATH parity

- GIVEN the fixture graph and a `GraphPlan::ShortestPath { from: "x", to: "y", max_hops: Some(3) }`
- WHEN both executors run
- THEN the returned `Path` objects have identical node and edge sequence

#### Scenario: NEIGHBORS parity with WHERE

- GIVEN the fixture and a `GraphPlan::Neighbors { root: "x", depth: 2, direction: Both, filters: [Confidence { op: Gt, value: 0.5 }] }`
- WHEN both executors run
- THEN the returned node id sets are equal as multisets

#### Scenario: SUBGRAPH parity with provenance filter

- GIVEN the fixture and a `GraphPlan::Subgraph { root: "x", depth: 3, filters: [Provenance { key: "lsp", value: "go_to_definition" }] }`
- WHEN both executors run
- THEN the returned node and edge subsets are equal as multisets

## ADDED Requirements

### Requirement: compile_to_plan populates PlanLimits

`compile_to_plan` MUST populate `PlanLimits` for the returned `MoldPlan`:
`GraphPlan::Subgraph` and `GraphPlan::Neighbors` carry `max_depth`;
`GraphPlan::ShortestPath` carries `max_hops` as a `MaxHops` limit.
When the source query omits a bound, the compiler defaults to a safe
value (`max_depth = 5`, `max_hops = 6`) and the resulting plan
emits no `PlanError::MissingLimit`.

#### Scenario: Subgraph without depth defaults to 5

- GIVEN a `SubgraphQuery` with `depth: 0` (caller-supplied)
- WHEN `compile_to_plan` is called
- THEN the produced `GraphPlan::Subgraph` has `max_depth = Some(5)` and `original_depth = 0`

#### Scenario: ShortestPath without max_hops defaults to 6

- GIVEN a `PathQuery` with `max_hops: None`
- WHEN `compile_to_plan` is called
- THEN the produced `GraphPlan::ShortestPath` has `max_hops = Some(6)`

### Requirement: Bridge entry point is deprecated

The legacy `compile(query, target)` returns `#[deprecated(note = "use compile_to_plan for new code")]` on the function and the `CompileTarget` enum. The deprecation is soft: warnings are emitted, but the API is not removed in this slice.

#### Scenario: Deprecation warning fires

- GIVEN a caller invoking `compile(&q, CompileTarget::Postgres)`
- WHEN the compiler runs
- THEN a `deprecated` warning is emitted for both `compile` and `CompileTarget`
- AND the call still produces a valid `CompiledQuery`

## REMOVED Requirements

None. The legacy `compile()`, `CompileTarget`, `CompiledQuery`,
`PostgresPlan`, `PetgraphPlan`, and `BooleanOp` types remain so the
existing call sites and `compile_fixtures.rs` test suite stay green.

## Out of Scope (locked)

- Removing the legacy `compile()` / `CompileTarget` (deferred)
- E28.2 differential executors (Postgres, Petgraph) — owned by E28.2
- Pattern Profile v1 grammar — E28.3
- Plan execution wiring — E28.2
- Optimizer rewrites (e.g. pushing filters into the recursive CTE)
- Cross-target result merging
- Index hints / SQL prepared statement caching
- Schema migration of `edges` / `symbols` / `graph_nodes` / `graph_edges`

## Cross-cutting

- **Strict TDD**: every scenario translates to a single `#[test]`
  fn. PG-required scenarios are tagged `#[pg]` in the implementation
  harness so the integration test split is unambiguous.
- **PG-required scenarios**: the "PG SQL safety is preserved at the
  executor boundary", "Bound string values are never inlined into SQL",
  "Filter equivalence PG vs petgraph", and the 3 parity scenarios all
  require a real PostgreSQL + fixture graph; the rest are unit tests.
- **Backward compatibility**: every existing test in
  `compile.rs::tests` and `compile_fixtures.rs` must continue to pass
  after the change. The bridge guarantees this.
