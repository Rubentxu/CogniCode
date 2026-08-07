# Proposal: Minimal PostgreSQL Repository

## Intent

The async `Repository` trait in `cognicode-core` has zero real implementations — only test stubs (`EmptyRepo`, `CountingRepo`). Phases 1 and 2 (archived, verified) delivered the `Symbol` aggregate, `Provenance` edge metadata, and the `Repository` trait seam. This slice provides the **first PostgreSQL-backed implementation** of that trait, establishing the connection pool, schema, and migration infrastructure that every future PostgreSQL slice depends on. Without this, the `Repository` trait is a dead seam.

## Why Now

- **Dependency chain clear**: Phases 1 (`explorer-graph-foundation`) and 2 (`explorer-graph-repository-bridge`) are archived and verified. This is the next unblocked slice.
- **No other work depends on us**: This slice blocks PostgreSQL `call_edges`, PostgreSQL `GraphStore`, explorer bridge, and MCP envelope — all Phase 3+. Landing it now unblocks the entire backend pipeline.
- **Pattern establishment**: The `sqlx` connection, migration, feature-flag, and test patterns are established once here and reused everywhere.

## Scope

### In Scope
- Add `sqlx` to workspace and `cognicode-core` (postgres + runtime-tokio features)
- Add `postgres` feature flag to `cognicode-core/Cargo.toml`
- Create `schema_postgres.sql` with a `symbols` table mirroring the SQLite schema (column-compatible for interoperability)
- Create `PostgresRepository` struct (`PgPool`, constructor, `run_migrations()`)
- Implement `Repository` trait (2 methods: `find_symbol_by_qualified_name`, `count_symbols`)
- Add in-crate integration tests using `sqlx::test` with a real PostgreSQL connection
- Feature-gate all PostgreSQL code behind `#[cfg(feature = "postgres")]`

### Out of Scope
- `call_edges` table or any edge queries
- PostgreSQL `GraphStore` implementation (blob-level persistence)
- `SymbolRepository` / `MetadataAwareRepository` bridge to explorer
- `ltree`, `pgvector`, or any PostgreSQL extensions
- New query methods on the `Repository` trait
- CI pipeline changes (flag the need, don't implement)
- Removal of `cognicode-store-traits` crate
- `#[serde]` on `PostgresRepository` — not a domain type

## Capabilities

### New Capabilities
- `postgres-symbol-repository`: PostgreSQL-backed `Repository` trait implementation with `symbols` table schema and `sqlx`-based async queries

### Modified Capabilities
- None — pure extension. The `Repository` trait contract is unchanged. Zero existing code modified.

## Approach

### Migration strategy: raw SQL via `include_str!`

**Chosen: raw SQL files embedded at compile time.** Rationale:
- One table (`symbols`), ~25 lines of DDL. A migration framework (`refinery`, `sqlx-cli`) adds a dependency for no benefit at this scale.
- `sqlx::query!()` uses compile-time checked SQL from the same files — no duplication.
- Commitment: switch to `refinery` when table count exceeds 3 (tracked in `schema_postgres.sql` header comment).

### Feature flag: `postgres` on `cognicode-core`

- `sqlx` adds ~30s compile time. Default build (no `postgres` feature) stays sqlx-free.
- Integration tests gated behind `#[cfg(feature = "postgres")]`. `sqlx::test` provides per-test isolated databases.
- No mutual exclusion with `persistence` feature — both are additive and can coexist.

### Schema design: parity with SQLite `symbols`

| Column | PostgreSQL type | SQLite equivalent | Notes |
|--------|----------------|-------------------|-------|
| `id` | `SERIAL PRIMARY KEY` | `INTEGER PRIMARY KEY AUTOINCREMENT` | Standard PG idiom |
| `file_path` | `TEXT NOT NULL` | `TEXT NOT NULL` | Identical |
| `name` | `TEXT NOT NULL` | `TEXT NOT NULL` | Identical |
| `kind` | `TEXT` | `TEXT` | Nullable (same as SQLite) |
| `line` | `INTEGER` | `INTEGER` | Nullable |
| `column` | `INTEGER` | `INTEGER` | Nullable |
| `complexity` | `INTEGER` | `INTEGER` | Nullable |

Index: `CREATE INDEX idx_pg_symbols_name ON symbols(name);`
Index: `CREATE INDEX idx_pg_symbols_file ON symbols(file_path);`

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `Cargo.toml` (workspace) | Modified | Add `sqlx` dep |
| `crates/cognicode-core/Cargo.toml` | Modified | Add `sqlx` + `postgres` feature |
| `crates/cognicode-core/src/infrastructure/persistence/schema_postgres.sql` | **New** | DDL |
| `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs` | **New** | Struct + trait impl + tests |
| `crates/cognicode-core/src/infrastructure/persistence/mod.rs` | Modified | Conditional re-export |

## Entropy Budget (Protocol B)

**Method**: Heuristic (±1 bit confidence). CogniCode graph build unavailable — using code reading.

| Metric | Estimate (bits) | Threshold | Status |
|--------|-----------------|-----------|--------|
| H(Δ_existing) — 3 files modified | log2(3) ≈ 1.58 | < 1.0 | ⚠️ AMBER (workspace-level dep) |
| H(Δ_new) — 2 new files | log2(2) ≈ 1.0 | > 0 | ✅ |
| New connascence pairs | 4 (Repository trait, PgPool, Symbol, SQL schema) | < 3 | ⚠️ AMBER |
| OCP compliant? | Yes — zero existing code modified | yes | ✅ |

**Verdict**: AMBER. The workspace Cargo.toml and feature-flag wiring push H(Δ_existing) above the 1.0-bit threshold, but this is a workspace-level configuration change — not an OCP violation on existing domain code. The connascence surface is narrow, explicit, and all pairs are through well-defined traits or column-name alignment.

**Design Quality Score (pre-slice)**: ~0.68/1.0 (ACCEPTABLE). Clean extension with low coupling through the existing `Repository` trait. High cohesion in the new PostgreSQL module. No LSP violations (no subtyping).

## Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| `sqlx` increases compile time (~30s) | High | Feature-gated behind `postgres`. Default build unaffected. |
| CI has no PostgreSQL service | Medium | Document prerequisite. Tests gate behind `#[cfg(feature = "postgres")]` — won't run in CI until service is added. |
| Raw SQL migration becomes unwieldy | Low | Only 1 table. Switch to `refinery` when table count > 3. |
| PostgreSQL schema drifts from SQLite | Low | Same column names/types. Column-compatibility checks in CI build matrix. |
| Feature flag proliferation | Low | `postgres` is additive to `persistence`. No mutual exclusion. |

## Rollback Plan

Revert the merge commit. `Repository` trait is unchanged — zero behavioral impact on existing consumers. `cognicode-core/Cargo.toml` drops the `postgres` feature and `sqlx` dep. Workspace `Cargo.toml` drops the `sqlx` workspace dep. No data migration to unwind — this slice creates tables, it doesn't alter them.

## Dependencies

- **Phase 1** (`explorer-graph-foundation`): Archived ✅ — provides `Symbol`, `Provenance`
- **Phase 2** (`explorer-graph-repository-bridge`): Archived ✅ — provides `Repository` trait, `RepositoryError`
- **External**: PostgreSQL 14+ running locally or in CI. `DATABASE_URL` env var or `sqlx::test` auto-provisioning.

## Success Criteria

- [ ] `cargo check --workspace` passes (no sqlx for default build)
- [ ] `cargo check --features postgres -p cognicode-core` passes
- [ ] `schema_postgres.sql` idempotent (run twice, no error)
- [ ] `find_symbol_by_qualified_name` returns `Some(Symbol)` for a seeded row
- [ ] `count_symbols` returns correct count per test database
- [ ] `sqlx::test` integration tests pass against real PostgreSQL
- [ ] All 295+ existing tests unaffected (zero regression)
- [ ] `cargo doc --no-deps` produces no new warnings

## Open Questions

**None.** Both architectural decisions are resolved:
1. **Migration strategy**: raw SQL via `include_str!` (auto-grill E1, OS=0.82). Revisit at >3 tables.
2. **Feature flag**: `postgres` feature on `cognicode-core`. Default build sqlx-free. Tests gated.

The spec phase can proceed immediately.
