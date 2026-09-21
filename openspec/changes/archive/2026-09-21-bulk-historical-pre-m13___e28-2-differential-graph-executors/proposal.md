# Proposal: E28.2 — Differential Graph Executors

> Change: `e28-2-differential-graph-executors` · Branch: `feat/e28-2-differential-graph-executors` · Mode: A-lite · Strict TDD: ACTIVE

## Intent
E28.1 froze the plan algebra (`MoldPlan`/`GraphPlan`), executor value-objects (`TypedValue`, `ResultSet`, `Path`, `ExecutionError`, `TruncationMarker`), and `PlanLimits`, explicitly deferring "concrete executor implementations" and "conformance fixtures" to E28.2. This change closes that gap: execute every existing `GraphPlan` variant (`PATH`, `NEIGHBORS`, `SUBGRAPH`, `CLUSTER`, `EXPLAIN`) and `BooleanComposition` in BOTH the PostgreSQL and the in-memory snapshot backends, and prove them equivalent via golden fixtures. Graph primitives are currently stubbed or semantically divergent; E28.2 makes the two executors interchangeable.

## Scope

### In Scope
- `Executor` port trait + internal backend-selection registry
- PG executor over `PostgresRepository::load_call_graph_ws`
- Snapshot executor over E28.0 `SnapshotProvider`
- Golden-fixture differential harness; petgraph as reference oracle
- Boolean composition (`AND`/`OR`/`NOT`) typed-multiset semantics
- Resource-governance enforcement (time, depth, rows, path-count, truncation)

### Out of Scope
- Pattern Profile v1 grammar (E28.3)
- Graph analytics admission/descriptors (E28.4+)
- Neo4j CI oracle wiring (E28.5)
- Explorer/MCP UI exposure — foundation slice, no user-facing clause (ADR-014 §9)

## Capabilities

> CONTRACT with sddk-spec. E28.1 specs (`executor-semantics`, `moldplan-graphplan`, `plan-limits`, `unsupported-operation-errors`) are consumed UNCHANGED.

### New Capabilities
- `graph-executor-port`: backend-neutral `Executor` trait — `async fn execute(&self, plan: &GraphPlan, limits: &PlanLimits) -> Result<ResultSet, ExecutionError>` — plus internal selection policy.
- `pg-graph-executor`: PostgreSQL executor over `PostgresRepository`; maps graph rows → typed `ResultSet`/`Path`/`ProvenanceSource`.
- `snapshot-graph-executor`: in-memory executor over `SnapshotProvider`; identical `ResultSet` contract.
- `executor-equivalence-conformance`: golden fixtures + `assert_equivalent`/`assert_approx_equal` differential suite; petgraph parity backend as reference oracle.

### Modified Capabilities
None — all E28.1 value-object specs are stable contracts consumed as-is.

## Approach
Both executors lower the SAME pinned `GraphPlan` (one workspace + one immutable `RevisionId`) to their native traversal and materialize `executor-semantics` value objects. PG uses recursive SQL for `PATH`/`SUBGRAPH`; the snapshot walks the petgraph held by `SnapshotProvider`. Backend choice is runtime policy, never surfaced in results. The conformance harness seeds deterministic fixture graphs, runs both executors on identical plan+revision, and compares typed multisets (unordered) and path sequences (ordered) — a mismatch is a parity violation, never promoted as normative.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/domain/plan/` | Modified | `Executor` port + result wiring (E28.1 plan types consumed) |
| `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs` | Modified | PG executor over `load_call_graph_ws` |
| `crates/cognicode-core/src/infrastructure/graph/snapshot_provider.rs` | Modified | snapshot executor over `SnapshotProvider` |
| `crates/cognicode-explorer/tests/e28_1_pg_conformance.rs` | Modified | reuse as golden-fixture base |
| `crates/cognicode-explorer/src/moldql/lower_plan.rs` | Modified | `MoldqlAstLowerer` feeds executors |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| SQL vs petgraph path-order divergence | High | ordered-path fixtures + `Path` sequence assertion |
| Truncation semantics differ per backend | Med | `TruncationMarker` parity tests; first-breach-wins rule |
| Approx numeric tolerance gaps | Low | `assert_approx_equal` reserved; no analytics this slice |

## Rollback Plan
Pure additive: no E28.1 spec/plan-type changes, no schema migration, no trait break. Revert with a single `git revert` of the feature-branch merge. Executors are unreached until `cognicode-runtime` wires selection (deferred). No canonical data is affected.

## Dependencies
- E28.0 `SnapshotProvider` + `VersionedGraphCache` (DONE)
- E28.1 `MoldPlan`/`GraphPlan`/`PlanLimits`/`executor-semantics` (DONE)
- PostgreSQL test instance (existing `postgres` feature + `#[sqlx::test]`)

## Success Criteria
- [ ] Every `GraphPlan` variant executes in both backends
- [ ] Golden fixtures prove equivalent typed multisets, ordering, paths, errors, provenance, truncation
- [ ] No supported operation returns synthetic empty success
- [ ] `cargo test --features postgres` passes; default build stays sqlx-free
