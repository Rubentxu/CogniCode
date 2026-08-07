# Exploration: postgres-default-config

> Make PostgreSQL the default development/CI configuration, with SQLite behind a feature flag only.
> Roadmap Phase 3: "The default development and CI configuration is PostgreSQL."

## Current State

### Feature Flags (as of 2026-06-09)

| Crate | Default features | `postgres` feature | `sqlite` feature |
|-------|-----------------|-------------------|-----------------|
| `cognicode-core` | `["persistence"]` | additive (`dep:sqlx`) | **does not exist** |
| `cognicode-explorer` | none | additive (`cognicode-core/postgres`, `dep:sqlx`, `dep:async-trait`) | **does not exist** |
| `cognicode-db` | none | n/a | **does not exist** — `rusqlite` always compiled in |

**Key fact**: SQLite/`rusqlite` is unconditionally compiled into both `cognicode-core` and `cognicode-explorer`. There is no `sqlite` feature flag anywhere. The `postgres` feature is additive to the default build — it's opt-in, not default.

### CLI Entry Points

**`api.rs` and `mcp.rs`** (identical pattern):
- `SqliteGraphStore` imported unconditionally
- `cognicode-db` imported unconditionally  
- Default path: `cwd/.cognicode/cognicode.db` (SQLite)
- `--postgres <URL>` flag: `#[cfg(feature = "postgres")]` gated, completely absent from default build
- PG path drops the pool after loading graph into memory (no live connection kept for API server path)

**`service.rs`**:
- `open_workspace()` (line 224) checks for `.cognicode/cognicode.db` existence — SQLite-specific
- `postgres_repo: Option<Arc<PostgresRepository>>`: `#[cfg(feature = "postgres")]` gated field
- Named-view CRUD methods (`save_view`, `load_view`, `list_views`, `delete_view`): all `#[cfg(feature = "postgres")]` gated
- When `postgres_repo` is `None`, named-view calls return `ExplorerError::FeatureDisabled`

### CI / Infrastructure

- **No CI config**: `.github/workflows/` does not exist
- **No docker-compose**: No `docker-compose.yml` at workspace root
- **No `.env.example`**: Only `.env` exists with `COGNICODE_PROJECT_PATH` only
- **`justfile`**: Has `explorer-api`, `explorer-dev`, etc., but no `dev-pg` or `test-pg` targets

### Persistence Architecture

```
cognicode-core (domain)
  ├── Repository trait (async, domain-level)       ← implemented by PostgresRepository
  ├── GraphStore trait (sync, blob storage)         ← deprecated, in migration
  ├── InMemoryGraphStore                            ← #[cfg(feature = "persistence")]
  └── PostgresRepository                            ← #[cfg(feature = "postgres")]
  
cognicode-db (SQLite impl)
  └── SqliteGraphStore + Fts5Index + QualityStore   ← always compiled

cognicode-store-traits (DEPRECATED)
  └── Old trait copies, kept for workspace compilation
```

**PG schema**: `cognicode-core/src/infrastructure/persistence/schema_postgres.sql` defines `symbols`, `call_edges`, `named_views` tables with indexes. Migrations run via `include_str!` at `PostgresRepository::new()`.

### Test Landscape

| Test file | Gate | Backend | Will break with PG default? |
|-----------|------|---------|---------------------------|
| `integration.rs` (~30 tests) | none — runs always | `SqliteGraphStore` | **YES** — hardcoded SQLite |
| `explorer_graph_foundation.rs` | none | `SqliteGraphStore` | **YES** — hardcoded SQLite |
| `pg_bridge_contract.rs` | `#[cfg(all(test, feature = "postgres"))]` | PG (via `TEST_DATABASE_URL`) | No — already correctly gated |
| `named_views_integration.rs` | `#[cfg(all(test, feature = "postgres"))]` | PG (via `TEST_DATABASE_URL`) | No — already correctly gated |
| `postgres_repository.rs::tests` | `#[cfg(all(test, feature = "postgres"))]` | PG (via `TEST_DATABASE_URL`) | No — already correctly gated |
| `brain_session_lifecycle.rs` | `#[cfg(feature = "test-utils")]` | in-memory only | No — no persistence |
| `mcp_edge_metadata.rs` | none | in-memory `CallGraph` | No — no persistence |
| `metadata_aware_repository.rs` | none | in-memory `CallGraph` | No — no persistence |

**PG test pattern**: Tests check `TEST_DATABASE_URL` env var, gracefully skip when unset. Each test creates a unique DB (`cognicode_test_{pid}_{n}`), runs schema, and leaks cleanup (best-effort via tokio task).

## Affected Areas

### Files to modify

```
Cargo.toml                                          — rusqlite optionality
crates/cognicode-core/Cargo.toml                     — default = ["postgres"], add "sqlite" feature
crates/cognicode-explorer/Cargo.toml                 — default = ["postgres"], add "sqlite" feature
crates/cognicode-db/Cargo.toml                       — add "sqlite" feature gate
crates/cognicode-explorer/src/bin/api.rs             — flip CLI default, add --sqlite
crates/cognicode-explorer/src/bin/mcp.rs             — flip CLI default, add --sqlite
crates/cognicode-explorer/src/service.rs             — open_workspace() PG path, clone_service_handle()
crates/cognicode-core/src/interface/mcp/handlers/aix_handlers.rs — gate rusqlite usage
```

### Files to gate behind `#[cfg(feature = "sqlite")]`

```
crates/cognicode-explorer/tests/integration.rs       — all 30 tests
crates/cognicode-explorer/tests/explorer_graph_foundation.rs — both tests
crates/cognicode-explorer/src/adapters/fts5_search_adapter.rs — rusqlite dep
crates/cognicode-explorer/src/adapters/sqlite_quality_adapter.rs — rusqlite dep
```

### Files to create

```
docker-compose.yml                                   — PG 16 for local dev
.env.example                                         — DATABASE_URL, TEST_DATABASE_URL
.github/workflows/ci.yml                             — CI with PG 16 service container
```

## Approaches

### Approach 1: Feature Flip + sqlite Gate (Recommended)

**What**: Add `sqlite` feature flag to `cognicode-core`, `cognicode-db`, `cognicode-explorer`. Make `postgres` the default feature. Gate all rusqlite/SqliteGraphStore paths behind `#[cfg(feature = "sqlite")]`. Flip CLI: `DATABASE_URL` env var becomes default, `--sqlite` flag for opt-in SQLite path.

**Default build behavior after change**:
- `cargo build` → compiles with `postgres` + `persistence` features, connects to PG via `DATABASE_URL`
- `cargo build --no-default-features --features sqlite` → old behavior (SQLite only)
- `cargo build --features sqlite` → both backends compiled
- `cargo test` → runs PG tests (if `TEST_DATABASE_URL` set) + in-memory tests; SQLite tests skipped unless `--features sqlite`

**Pros**:
- Matches roadmap Phase 3 intent exactly
- Reversible: `cargo build --no-default-features --features sqlite` restores old behavior
- Clean separation — no dead code in PG-default builds
- PG implementation already battle-tested (~20 contract tests across 3 files)
- No new domain logic — pure configuration/flag plumbing

**Cons**:
- Touches many files (3 Cargo.toml, 2 binaries, 1 service, 1 handler, 5 test files)
- `cognicode-db` consumers (`cognicode-mcp`, `cognicode-quality`) also need feature propagation
- `aix_handlers.rs` in `cognicode-core` uses raw `rusqlite::Connection` — needs gating

**Effort**: Medium — flag plumbing, ~20 files modified/created, no new logic

### Approach 2: Dual-Default with Runtime Detection (Not Recommended)

**What**: Compile both backends always. At runtime, try PG first (`DATABASE_URL`), fall back to SQLite silently.

**Pros**: No feature flag complexity, no build matrix.

**Cons**:
- Always pulls `sqlx` dependency (heavier compile times for all builds)
- Silent fallback hides PG connection problems
- Violates roadmap intent ("SQLite behind feature flag only")
- Harder to enforce in CI — PG tests pass with SQLite silently
- Goes against stack-recommendation.md directive: "SQLite is acceptable only as a transitional compatibility layer"

**Effort**: Low — less code change, but wrong architectural direction

### Approach 3: PG Default in CI Only, Local Stays SQLite

**What**: Keep local dev SQLite, make CI use PG only.

**Pros**: Minimal disruption to developer workflow.

**Cons**: CI != dev parity risk. Roadmap says "development AND CI configuration is PostgreSQL". Half-measure that delays the inevitable.

## Recommendation

**Approach 1: Feature Flip + sqlite Gate.** 

The roadmap and stack recommendation are unambiguous — PostgreSQL is the canonical store, SQLite is transitional compatibility behind a flag. The PG implementation already exists and is tested. This change is a flag flip, not a rewrite.

The main value is in establishing CI parity with production intent. Running PG in CI from now catches integration issues early (connection pooling, migration idempotency, concurrent access patterns) that SQLite hides.

### Implementation Outline (for proposal phase)

1. **Cargo.toml changes**: Add `sqlite` feature, swap defaults
2. **CLI flip**: `DATABASE_URL` env var as default, `--sqlite` behind feature flag
3. **Test gating**: Gate SQLite-only tests, add CI mode that fails when `TEST_DATABASE_URL` unset
4. **Infrastructure**: `docker-compose.yml` + `.env.example` + `.github/workflows/ci.yml`
5. **`justfile`**: Add `just dev-pg` (start PG via docker-compose + build), `just test-pg`
6. **Documentation**: Update `.env.example`, `README.md` or `CONTRIBUTING.md` with PG setup

## Key Decisions Needed

1. **Default DATABASE_URL**: `postgres://cognicode:cognicode@localhost:5432/cognicode`? Or `postgres://localhost:5432/cognicode` (trust auth for local docker)?
2. **cognicode-db restructuring**: Feature-gate the existing crate, or split into `cognicode-db` (types) + `cognicode-db-sqlite` (impl)?
3. **CI PG version**: PostgreSQL 16 (latest stable) or 15 (Debian/Ubuntu LTS default)?
4. **Test strictness**: Should CI fail when `TEST_DATABASE_URL` is unset? Or keep the graceful skip pattern?
5. **Migration path for existing `.cognicode/cognicode.db` files**: Auto-import tool? Manual export/import? Just start fresh?

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| `cognicode-core` MCP handlers use raw `rusqlite::Connection` (aix_handlers.rs:1771) — breaking when sqlite feature off | High | Gate all rusqlite usage in core behind `#[cfg(feature = "sqlite")]`. These handlers are core-level SQLite access, not through `cognicode-db`. |
| `cognicode-mcp` and `cognicode-quality` crates depend on `cognicode-db` → need feature propagation | Medium | Either propagate `sqlite` feature through dependency chain, or keep `cognicode-db` always compiled (making SQLite un-gateable at the db crate level). The latter means `rusqlite` is always in the dep graph but test gating still works. |
| No existing CI → must build from scratch | Medium | Start with minimal CI: checkout, install Rust, start PG service, `cargo test --workspace`. Expand later. |
| `service.rs:233` hardcodes `.cognicode/cognicode.db` check | Low | Add PG-aware variant that checks `named_views` table count instead of file existence. |
| Developer friction — requiring PG for local dev | Low | Docker Compose makes PG a single `docker compose up -d` away. SQLite still available behind feature flag for quick experiments. |
| PG test DB cleanup leaks databases on CI | Low | Current pattern already handles this — databases are named with PID+counter, manual cleanup script for operator. |

## Ready for Proposal

Yes. The analysis covers all key questions from the exploration brief. The orchestrator can proceed to `sdd-propose` with a clear scope:

> Invert the PostgreSQL/SQLite feature flag relationship: make PostgreSQL the default, introduce a `sqlite` feature flag, gate all SQLite-specific code behind it, add docker-compose for local development, and create a GitHub Actions CI workflow with a PostgreSQL service container.
