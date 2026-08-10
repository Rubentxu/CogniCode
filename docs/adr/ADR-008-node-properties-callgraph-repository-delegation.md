# ADR-008: node_properties() — CallGraphRepository delegates to PostgresRepository

**Status**: ACCEPTED (promoted 2026-08-10)
**Date**: 2026-07-03  
**Deciders**: User, OpenCode orchestrator — PR 1 + PR 2 merge review for e12f-ownership-map  

## Context

PR 1 of e12f-ownership-map added `node_properties()` to `GraphQueryPort` with a default `None` implementation. The production implementation in `PostgresRepository` is also in PR 1. However, `OwnershipMapExecutor::build()` calls `ctx.graph_query.node_properties()` — and `ctx.graph_query` is a `CallGraphRepository` backed by an in-memory `CallGraph`, NOT `PostgresRepository`.

The `CallGraph` (in-memory) does not store `properties` JSONB — those live in PostgreSQL. So `CallGraphRepository::node_properties()` will always return `None`, and the executor will always fall back to the `"ownership unavailable"` placeholder, even after PR 1+2 land.

Three options were evaluated:

| Option | Tradeoff | Decision |
|--------|----------|----------|
| `CallGraphRepository` queries `PostgresRepository` at runtime | Tight coupling between repos; runtime dependency; but only for `ownership` feature | **Chosen** |
| Separate `node_properties` on service/facade passed to executor | Breaks ISP (executor reaches into service); significant refactor | Rejected |
| `CallGraphRepository` wraps `PostgresRepository` (DI) | Requires changing `CallGraph` construction across runtime; large blast radius | Rejected |
| `OwnershipMapExecutor` takes `&dyn PostgresRepository` directly | Not consistent with `ViewContext` design; different port | Rejected |

## Decision

`CallGraphRepository` will delegate `node_properties()` to a new `&dyn PostgresRepository` reference held internally (added behind the `ownership` feature gate).

Specifically:

```rust
// CallGraphRepository fields (ownership feature)
struct CallGraphRepository {
    graph: Arc<CallGraph>,
    pg_repo: Option<Arc<PostgresRepository>>, // None when feature OFF
}

// GraphQueryPort impl
#[cfg(feature = "ownership")]
fn node_properties(&self, id: &SymbolId) -> Option<HashMap<String, String>> {
    self.pg_repo.as_ref()?.node_properties(id)
}
```

When `ownership` is OFF: `pg_repo = None`, `node_properties` returns `None` (degraded, same as today).  
When `ownership` is ON: `pg_repo = Some(...)`, `node_properties` delegates to PostgreSQL.

The `CallGraphRepository` is constructed in `GraphCache::new()` or equivalent runtime bootstrap. The `PostgresRepository` instance is available at bootstrap time. Adding the reference there is a localized change.

## Consequences

**Positive:**
- `OwnershipMapExecutor` works as designed with no further changes
- Feature-gated: non-ownership code paths have no overhead
- Minimal blast radius: `CallGraphRepository` only, one method, behind `ownership` feature
- `PostgresRepository` implementation (already in PR 1) is reused without change

**Negative:**
- `CallGraphRepository` gains a runtime reference to `PostgresRepository` — architectural coupling
- `GraphCache` / runtime bootstrap needs to pass the `PostgresRepository` reference at construction time

**Mitigation**: The coupling is feature-gated. Without `ownership`, `pg_repo = None` and the architecture is unchanged. Once the feature ships broadly, a future ADR can consider a cleaner long-term property-access design.

## Implementation Note

The `CallGraphRepository` is constructed in `cognicode-explorer/src/infrastructure/graph_cache.rs` (or equivalent bootstrap). The `PostgresRepository` instance used by `GraphCache` already exists — it needs to be passed to `CallGraphRepository::new()` behind the `ownership` feature gate.

## References

- ADR-007: `node_properties()` on `GraphQueryPort` (trait definition)
- PR 105: `feat/ownership-attribution` — ownership feature, `node_properties` trait + PG impl (git CLI blame via `std::process::Command`, no `gix` dep)
- PR 106: `feat/ownership-ingest` — CODEOWNERS parser + blame enricher + wiring
- `crates/cognicode-explorer/src/infrastructure/graph_cache.rs` — `CallGraphRepository` construction site

## Implementation Log

- **2026-08-10 (E31-C)**: CallGraphRepository delegation in place. Domain services depend on the CallGraphRepositoryPort trait, not the concrete infrastructure::graph::CallGraphProjection (per ADR-029).
