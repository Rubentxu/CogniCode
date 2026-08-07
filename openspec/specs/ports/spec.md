# Spec: Ports — Port Type Contracts for DB-Agnostic Adapters

> **Domain**: ports · **Change**: e29-0-clean-ports · **Date**: 2026-08-01
> **Source**: ADR-028 port abstraction principles; ROADMAP E29 Phase 0 row 1

## Intent

The port types in `crates/cognicode-core/src/domain/ports/` MUST be DB-agnostic so the Phase 1 `LadybugStore` adapter can implement them without misrepresenting its runtime. ADR-028 §Port Constraints identifies PostgreSQL-specific terms (`SQLSTATE`, `ts_rank_cd`, `pg_stat_user_tables`) in doc comments that violate the abstraction. All port surface doc comments MUST use DB-neutral vocabulary.

## Preconditions (Given)

- **Given** ADR-028 §Port Constraints mandates DB-agnostic port types
- **And** `crates/cognicode-core/src/domain/ports/` contains the affected port types
- **And** `CallGraphStoreError` (renamed from `RepositoryError` on 2026-07-30) and `SearchPage` are the primary carriers of PG doc leaks

## Acceptance Scenarios (When/Then)

### Scenario S1 — `CallGraphStoreError::UniqueViolation` doc neutralized · @id: ports.s1

- **Given** the doc on `CallGraphStoreError::UniqueViolation` references "PostgreSQL unique-violation (`SQLSTATE 23505`)"
- **When** the developer reads the source
- **Then** the doc MUST describe "unique-constraint violation" WITHOUT referencing "PostgreSQL" or "SQLSTATE 23505"
- **And** the variant name `UniqueViolation` MUST remain unchanged

### Scenario S2 — `SearchPage::raw_rank` doc neutralized · @id: ports.s2

- **Given** the doc on `SearchPage::raw_rank` references "the `ts_rank_cd` value"
- **When** the developer reads the source
- **Then** the doc MUST describe "the underlying search backend's relevance score" WITHOUT referencing `ts_rank_cd` or `PostgreSQL`
- **And** the field name `raw_rank` MUST remain unchanged

### Scenario S3 — Adjacent PG mentions scrubbed in the port surface · @id: ports.s3

- **Given** ADR-028 cataloged 9+ PG mentions across 4 port files (e.g. `pg_stat_user_tables`, `FTS5`, `sqlx`, `PG stub`, `PG adapter`)
- **When** the developer runs `rg -i 'postgres|sqlstate|ts_rank|pg_trgm|FTS5' crates/cognicode-core/src/domain/ports/`
- **Then** matches MUST appear ONLY inside `#[cfg(feature = "postgres")]` adapter docs or where PG-specificity is intentional
- **And** the count of port-trait-doc leaks MUST be zero

### Scenario S4 — No behavioral change · @id: ports.s4

- **Given** the change is doc-only
- **When** the developer runs `cargo check --workspace` and `cargo test --workspace`
- **Then** both MUST exit 0 with NO new warnings and NO test regressions

### Scenario S5 — Naming drift acknowledged · @id: ports.s5

- **Given** ADR-028 references `RepositoryError` but the current type is `CallGraphStoreError` (renamed 2026-07-30)
- **When** the developer reads the spec
- **Then** `CallGraphStoreError` is used as the active type name
- **And** `RepositoryError` is acknowledged as the pre-rename legacy name

## PG-Neutral Vocabulary

| PG term | Replacement |
|---------|-------------|
| `SQLSTATE 23505` | "unique-constraint violation" |
| `ts_rank_cd` | "the underlying search backend's relevance score" |
| `pg_stat_user_tables` | (remove — not a port-level concern) |
| `tsquery` / `tsvector` | "full-text search query" / "full-text search document" |
| `ILIKE` | "case-insensitive LIKE" |
| `gen_random_uuid()` | "random UUID" |
| `jsonb` / `jsonb_path_*` | `JSON` / "JSON path query" |
| `citext` | "case-insensitive text" |
| `pg_dump` / `pg_restore` | "database dump" / "database restore" |
| `pg_trgm` | "trigram index" |
| `FTS5` | "full-text search" |

## Out of Scope

- Behavioral changes to `CallGraphStoreError` variants
- New error variants for LadybugDB-specific cases (Phase 1)
- Refactoring call sites of `CallGraphStoreError` (separate change `e29-0-refactor-call-sites`)
- Adapter files under `infrastructure/persistence/postgres_*` — these are correctly PG-specific

## Verification

- `cargo check --workspace` exits 0
- `cargo test --workspace` exits 0 with no test changes
- `rg -i 'postgres|sqlstate|ts_rank|pg_trgm|FTS5' crates/cognicode-core/src/domain/ports/` returns matches ONLY inside `#[cfg(feature = "postgres")]` blocks

## References

- [ADR-028 port abstraction](../../docs/adr/ADR-028-ladybugdb-port-abstraction-architecture.md)
- [e29-0-clean-ports delta spec](../../sddk/e29-0-clean-ports/spec.md)
