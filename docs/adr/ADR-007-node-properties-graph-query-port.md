# ADR-007: node_properties() on GraphQueryPort

**Status**: ACCEPTED (promoted 2026-08-10)
**Date**: 2026-07-03  
**Deciders**: User, OpenCode orchestrator — design phase of e12f-ownership-map (SDDK)  

## Context

`OwnershipMapExecutor::build()` needs to read `graph_nodes.properties` JSONB — specifically `codeowners`, `last_author`, and `author_email` — to surface real ownership attribution instead of the `"ownership unavailable"` placeholder.

The executor already has `&ViewContext` which carries `graph_query: Option<&dyn GraphQueryPort>`. There is no existing port method that exposes node properties.

Three alternatives were evaluated in the e12f-ownership-map design (design.md, line 53–69):

| Option | Rejection reason |
|--------|-----------------|
| Separate `SymbolPropertiesPort` | Threads a 6th port through `ViewContext`, facades, MCP handler, runtime, and every mock — high blast radius for one method |
| Properties on `ResolvedSymbol` | Changes the identity-resolution contract; bloats every resolved symbol |
| "Properties as a neighbor query" | `neighbors` returns edges, not node attributes |

`GraphQueryPort` already carries structural metadata (`callers_with_metadata` carries provenance/confidence). `ViewContext.graph_query` is already wired from `GraphCache` in runtime/api/facades. Adding one method with a default `None` implementation is the minimal change.

## Decision

Add `fn node_properties(&self, id: &SymbolId) -> Option<HashMap<String, String>>` to **`GraphQueryPort`**.

Default implementation returns `None` (degraded mode). Production implementations — `CallGraphRepository` backed by PostgreSQL — return the `properties` JSONB column.

The method is consistent with ADR-010 Phase 4: identity methods stay on `SymbolRepository`; structural metadata (including node properties) lives on `GraphQueryPort`.

## Consequences

**Positive:**
- Minimal blast radius: one method, default `None`, only called where needed
- ISP-pure: callers that don't need properties don't call the method
- Consistent with existing port model (`GraphQueryPort` already carries structural metadata)
- No schema migration: `properties` JSONB column already exists

**Negative:**
- Trait method ships to all implementations and mocks — non-trivial to remove once adopted
- `GraphQueryPort` is named "query" but gains an attribute-read capability — mild conceptual surprise

**Mitigation for negative:** default implementation `None` ensures existing implementations degrade gracefully. If the method is later removed, implementations without an override will simply return `None`.

## Implementation note

The method signature is:

```rust
fn node_properties(&self, _id: &SymbolId) -> Option<HashMap<String, String>> { None }
```

PostgreSQL implementation in `PostgresRepository`:

```rust
fn node_properties(&self, id: &SymbolId) -> Option<HashMap<String, String>> {
    // SELECT properties FROM graph_nodes WHERE id = $1
}
```

## References

- Design: `sddk/e12f-ownership-map/design.md` (line 53–69, "Decision: Port for reading properties")
- Spec: `sddk/e12f-ownership-map/spec.md` (Requirement: OwnershipMapExecutor renders real ownership)
- ADR-010: SymbolRepository / GraphQueryPort separation
- `crates/cognicode-core/src/domain/traits/graph_query_port.rs`
- `crates/cognicode-explorer/src/domain/views.rs` (OwnershipMapExecutor)

## Implementation Log

- **2026-08-10 (E31-C)**: Node properties graph query port implemented. NodePropertiesGraphQueryPort lives in crates/cognicode-core/src/domain/ports/ and is consumed by graph_analytics services.
