# Spec: postgres-call-edges (OBSOLETE — 2026-08-04)

> **Status: OBSOLETE** — PostgreSQL removed (ADR-026, e29-7).
> Archive this spec.

## Purpose

PostgreSQL persistence and read-path for the canonical `call_edges` table
in `cognicode-core`. Closes the parity gap with the SQLite backend, which
already stores edges with full `(Provenance, confidence)` metadata.
This domain provides:

- A column-for-column parity `call_edges` table (SQLite v2 schema).
- An `EdgeMetadata` value object carrying caller, callee, dependency
  type, provenance, and confidence.
- Three async read methods on the `Repository` trait:
  `find_edges_by_caller`, `find_edges_by_callee`, `count_edges`.
- A `PostgresRepository` implementation behind the `postgres` feature
  flag.
- A `pub(crate)` write helper (`insert_edge`) for test seeding only.

Out of scope: full `save_call_graph(&CallGraph)` write path, `GraphStore`
impl for PostgreSQL, explorer/MCP/petgraph integration, ltree/pgvector,
batch insert, upsert.

## Requirements

### Requirement: Canonical `call_edges` schema

`call_edges` MUST have columns
`id, caller_id, caller_name, callee_id, callee_name, dependency_type,
provenance, confidence`. The migration MUST be idempotent (all DDL
guarded by `IF NOT EXISTS`).

### Requirement: `EdgeMetadata` value object

`EdgeMetadata` MUST be a `Clone + Debug + PartialEq` struct with 7
fields. Lives in `cognicode_core::domain::value_objects`; no `Serialize`
required in this slice.

### Requirement: Edge queries on the `Repository` trait

The async `Repository` trait MUST expose `find_edges_by_caller`,
`find_edges_by_callee`, and `count_edges`. Implementations MUST return
empty `Vec` / `0` for empty results, never `Err(NotFound)`.

### Requirement: `PostgresRepository` edge implementation

`PostgresRepository` MUST implement the three edge methods using
`sqlx::query_as` with a private `EdgeRow` deriving `sqlx::FromRow`. All
code is `#[cfg(feature = "postgres")]`-gated. Unparseable `provenance`
or `dependency_type` strings MUST default to `Provenance::Extracted` /
`DependencyType::Calls` rather than error the whole call.

### Requirement: `insert_edge()` test-seeding helper

`PostgresRepository::insert_edge(&self, edge: &EdgeMetadata)` MUST be a
`pub(crate)` inherent method (NOT on the `Repository` trait) for test
seeding. No batch / bulk / upsert variant in this domain.

### Requirement: Feature flag and non-breaking default build

All PostgreSQL code MUST be `#[cfg(feature = "postgres")]`-gated. The
`EdgeMetadata` struct and the `Repository` trait extension MUST be
unguarded (pure value object + port). The `postgres` feature MUST NOT
be required by default. Adding this domain MUST NOT change any
pre-slice public API of `cognicode-core` other than additive trait
methods and the new value object re-export.

### Requirement: Testability — per-test isolation

Integration tests MUST use the existing `pg_test!` macro for
per-test isolated databases. Test cases MUST cover caller lookup,
callee lookup, empty-result semantics, count, and insert round-trip.

### Requirement: Rollout safety

Migrations MUST be idempotent on populated databases. Re-running
`run_migrations()` MUST preserve every row. Reverting this domain's
changes MUST restore the workspace to a green build with no schema
state to unwind.

---

> **Reconciliation note (2026-08-01)**: the `save_call_graph` /
> `load_call_graph` inherent methods on `PostgresRepository` referenced in
> this spec are the **pre-Phase-0 surface**. The e29-0-define-new-ports +
> e29-0-refactor-call-sites changes relocated them behind the
> `CallGraphStore` domain port (with the `_ws` suffix):
>
> - `PostgresRepository::save_call_graph(&self, graph)` →
>   `CallGraphStore::save_call_graph_ws(&self, graph, ws)`
> - `PostgresRepository::load_call_graph(&self)` →
>   `CallGraphStore::load_call_graph_ws(&self, ws, rev)` or
>   `CallGraphStore::load_call_graph_current(&self, ws)`
>
> The **contract** (workspace-scoped, atomic per revision, idempotent
> re-save) is unchanged — only the port path changed. The pre-Phase-0
> `PostgresRepository` inherent method names remain in the concrete adapter
> as pass-through delegates to the new port.
