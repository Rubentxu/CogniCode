# Proposal: PostgreSQL Default Configuration

## Intent

Invert the PostgreSQL/SQLite relationship. Make PostgreSQL the default build, move SQLite behind an opt-in `sqlite` feature flag. This fulfills Roadmap Phase 3: "The default development and CI configuration is PostgreSQL." PG is already implemented and tested — this is a flag flip, not a rewrite.

## Scope

### In Scope
- Add `sqlite` feature flag to `cognicode-core`, `cognicode-db`, `cognicode-explorer`
- Make `postgres` the default feature in `cognicode-core` and `cognicode-explorer`
- Gate all `rusqlite`/`SqliteGraphStore` usage behind `#[cfg(feature = "sqlite")]`
- Flip CLI: `DATABASE_URL` env var as default, `--sqlite` flag for opt-in SQLite
- Gate SQLite-only tests (`integration.rs`, `explorer_graph_foundation.rs`) behind `sqlite` feature
- Add `docker-compose.yml` (PG 16), `.env.example`, `.github/workflows/ci.yml`
- Add `just dev-pg` and `just test-pg` targets

### Out of Scope
- SQLite-to-PostgreSQL data migration tool
- `cognicode-db` crate split (feature-gate existing crate only)
- PG version compatibility beyond 16
- Expanding PG test coverage beyond existing contract tests

## Capabilities

### New Capabilities
- `sqlite-feature-gate`: SQLite backend gated behind opt-in `sqlite` feature flag, isolated from default build
- `ci-postgres-pipeline`: GitHub Actions CI with PG 16 service container for workspace tests

### Modified Capabilities
- `explorer-postgres-bridge`: Invert default — PG becomes primary path via `DATABASE_URL`, SQLite demoted to `--sqlite` opt-in flag

## Approach

**Feature flip + sqlite gate.** Pure Cargo feature plumbing — no new domain logic.

1. Workspace `Cargo.toml`: make `rusqlite` optional
2. Add `sqlite` feature per crate, make `postgres` default
3. Gate `rusqlite`/`SqliteGraphStore`/SQLite adapters behind `#[cfg(feature = "sqlite")]`
4. CLI: `DATABASE_URL` → PG path (default), `--sqlite` flag → SQLite path (opt-in)
5. Docker Compose + CI: PG 16 service, `DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode`
6. Test gating: SQLite tests require `--features sqlite`; PG tests run by default when `TEST_DATABASE_URL` is set

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `Cargo.toml` (root) | Modified | `rusqlite` optionality |
| `crates/cognicode-core/Cargo.toml` | Modified | `default = ["postgres"]`, new `sqlite` feature |
| `crates/cognicode-explorer/Cargo.toml` | Modified | `default = ["postgres"]`, new `sqlite` feature |
| `crates/cognicode-db/Cargo.toml` | Modified | `sqlite` feature gate |
| `crates/cognicode-explorer/src/bin/api.rs` | Modified | Flip CLI default to PG |
| `crates/cognicode-explorer/src/bin/mcp.rs` | Modified | Flip CLI default to PG |
| `crates/cognicode-explorer/src/service.rs` | Modified | PG-aware `open_workspace()` |
| `crates/cognicode-core/src/interface/mcp/handlers/aix_handlers.rs` | Modified | Gate raw `rusqlite` usage |
| `crates/cognicode-explorer/tests/integration.rs` | Modified | Gate behind `#[cfg(feature = "sqlite")]` |
| `crates/cognicode-explorer/tests/explorer_graph_foundation.rs` | Modified | Gate behind `#[cfg(feature = "sqlite")]` |
| `docker-compose.yml` | New | PG 16 for local dev |
| `.env.example` | New | `DATABASE_URL`, `TEST_DATABASE_URL` |
| `.github/workflows/ci.yml` | New | CI with PG 16 service |
| `justfile` | Modified | `dev-pg`, `test-pg` targets |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `aix_handlers.rs` raw `rusqlite` breaks when `sqlite` off | Med | Gate all rusqlite usage in core behind `#[cfg(feature = "sqlite")]` |
| `cognicode-mcp`/`cognicode-quality` break without `sqlite` | Med | Propagate `sqlite` feature through dep chain |
| No existing CI — must build from scratch | Low | Start minimal: checkout, Rust, PG service, `cargo test` |
| Developer friction requiring local PG | Low | Docker Compose makes PG one command away; `--features sqlite` restores old behavior |

## Rollback Plan

`cargo build --no-default-features --features sqlite` restores exact pre-change behavior. Feature flags are additive — removing `postgres` from defaults returns to SQLite-only compilation. Docker Compose and CI are new files with zero impact on existing code.

## Dependencies

- Rust toolchain (already present)
- Docker for local PG (new dev dependency)
- GitHub Actions for CI (new)

## Success Criteria

- [ ] `cargo build` compiles with PG as default, no `rusqlite` in dep tree (without `--features sqlite`)
- [ ] `cargo test` runs PG tests when `TEST_DATABASE_URL` is set, skips gracefully when unset
- [ ] `cargo build --no-default-features --features sqlite` restores SQLite-only behavior
- [ ] CI passes: `cargo test --workspace` with PG 16 service container
- [ ] `docker compose up -d` starts PG and app connects via `DATABASE_URL`
