# moldplan-graphplan Specification (NEW)

## Purpose

The versioned, backend-neutral plan algebra that every MoldQL operation
lowers to before execution. `MoldPlan` is a discriminated union; only
graph-selecting variants lower further to `GraphPlan`. The plan is
revision-pinned and contains no SQL, Petgraph, MCP, or React types
(ADR-014 §3). The legacy `CompiledQuery` / `CompileTarget` stay alive
behind a bridge but are no longer the canonical compilation target.

## Requirements

### Requirement: MoldPlan Discriminated Union

`MoldPlan` is a Rust enum with one variant per non-composite operation
(`Graph(GraphPlan)`, `ObjectSelection`, `Quality`, `Lens`, `ViewExecution`).
Each variant MUST carry a `PlanVersion` and (when applicable) the
`WorkspaceId`+`RevisionId` the plan is pinned to. The type MUST derive
`Debug, Clone, PartialEq, Serialize, Deserialize`.

#### Scenario: Graph variant carries its sub-plan

- GIVEN a `GraphPlan::ShortestPath { from, to, max_hops }`
- WHEN wrapped in `MoldPlan::Graph(…)`
- THEN `match mold_plan { MoldPlan::Graph(g) => g, _ => unreachable!() }` recovers the inner plan

#### Scenario: All variants serialize round-trip

- GIVEN one `MoldPlan` per variant
- WHEN each is serialized to JSON and deserialized back
- THEN the original variant and payload are recovered

### Requirement: GraphPlan Bounded Traversal

`GraphPlan` MUST support `ShortestPath`, `Neighbors`, `Subgraph`,
`Cluster`, `Explain`, and a `BooleanComposition` wrapper. Each variant
carries bounded quantifiers (`max_hops`, `depth`) and a `PlanFilter`
for predicates. Unbounded quantifiers MUST NOT be representable.

#### Scenario: Cannot construct an unbounded path

- GIVEN `GraphPlan::ShortestPath` and `GraphPlan::Neighbors` constructors
- WHEN called without a `max_hops` / `depth` argument
- THEN the constructor returns `Err(GraphPlanError::MissingBound)`

#### Scenario: Boolean composition wraps sub-plans

- GIVEN `GraphPlan::ShortestPath` and `GraphPlan::Neighbors`
- WHEN combined under `BooleanComposition { op: And, operands: [a, b] }`
- THEN the wrapper preserves both sub-plans and the `BooleanOp`

### Requirement: Backend-Neutrality

A `MoldPlan` or `GraphPlan` MUST NOT contain any field whose type comes
from `sqlx`, `petgraph`, `serde_json::Value` (used as a query shape),
`tokio`, `mcp`, or the renderer crates. A static-assertion test MUST
fail compilation if any backend type leaks in.

#### Scenario: Static neutrality assertion

- GIVEN the `MoldPlan` and `GraphPlan` types
- WHEN `cargo build -p cognicode-core` runs
- THEN the build fails if any variant field re-exports a banned backend type

#### Scenario: Plan is Send + Sync + 'static

- GIVEN a `MoldPlan::Graph(GraphPlan::ShortestPath { ... })`
- WHEN the compiler checks trait bounds
- THEN the value is `Send + Sync + 'static`

### Requirement: PlanVersion and Hash

Every `MoldPlan` carries a `PlanVersion` (semver string) and a
`PlanHash` (deterministic SHA-256 over the canonical JSON). `PlanHash`
MUST be stable for byte-identical inputs and MUST change when any
field changes.

#### Scenario: Hash stability

- GIVEN two `MoldPlan` instances built from the same AST and limits
- WHEN each is hashed
- THEN the two `PlanHash` values are equal

#### Scenario: Hash sensitivity

- GIVEN a `MoldPlan` whose `max_hops` is changed from `3` to `4`
- WHEN hashed again
- THEN the new `PlanHash` differs from the previous one

### Requirement: Revision Pinning

Every `MoldPlan` whose variant reads graph state MUST carry a
`(WorkspaceId, RevisionId)` pair. The pair is set at lowering time and
MUST be immutable for the plan's lifetime.

#### Scenario: Pinned plan survives concurrent ingest

- GIVEN a plan pinned to `(ws1, rev=3)`
- WHEN a concurrent ingest advances head to `rev=4`
- THEN the pinned plan still references `rev=3` and produces results for that revision

#### Scenario: Constructing a plan without a pin is rejected

- GIVEN `MoldPlan::Graph(GraphPlan::ShortestPath { ... })` constructed
- WHEN `with_pin(ws, rev)` is not called
- THEN the resulting plan is `Err(PlanError::UnpinnedGraphPlan)`

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Plan with `max_hops: 0` | Permitted; executor stops at root |
| Two plans identical except `PlanVersion` | Different `PlanHash` |
| Plan constructed from unit struct (no fields) | Hashes deterministically; pins accepted |

## Out of Scope

- Backend executors (Postgres, Petgraph) — E28.2
- Plan execution wiring — E28.2
- Pattern Profile v1 grammar — E28.3
- Removing legacy `CompiledQuery` — bridged and deprecation-tracked

## Dependencies

- `RevisionId`, `WorkspaceId` (E28.0)
- `GraphNode`, `GraphEdge`, `NodeKind`, `EdgeKind` (generic-graph-model)
- `GraphQueryPort` (in CONTEXT.md)
- ADR-014 §3, §4, §7
