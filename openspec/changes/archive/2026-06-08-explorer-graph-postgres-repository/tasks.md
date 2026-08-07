# Tasks: Minimal PostgreSQL Repository

**Change**: `explorer-graph-postgres-repository` (Phase 3 of Explorer Graph roadmap)
**Project**: cognicode
**Mode**: automatic, hybrid (LogSeq + Engram)
**Delivery**: single PR
**Spec/Design Status**: approved

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~200 added, ~5 modified (~205 total) |
| 400-line budget risk | Low (single feature-gated module) |
| Chained PRs recommended | No |
| Suggested split | single PR |
| Delivery strategy | single-pr |
| Chain strategy | size:exception |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low

## Phase 1: Foundation — Cargo Wiring (sqlx workspace dep + feature flag)

- [x] 1.1 Add `sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "postgres", "macros"] }` to `[workspace.dependencies]` in `Cargo.toml` (workspace root, after line 116 `tokio-test` block).
- [x] 1.2 Add `postgres = ["dep:sqlx"]` feature in `[features]` of `crates/cognicode-core/Cargo.toml` (after line 11 `persistence` feature — keeps `postgres` additive to `persistence`).
- [x] 1.3 Add `sqlx = { workspace = true, optional = true }` in `[dependencies]` of `crates/cognicode-core/Cargo.toml` (next to `rusqlite` line 94 — same optional-dep pattern).
- [x] 1.4 Verify `cargo check -p cognicode-core` (default features) still passes (no `--features postgres` → sqlx absent from dep graph).

## Phase 2: Core Implementation — Schema, Struct, Repository Impl

- [x] 2.1 Create `crates/cognicode-core/src/infrastructure/persistence/schema_postgres.sql` with: (a) header comment committing to `refinery` once table count > 3; (b) `CREATE TABLE IF NOT EXISTS symbols (id SERIAL PRIMARY KEY, file_path TEXT NOT NULL, name TEXT NOT NULL, kind TEXT, line INTEGER, column INTEGER, complexity INTEGER)` — column-for-column parity with `crates/cognicode-db/src/schema.rs:69-77`; (c) `CREATE INDEX IF NOT EXISTS idx_pg_symbols_name ON symbols(name)`; (d) `CREATE INDEX IF NOT EXISTS idx_pg_symbols_file ON symbols(file_path)`.
- [x] 2.2 Create `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs` with module-level `#[cfg(feature = "postgres")]` — file compiles to nothing when feature disabled.
- [x] 2.3 In `postgres_repository.rs`, define `pub struct PostgresRepository { pool: sqlx::PgPool }` + `impl PostgresRepository { pub async fn new(database_url: &str) -> Result<Self, RepositoryError>; pub fn from_pool(pool: PgPool) -> Self; pub async fn run_migrations(&self) -> Result<(), RepositoryError> { /* include_str!("schema_postgres.sql") via sqlx::query */ } }`.
- [x] 2.4 In same file, add internal `#[derive(sqlx::FromRow)] struct SymbolRow { file_path: String, name: String, kind: Option<String>, line: Option<i32>, column: Option<i32> }` with `fn into_symbol(self) -> Symbol` that parses `kind` via `SymbolKind` `FromStr` (extend `SymbolKind` with `FromStr` impl if absent — see Phase 2.5) and constructs `Location::new(file_path, line as u32, column as u32)`.
- [x] 2.5 In `crates/cognicode-core/src/domain/value_objects/symbol_kind.rs`, add `impl FromStr for SymbolKind` (round-trips `Display` output, returns `RepositoryError::InvalidQuery` upstream) — **only** if the inverse `FromStr` is not already present; if present, reuse it.
- [x] 2.6 In `postgres_repository.rs`, write `#[async_trait] impl Repository for PostgresRepository`: `find_symbol_by_qualified_name` splits `name` on `:` (format `file:name:line`) and runs `SELECT file_path, name, kind, line, column FROM symbols WHERE file_path = $1 AND name = $2 AND line = $3 LIMIT 1`; `count_symbols` runs `SELECT COUNT(*) FROM symbols`. Errors map `sqlx::Error::RowNotFound` → no special handling (use `.optional()`); other errors → `RepositoryError::Store(e.to_string())`.
- [x] 2.7 In `crates/cognicode-core/src/infrastructure/persistence/mod.rs`, append (under line 10 `pub use memory_graph_store`): `#[cfg(feature = "postgres")] pub mod postgres_repository;` and `#[cfg(feature = "postgres")] pub use postgres_repository::PostgresRepository;`. Do NOT modify existing lines.
- [x] 2.8 Verify `cargo check -p cognicode-core --features postgres` passes and `sqlx::PgPool` is reachable.

## Phase 3: Integration Tests — `sqlx::test` with real PostgreSQL

- [x] 3.1 In `postgres_repository.rs`, add `#[cfg(all(test, feature = "postgres"))] mod tests` block at the bottom. **DEVIATION**: instead of `#[sqlx::test]` (requires sqlx `migrate` feature which conflicts with the workspace's `rusqlite` via the `links=sqlite3` constraint), a small `pg_test!` macro creates a per-test uniquely-named database via a manual fixture. Same per-test isolation guarantee; the `TEST_DATABASE_URL` env var picks the base URL. Documented in PR.
- [x] 3.2 Test `find_returns_seeded_symbol`: insert row `(file_path='src/lib.rs', name='foo', kind='Function', line=10, column=2)` then assert `repo.find_symbol_by_qualified_name("src/lib.rs:foo:10").await` returns `Ok(Some(Symbol))` with `name=="foo"`, `kind==Function`, `location().line()==10`, `location().column()==2`, `location().file()=="src/lib.rs"`. (Covers spec scenario 1a.)
- [x] 3.3 Test `find_returns_none_when_missing`: empty DB → `repo.find_symbol_by_qualified_name("nope").await` returns `Ok(None)` (NOT `Err(NotFound)`). (Covers spec scenario 1b.)
- [x] 3.4 Test `count_symbols_matches_rows`: empty DB → `count_symbols()==0`; insert N=7 rows → `count_symbols()==7`. (Covers spec scenario 1b second clause.)
- [x] 3.5 Test `run_migrations_idempotent_on_empty`: call `repo.run_migrations()` twice on an empty DB → second call succeeds, post-state equals post-first-call state (no error from `IF NOT EXISTS`). (Covers spec scenario 2a.)
- [x] 3.6 Test `run_migrations_preserves_rows`: seed row then call `run_migrations()` → migration succeeds AND row still present (no DROP, no schema-altering DDL). (Covers spec scenario 4b second clause.)
- [x] 3.7 Test `per_test_isolation`: two `pg_test!` functions in the same suite each insert different rows → neither sees the other's row (proves per-test DB isolation). (Covers spec scenario 6a.)
- [x] 3.8 Test `kind_round_trip_via_display`: insert row with `kind='Function'`, query via `find_symbol_by_qualified_name` → returned `Symbol::kind() == SymbolKind::Function` (proves the `FromStr` mapping from Phase 2.5).
- [x] 3.9 Test `dyn_repository_compatible`: in a test, write `let boxed: Box<dyn Repository> = Box::new(PostgresRepository::from_pool(pool));` then call `boxed.count_symbols().await` — proves the trait object path works with the new impl. (Covers spec scenario 1a `Send + Sync` + `dyn Repository` clauses.)

## Phase 4: Verification & Rollout

- [x] 4.1 Run `cargo check -p cognicode-core` (no features) — passes; sqlx NOT in dep graph for default build.
- [x] 4.2 Run `cargo check -p cognicode-core --features postgres` — passes; `sqlx::PgPool` reachable from `cognicode_core`.
- [x] 4.3 Run `cargo check -p cognicode-core --all-features` — passes; `postgres` + `persistence` + `rig` coexist.
- [x] 4.4 Run `cargo test -p cognicode-core --lib` (no features) — all 1073+ pre-slice tests pass with zero regression (SymbolKind FromStr tests + repository trait tests verified).
- [ ] 4.5 Run `cargo test -p cognicode-core --features postgres` against a running PostgreSQL 14+ (local or CI) — **CANNOT VERIFY HERE**: no PostgreSQL service available in this environment. Test code is verified to compile (`cargo check --features postgres --tests` passes). PR description will document the prerequisite.
- [x] 4.6 Run `cargo check -p cognicode-core --features postgres` (no doc) — no new warnings.
- [x] 4.7 Confirmed via `strings target/debug/libcognicode_core.rlib | grep "CREATE TABLE"` — `schema_postgres.sql` is embedded at compile time (spec scenario 4a).
- [x] 4.8 Confirmed via a small `cognicode-core` consumer crate: `use cognicode_core::infrastructure::persistence::PostgresRepository;` does NOT compile under default features (`error[E0425]: cannot find type 'PostgresRepository' in module 'infrastructure::persistence'`). With `--features cognicode-core/postgres` it compiles. The cfg gate is hermetic.
- [ ] 4.9 Verify line budget: actual diff is ~584 lines (added), 2 deletions. **OVER 400 budget**. The spec forecast of ~205 was too optimistic — full integration test code (9 tests with a manual fixture) accounts for the extra lines. Production code is ~198 lines; tests are ~247 lines; SQL + Cargo + mod.rs + symbol_kind edits are the rest. Recommend accepting size:exception for this slice (already pre-flagged in the forecast).
- [x] 4.10 Single-commit reversibility: revert the merge commit drops the `postgres` feature, drops the `sqlx` dep, drops `PostgresRepository` re-export. No data migration to unwind.
- [ ] 4.11 Update PR description with: feature-flag pattern, sqlx migration strategy commitment, Phase 3 unblocking of `call_edges` + `GraphStore-PG` + explorer bridge + MCP envelope, manual test fixture note (replaces `#[sqlx::test]` due to sqlite links conflict).

## Dependencies Between Tasks

- Phase 1 (Cargo) → Phase 2 (code that uses `sqlx`) — must compile with the dep
- Phase 2.1 (schema file) → Phase 2.3 (`include_str!` references it)
- Phase 2.5 (FromStr) → Phase 2.4 (SymbolRow uses it) → Phase 2.6 (impl uses it)
- Phase 2.7 (mod.rs gate) → Phase 3 (tests must be reachable)
- Phase 2 → Phase 3 (impl must compile before tests run)
- Phase 1 + 2 + 3 → Phase 4 (verification needs the full slice in tree)

## Constraints Honored

- No `call_edges` table or edge queries
- No PostgreSQL `GraphStore` impl (synchronous blob path untouched)
- No `SymbolRepository` / `MetadataAwareRepository` bridge work
- No `ltree`, `pgvector`, or any PostgreSQL extensions
- No new methods on the `Repository` trait
- No CI pipeline changes (Phase 4.5 documents the prereq; doesn't add the service)
- `cognicode-store-traits` removal deferred
- Synchronous `GraphStore` implementors (`InMemoryGraphStore`, `SqliteGraphStore`, `PetGraphStore`) untouched
- `Repository` trait contract byte-for-byte unchanged
- No `#[serde]` on `PostgresRepository` (not a domain type)
- All PostgreSQL code is `#[cfg(feature = "postgres")]`-gated

## Risk Notes for sdd-apply

- **sqlx compile time**: ~30s added when `postgres` feature is enabled. Default build is unaffected.
- **PostgreSQL service**: Phase 3 tests + Phase 4.5 require a running PostgreSQL 14+. Document prerequisite in PR description; do not gate CI on it yet.
- **Phase 2.5 FromStr**: if `SymbolKind` already has a `FromStr` impl, reuse it — do not add a duplicate. Check `crates/cognicode-core/src/domain/value_objects/symbol_kind.rs` first. **(CONFIRMED: not present, added.)**
- **`Location::line`/`column` types**: spec scenario uses `line()==1` (i.e. `u32`). Confirm `Location::line()` returns `u32` before casting in `SymbolRow::into_symbol` — current `symbol.rs:23` shows `line as u32` cast in `Symbol::new` itself, so cast inside `into_symbol` is redundant. **(CONFIRMED: `Location::line()` returns `u32`, but `SymbolRow` reads from `Option<i32>` so the `as u32` cast inside `into_symbol` IS needed.)**
- **sqlx::Error mapping**: `sqlx::Error::RowNotFound` is NOT used here because `fetch_optional` returns `Ok(None)`. Only `Store(String)` variant is used for non-NotFound errors per spec scenario 1b. **(HANDLED: `fetch_optional` + `?` operator, no `RowNotFound` mapping needed.)**
- **DEVIATION — `#[sqlx::test]` replaced**: the `migrate` feature required by that macro pulls `sqlx-sqlite`, which conflicts with the workspace's `rusqlite` due to `links="sqlite3"` (cargo resolution error). We substitute a small `pg_test!` macro that creates per-test unique databases via `TEST_DATABASE_URL`. Same isolation guarantee, no extra features. Documented in PR.
- **DEVIATION — line budget**: ~584 lines added vs 400 budget. Production code is ~198 lines; tests ~247 lines. Spec forecast of ~205 was too optimistic. Pre-flagged `size:exception` in the forecast; this is the expected outcome. Recommend accepting the exception.
- **Pre-existing workspace issue**: `cognicode-axiom` has 7 missing rule files (`s1226_rule`, etc.) — unrelated to this slice, pre-existing. `cognicode-core` builds cleanly. Spec scenario 4.4 was "all 295+ tests pass" — the **cognicode-core** test suite passes (verified 1073+ tests in lib test). Other crates' pre-existing test issues are out of scope.
