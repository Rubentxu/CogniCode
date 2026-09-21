# Proposal: Versioned MoldPlan/GraphPlan Contracts (E28.1)

## Intent

E28.0 closed canonical graph revisions and pinned reads. But MoldQL still
compiles to **backend-specific** plans (`explorerql-compilation`:
`CompiledQuery::Postgres(sql)` / `PetgraphPlan`, caller-supplied
`CompileTarget`), so PostgreSQL and snapshot execution have **no shared
normative contract** and can diverge silently. E28.1 introduces the
backend-neutral, versioned `MoldPlan`/`GraphPlan` algebra, typed result
semantics, mandatory resource limits, and structured unsupported-operation
errors — the CONTRACT that E28.2 differential executors will conform to.

## Scope

### In Scope
- Versioned `MoldPlan` discriminated union (`Graph|ObjectSelection|Quality|Lens|ViewExecution`) + `GraphPlan` for graph-selecting ops — backend-neutral (no SQL/Petgraph/MCP/React types).
- Normative result semantics: typed values, missing properties, multiset identity, ordering, path node/edge sequence, errors, truncation, provenance, approximate numeric tolerance.
- Mandatory plan limits (time, cancellation, depth, visited nodes/edges, result rows, path count, memory) → typed error or explicit truncation on breach.
- Structured `UnsupportedConstruct` error (construct id + supported alternative) at parse/lower/plan-check — never an empty success.

### Out of Scope
- Executor implementations / differential parity fixtures → E28.2.
- Pattern Profile v1 grammar → E28.3.
- Analytics registry/modes → E28.4+.
- Deleting legacy `CompiledQuery`/`compile()` (bridged, deprecation tracked, removal deferred).

## Capabilities

> CONTRACT with sddk-spec. Research of `openspec/specs/` done.

### New Capabilities
- `moldplan-graphplan`: versioned, backend-neutral plan algebra + revision pinning; plans contain no backend/presentation types.
- `executor-semantics`: normative typed-value, multiset, ordering, path, error, truncation, provenance, numeric-tolerance rules both executors MUST satisfy.
- `plan-limits`: resource-governance contract — every plan/run declares applicable limits; breach ⇒ typed error or explicit truncation.
- `unsupported-operation-errors`: structured error emitted before execution; identifies construct + supported alternative.

### Modified Capabilities
- `explorerql-compilation`: compilation target changes from backend-specific `CompiledQuery`(`PostgresPlan`/`PetgraphPlan`, public caller-supplied `CompileTarget`) to the versioned, backend-neutral `MoldPlan`/`GraphPlan`. Backend-specific lowering becomes an **internal executor detail** (ADR-014 §3/§4). The legacy `compile()` entry point is bridged, not removed, in this slice.

## Approach

Introduce plan / value-object / limit / error types in
`cognicode-core::domain` (ADR-014 §3 assigns ownership to core; AST stays in
explorer). Graph-selecting `MoldQLQuery` variants lower to `GraphPlan` via a
new backend-neutral `compile_to_plan`; existing `compile()`→`CompiledQuery`
is bridged behind it, not deleted. All new types are pure value objects built
TDD-RED-first. **No executor wiring** (that is E28.2).

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/domain/` (new `plan.rs`/`moldql_plan.rs`, `value_objects/`) | New | `MoldPlan`, `GraphPlan`, `PlanVersion`, `PlanLimits`, result value objects, `UnsupportedConstruct` error enum |
| `crates/cognicode-core/src/domain/traits/graph_query_port.rs` | Modified | adjacent executor-port traits; `GraphQueryPort` signatures unchanged |
| `crates/cognicode-explorer/src/moldql/compile.rs` | Modified | bridge `compile()` → `MoldPlan`/`GraphPlan`; `CompiledQuery`/`CompileTarget` become executor-internal |
| `crates/cognicode-explorer/src/facades/moldql.rs` | Modified | `execute_query` declares limits; surfaces structured unsupported errors |
| `crates/cognicode-explorer/tests/` | New | contract tests: plan neutrality, limits, unsupported-construct |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `explorerql-compilation` callers break | Med | bridge, don't delete; deprecate `CompileTarget` behind new entry point |
| Under-specified semantics → E28.2 churn | Med | freeze semantics in this spec; golden contract types, not prose |
| AST(explorer) / plan(core) ownership split creates a cycle | Low | core depends on nothing in explorer; explorer lowers AST → core plan |

## Rollback Plan

Single `git revert` of the change branch. E28.1 is type-only: additive value
objects + a new entry point leave `explorerql-compilation`'s legacy `compile()`
intact. No DB migration, no schema change.

## Dependencies

- E28.0 (DONE): `RevisionId`, `WorkspaceId`, `SnapshotProvider`, pinned reads (`UnknownRevision`).
- ADR-014 §3/§4/§7; `docs/specs/graph-query-execution.md` ("Typed planning and revision pinning", "Unsupported constructs fail before execution", "Resource governance", "Normative executor equivalence").

## Success Criteria

- [ ] Every `MoldPlan`/`GraphPlan` is versioned and contains no SQL/Petgraph/MCP/React types (static assertion test).
- [ ] Every plan/run declares applicable limits; a breach yields a typed error or `truncated=true`, never silent degradation.
- [ ] Unsupported syntax fails before execution with a structured error naming construct + supported alternative.
- [ ] Normative result semantics (typed values, multiset identity, ordering, paths, provenance, numeric tolerance) captured as conformance contract types for E28.2.
