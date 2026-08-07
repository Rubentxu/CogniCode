# Design: Minimal PostgreSQL Repository

## Technical Approach

Pure extension: add a `PostgresRepository` struct behind a `postgres` feature flag that implements the existing async `Repository` trait (2 methods: `find_symbol_by_qualified_name`, `count_symbols`). Introduces `sqlx` as a workspace dependency with `postgres` + `runtime-tokio` features. Schema DDL is a single embedded SQL file executed via `sqlx::query`. Zero existing code modified.

## Architecture Decisions

| Decision | Choice | Rejected | Rationale |
|----------|--------|----------|-----------|
| Migration strategy | Raw SQL via `include_str!` | refinery, sqlx-cli | 1 table, ~25 lines DDL. Switch to refinery at >3 tables (header comment commitment). |
| Feature flag location | `postgres` on `cognicode-core` | Always-on, separate crate | sqlx adds ~30s compile time. Default build stays sqlx-free. |
| Crate location | Inside `cognicode-core` | New `cognicode-postgres` crate | `Repository` trait lives in core. Premature crate split risks circular deps. Revisit at >500 lines. |
| Pool ownership | `PostgresRepository` owns `PgPool` | Shared `Arc<PgPool>` via constructor | Single-owner is simplest. Consumers wrap in `Arc` if sharing needed. Constructor takes `PgPoolOptions` or `DatabaseURL`. |
| Schema idempotency | `CREATE TABLE IF NOT EXISTS` | Versioned migrations | Matches SQLite pattern in `cognicode-db/src/schema.rs`. No migration tooling needed for one table. |
| Row→Domain mapping | Manual `sqlx::query_as` + `FromRow` | sqlx::query! compile-time checked | `query!` requires `DATABASE_URL` at compile time — CI friction. Manual mapping is explicit and CI-friendly. Add `query!` later when CI has PG. |

## Data Flow

```
┌─────────────────┐
│  Consumer code   │
│  (Box<dyn Repository>)│
└────────┬────────┘
         │ async call
         ▼
┌─────────────────────────┐
│  PostgresRepository     │  #[cfg(feature = "postgres")]
│  - pool: PgPool         │
│  - run_migrations()     │  ← executes embedded schema_postgres.sql
│  - find_symbol_by_qualified_name() │
│  - count_symbols()      │
└────────┬────────────────┘
         │ sqlx::query
         ▼
┌─────────────────────────┐
│  PostgreSQL              │
│  symbols table           │
│  idx_pg_symbols_name     │
│  idx_pg_symbols_file     │
└─────────────────────────┘
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `Cargo.toml` (workspace) | Modify | Add `sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }` to `[workspace.dependencies]` |
| `crates/cognicode-core/Cargo.toml` | Modify | Add `postgres = ["dep:sqlx"]` feature; add `sqlx` dep with optional = true |
| `crates/cognicode-core/src/infrastructure/persistence/schema_postgres.sql` | Create | `symbols` table DDL with `CREATE TABLE IF NOT EXISTS` + indexes |
| `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs` | Create | `PostgresRepository` struct, `Repository` trait impl, `run_migrations()`, integration tests |
| `crates/cognicode-core/src/infrastructure/persistence/mod.rs` | Modify | Add `#[cfg(feature = "postgres")] pub mod postgres_repository;` + conditional re-export |

## Interfaces / Contracts

### `PostgresRepository` struct (new)

```rust
// postgres_repository.rs — #[cfg(feature = "postgres")]

use sqlx::PgPool;
use async_trait::async_trait;
use crate::domain::aggregates::Symbol;
use crate::domain::value_objects::{Location, SymbolKind};
use crate::domain::traits::{Repository, RepositoryError};

pub struct PostgresRepository {
    pool: PgPool,
}

impl PostgresRepository {
    pub async fn new(database_url: &str) -> Result<Self, RepositoryError> { ... }
    pub async fn from_pool(pool: PgPool) -> Self { ... }
    pub async fn run_migrations(&self) -> Result<(), RepositoryError> { ... }
}

#[async_trait]
impl Repository for PostgresRepository {
    async fn find_symbol_by_qualified_name(&self, name: &str) -> Result<Option<Symbol>, RepositoryError>;
    async fn count_symbols(&self) -> Result<usize, RepositoryError>;
}
```

### Row mapping: `SymbolRow` internal type

```rust
#[derive(sqlx::FromRow)]
struct SymbolRow {
    file_path: String,
    name: String,
    kind: Option<String>,
    line: Option<i32>,
    column: Option<i32>,
}
```

`SymbolRow::into_symbol()` maps `kind` via `SymbolKind` display string, constructs `Location::new(file_path, line as u32, column as u32)`, builds `Symbol::new(name, kind, location)`.

### Qualified name parsing

`find_symbol_by_qualified_name("src/lib.rs:foo:10")` splits on `:` → `WHERE file_path = $1 AND name = $2 AND line = $3`.

### `schema_postgres.sql`

```sql
-- Commitment: switch to refinery when table count > 3.
CREATE TABLE IF NOT EXISTS symbols (
    id     SERIAL PRIMARY KEY,
    file_path TEXT NOT NULL,
    name      TEXT NOT NULL,
    kind      TEXT,
    line      INTEGER,
    column    INTEGER,
    complexity INTEGER
);
CREATE INDEX IF NOT EXISTS idx_pg_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_pg_symbols_file ON symbols(file_path);
```

### Cargo.toml changes

**Workspace `Cargo.toml`:**
```toml
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
```

**`crates/cognicode-core/Cargo.toml`:**
```toml
[features]
default = ["persistence"]
rig = ["dep:rig-core"]
persistence = ["dep:blake3"]
postgres = ["dep:sqlx"]

[dependencies]
sqlx = { workspace = true, optional = true }
```

### `mod.rs` changes

```rust
#[cfg(feature = "persistence")]
pub mod memory_graph_store;

#[cfg(feature = "persistence")]
pub use memory_graph_store::InMemoryGraphStore;

#[cfg(feature = "postgres")]
pub mod postgres_repository;

#[cfg(feature = "postgres")]
pub use postgres_repository::PostgresRepository;

#[cfg(test)]
mod store_contract_tests;
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `SymbolRow::into_symbol()` mapping | In-memory, no DB. Verify kind parsing, default values. |
| Integration | `run_migrations` idempotency | `#[sqlx::test]` → call twice → assert no error, table exists. |
| Integration | `find_symbol_by_qualified_name` happy path | `#[sqlx::test]` → seed row → query by qualified name → assert `Symbol` fields match. |
| Integration | `find_symbol_by_qualified_name` not-found | `#[sqlx::test]` → empty DB → assert `Ok(None)`. |
| Integration | `count_symbols` | `#[sqlx::test]` → assert 0 empty → seed N rows → assert N. |
| Regression | Default build sqlx-free | `cargo check --workspace` (no `--features postgres`) passes. |
| Regression | Existing tests pass | `cargo test --workspace` (no `--features postgres`) — all 295+ tests green. |

Integration tests gated: `#[cfg(all(test, feature = "postgres"))]`.

## Migration / Rollout

No data migration required. Pure addition. Rollback = revert merge commit, drop `postgres` feature + `sqlx` dep.

## Entropy Constraints (Protocol C)

**Method**: Heuristic (±1 bit confidence)

| Interface | I(X;T) Leakage | I(T;Y) Coverage | Bottleneck Quality | SOLID Check |
|-----------|---------------|-----------------|-------------------|-------------|
| `Repository` trait (existing, unchanged) | 0 bits — `PostgresRepository` internals invisible | 0.95 — 2 methods match all caller needs | ✅ Optimal | SRP ✅ ISP ✅ DIP ✅ |
| `PostgresRepository` (new) | 0.2 bits — `PgPool` type visible in constructor | 1.0 — implements full trait | ✅ Optimal | SRP ✅ (single purpose: PG persistence) |

**Interface Design Issues**: None — trait is minimal (2 methods), implementation is behind feature gate.

**SRP Split Candidates**: None — `PostgresRepository` has single responsibility.

**ISP Violations**: None — `Repository` exposes exactly what callers need (find + count).

**DIP Assessment**: Consumers depend on `dyn Repository` (high-H abstraction), not `PostgresRepository` (low-H concretion). ✅

**Design Quality Score**: ~0.78/1.0 (GOOD) — clean extension, zero H(Δ_existing) on domain code, low coupling, high cohesion.

## Open Questions

None. Both architectural decisions resolved in explore/proposal (auto-grill OS=0.82).
