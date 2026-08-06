# Spec: postgres-symbol-repository (explorer-graph-postgres-repository)

> **OBSOLETE (2026-08-04)** — PostgreSQL was fully removed from the runtime in E29 (v0.79.0,
> `e29-7-remove-postgres-repository`). This spec describes a backend that no
> longer exists in the canonical build. Retained for historical record; its
> requirements count as *triaged* (legacy_obsolete) in the conformance matrix.

## Purpose

First PostgreSQL-backed impl of the async `Repository` trait in `cognicode-core`. Pure extension — trait unchanged, no existing code modified.

## ADDED Requirements

### Requirement: PostgresRepository implements the async `Repository` trait

`PostgresRepository` in `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs`, gated `#[cfg(feature = "postgres")]`, MUST hold a `sqlx::PgPool` and implement both methods of `cognicode_core::domain::traits::Repository` via `#[async_trait]`.

#### Scenario: Trait satisfied + find returns seeded row
- GIVEN `(file_path='src/lib.rs', name='foo', kind='Function', line=10, column=2)` inserted AND a `PostgresRepository` from a `PgPool`
- WHEN passed where `&dyn Repository` is expected
- THEN both methods MUST be reachable, the type MUST be `Send + Sync`, AND `find_symbol_by_qualified_name("src/lib.rs:foo:10")` MUST return `Ok(Some(Symbol{name="foo", kind=Function, location(10,2)}))`

#### Scenario: Not-found is `Ok(None)`; count matches
- GIVEN empty `symbols` initially AND N=7 rows after seeding
- WHEN `find_symbol_by_qualified_name("nope")` is called
- THEN result MUST be `Ok(None)` (NOT `RepositoryError::NotFound`)
- AND `count_symbols()` MUST equal 0 in the empty case AND 7 after seeding

### Requirement: Canonical `symbols` schema for PostgreSQL

`schema_postgres.sql` MUST define `symbols` column-compatible with SQLite `symbols` in `cognicode-db/src/schema.rs`. Header MUST commit to `refinery` once table count > 3. Columns: `id SERIAL PK`, `file_path TEXT NOT NULL`, `name TEXT NOT NULL`, `kind/line/column/complexity` (nullable). Indexes: `idx_pg_symbols_name(name)`, `idx_pg_symbols_file(file_path)`.

#### Scenario: Idempotent apply + column parity
- GIVEN an empty PG database
- WHEN `run_migrations()` is called twice
- THEN the second call MUST succeed, post-state MUST equal post-first-call state, AND column names MUST match exactly: `id, file_path, name, kind, line, column, complexity`

### Requirement: Feature flag `postgres` on `cognicode-core`

A `postgres` feature on `crates/cognicode-core/Cargo.toml` MUST activate `sqlx` (`postgres`, `runtime-tokio`). Default features (`["persistence"]`) MUST NOT pull `sqlx`. `postgres` MUST be additive to `persistence`.

#### Scenario: Default sqlx-free vs feature-enabled
- GIVEN a clean workspace
- WHEN `cargo check --workspace` runs
- THEN `sqlx` MUST NOT appear in the dep graph AND build MUST succeed
- AND WHEN `cargo check -p cognicode-core --features postgres --no-default-features` runs
- THEN compile MUST succeed AND `sqlx::PgPool` MUST be reachable from `cognicode_core`

### Requirement: Raw SQL migration loading via `include_str!`

`schema_postgres.sql` MUST be loaded at compile time via `include_str!` and executed via `sqlx::query(...)` inside `PostgresRepository::run_migrations(&self)`. No migration framework SHALL be added.

#### Scenario: Embedded at compile time + idempotent on populated DB
- GIVEN the `postgres` feature AND a DB with existing `symbols` rows
- WHEN `cargo build -p cognicode-core` runs
- THEN schema bytes MUST be present in the rlib (verifiable via `strings`) AND editing the SQL MUST require rebuild
- AND WHEN `run_migrations()` is called on the populated DB
- THEN it MUST succeed AND MUST NOT drop or alter existing rows

### Requirement: Compatibility — non-breaking build without `postgres`

`cargo check --workspace` MUST stay green without `postgres`. `PostgresRepository`, its impl, and the re-export MUST be absent from the default build (`#[cfg(feature = "postgres")]`). Zero existing public API of `cognicode-core` SHALL change.

#### Scenario: No API drift + existing tests unaffected
- GIVEN pre-slice public API of `cognicode-core` AND the 295+ pre-slice test suite
- WHEN `cargo doc --no-deps -p cognicode-core` runs without `--features postgres`
- THEN generated docs MUST expose the same public items as before
- AND WHEN `cargo test --workspace` runs without `--features postgres`
- THEN every pre-slice-passing test MUST still pass

### Requirement: Testability — `sqlx::test` integration tests

Integration tests in `postgres_repository.rs` (`#[cfg(all(test, feature = "postgres"))]`) MUST use `#[sqlx::test]` for per-test isolated DBs. Suite MUST cover: happy-path `find`; not-found returns `None`; `count_symbols` matches row count; `run_migrations` idempotence.

#### Scenario: Per-test isolation + golden Symbol match
- GIVEN two `#[sqlx::test]` functions AND a row `(file_path='a.rs', name='fn', kind='Function', line=1, column=0)`
- WHEN the tests run in parallel
- THEN each MUST observe an isolated DB AND rows of one MUST NOT be visible to the other
- AND `find_symbol_by_qualified_name("a.rs:fn:1")` MUST return `Symbol{name="fn", kind=Function, line()==1, column()==0, file()=="a.rs"}`

### Requirement: Rollout safety — single feature, no behavior leak

The slice MUST be a single feature-gated module. With `postgres` disabled, no code path reaches `PostgresRepository` or `PgPool`. The slice MUST NOT alter any synchronous `GraphStore` implementor and MUST NOT modify the `Repository` trait. PR budget: `additions + deletions ≤ 400` (risk Low).

#### Scenario: Disabled build isolation + PR size budget
- GIVEN a default-features build AND planned changes (1 schema file, 1 module, 1 Cargo.toml edit, 1 conditional re-export)
- WHEN a consumer writes `use cognicode_core::infrastructure::persistence::PostgresRepository;`
- THEN it MUST fail to compile
- AND WHEN the diff is computed
- THEN `additions + deletions` MUST be ≤ 400 AND `400-line-budget-risk` MUST be Low
