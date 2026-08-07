# Proposal: E28.2 Runtime Closure — Production Executor Wiring & Differential Conformance

## Intent
E28.2 shipped the `GraphExecutor` port, `PgGraphExecutor`, and `SnapshotGraphExecutor`, but production MoldQL still routes through the **deprecated** `compile()`/`CompileTarget` (Postgres|Petgraph) path. The runtime injects `GraphQueryPort` but never a `GraphExecutor`; `StubExecutor` returns synthetic empty success (ADR-014 §4 forbids this); limits are checked post-walk; `PlanHash` is non-canonical; backend choice leaks to callers; REST/MCP return untyped results. This slice closes E28.2 by making executor contracts the sole production path.

## Scope

### In Scope
- `compile_to_plan` → `GraphExecutor` as the sole runtime graph path
- Runtime injection of `GraphExecutor` + `GraphQueryPort`
- Eliminate synthetic empty success; unsupported ops fail before execution
- Canonical, backend-neutral `PlanHash`
- Enforce `PlanLimits` *during* traversal (per-hop), not post-walk
- PG↔snapshot differential conformance (`assert_equivalent` + oracle)
- Retire user-visible `CompileTarget`/legacy compile behind a compatibility facade
- REST/MCP return typed `ResultSet`

### Out of Scope
- Pattern Profile v1 (E28.3); Analytics registry (E28.4)
- `calls`/`DependencyType` edge-filter (`e28-2-pr5-edge-filter`, shipped v0.71.0)
- Neo4j oracle; changes to the shipped `graph_plan.rs` edge-filter contract

## Capabilities
> Contract with sddk-spec.

### New Capabilities
- `graph-runtime-composition`: runtime assembly of executor registry, `GraphQueryPort`, and plan compiler into one production graph path

### Modified Capabilities
- `moldql`: `compile_to_plan` is the sole entry; legacy `compile()` demoted to compatibility facade
- `executor-equivalence-conformance`: PG↔snapshot `assert_equivalent` harness is normative
- `graph-executor-port`: executors receive injected limits; pin-fails-closed enforced at runtime
- `executor-semantics`: no executor returns synthetic empty success
- `plan-limits`: enforced during traversal via per-hop counters
- `unsupported-operation-errors`: unsupported constructs reject before executor dispatch

## Approach
`GraphExecutorRegistry` selects `PgGraphExecutor` (canonical) with `SnapshotGraphExecutor` as differential oracle. A `LegacyCompileAdapter` shims the deprecated `compile()` onto `compile_to_plan`, so callers compile unchanged until removal. Limits become per-hop counters checked inside each BFS expansion. `PlanHash` is recomputed over the normalized serializable plan so both backends derive identical hashes.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-runtime/src/lib.rs` | Modified | Inject executor registry into `ApiState`/MCP |
| `crates/cognicode-explorer/src/moldql/compile.rs` | Modified | Facade `compile()` → `compile_to_plan` |
| `crates/cognicode-core/src/domain/plan/executor.rs` | Modified | `StubExecutor` ceases empty success |
| `crates/cognicode-core/src/infrastructure/graph/snapshot_graph_executor.rs` | Modified | Per-hop limit enforcement |
| REST/MCP handlers | Modified | Typed `ResultSet` serialization |

## Entropy Budget
- **Target DQS**: ≥0.35 gain — eliminate connascence-of-name on `CompileTarget`.
- **Coupling removed**: `CoN(CompileTarget)` between explorer executor and runtime.
- **SOLID**: SRP (single production path), ISP (ports stay read-only), DIP (runtime depends on trait).

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Facade behavioral drift | Med | Conformance gate asserts facade == plan path |
| New production path regresses an incident workflow | Med | Keep an explicit, observable emergency rollback flag during stabilization |

## Rollback Plan
`compile_to_plan` plus `GraphExecutor` is the default and sole normal production
path. `LegacyCompileAdapter` remains available only behind an explicit,
observable emergency rollback flag during stabilization; it is disabled by
default and scheduled for removal after parity evidence is retained. No database
migration is required.

## Dependencies
- E28.2 PR1-PR3 (shipped v0.68–v0.70.1): port, PG executor, snapshot executor
- E28.2 PR4 Conformance (`assert_equivalent` harness; shipped v0.71.1)
- E29.1 (runtime/workspace refactor prerequisite)

## Success Criteria
- [ ] No production graph query routes through `compile()`/`CompileTarget`
- [ ] `StubExecutor` returns `Err(UnsupportedConstruct)` (no empty success)
- [ ] `assert_equivalent` passes for all golden fixtures across PG + snapshot
- [ ] `PlanHash` matches across backends for the same logical plan
- [ ] REST/MCP graph endpoints return typed `ResultSet`
- [ ] Limit breach during traversal yields typed error before completion
