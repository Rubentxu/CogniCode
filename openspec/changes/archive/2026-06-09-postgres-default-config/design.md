# Design: PostgreSQL Default Configuration

## Technical Approach

Invert the dependency direction without changing domain logic. Three coordinated changes: (1) flip the Cargo feature default from implicit (`rusqlite` always-on) to explicit (`postgres` default, `sqlite` opt-in), (2) flip the CLI precedence from `--postgres` flag to `DATABASE_URL` env var, (3) wire up local-dev + CI infrastructure (docker-compose, CI workflow PG service, justfile recipes). The existing `cognicode-explorer/src/postgres_bridge.rs::open_graph_from_postgres` and `ExplorerService::open_workspace` are the integration points; we modify their callers (api.rs, mcp.rs) and gate their `rusqlite` peers in `cognicode-core` and `cognicode-db`.

## Architecture Decisions

| # | Decision | Choice | Alternatives considered | Rationale |
|---|----------|--------|--------------------------|-----------|
| 1 | Default feature on `cognicode-core` | `default = ["postgres"]` | Make `persistence` default (no PG/SQLite) | PG is the default backend per the proposal; `persistence` alone is incomplete and the spec is silent on a no-backend build. |
| 2 | `rusqlite` location in workspace `Cargo.toml` | Keep in `[workspace.dependencies]`, mark optional via `optional = true` on consumers | Remove entirely; add only in `cognicode-db` | Removing it breaks the future `--features sqlite` opt-in. Keeping the workspace dep avoids the `cognicode-db` consumers from having to declare it locally. |
| 3 | CLI precedence | `--postgres <URL>` > `DATABASE_URL` env > `--sqlite` > fail | `DATABASE_URL` > `--postgres` > `--sqlite` | The flag is the strongest signal of intent (explicit user action beats implicit env). This preserves the existing `explorer-postgres-bridge` contract. |
| 4 | PG-feature-off + env-set behavior | Fail fast with explicit error | Silently fall back to SQLite | Silent fallback masks misconfiguration. The spec mandates an explicit error. |
| 5 | `cognicode-db` feature shape | `sqlite = ["dep:rusqlite"]` | `sqlite = []` and gate `rusqlite` only in source | Cargo's `dep:rusqlite` syntax makes the dep explicitly optional; cleaner. |
| 6 | Feature propagation | Cargo's `crate/feature` syntax (`cognicode-core/sqlite`) | Manual re-export in each downstream crate | Native forwarding keeps the dependency graph honest. |
| 7 | Test gating for SQLite tests | File-level `#![cfg(feature = "sqlite")]` | Per-test `#[cfg]` | File-level is one line per file, harder to forget, matches the proposal. |
| 8 | CI runner | Existing `.github/workflows/ci.yml` extended with a `postgres` service | New workflow file | Extension preserves the existing test/e2e/UI jobs. |
| 9 | Local dev stack | `docker-compose.yml` (PG 16 alpine) | Single `Dockerfile` for the app + external PG | docker-compose is the lowest-friction local dev path. |
| 10 | `DATABASE_URL` reading in binaries | Inline at startup in `bin/api.rs` and `bin/mcp.rs` (3 lines) | New `open_workspace()` helper that owns dispatch | Inline keeps the precedence logic visible and explicit; helper is over-engineering for a 3-arm match. |

## Data Flow

### CLI startup (both binaries)

```
┌──────────────────────────────────────────────────────────────┐
│  bin startup                                                  │
│  1. parse args (clap)                                         │
│  2. resolve URL:                                              │
│       if --postgres <u>      → <u>                            │
│       else if DATABASE_URL   → $DATABASE_URL                  │
│       else if --sqlite       → SQLite path                    │
│       else                   → exit(2) "no URL / no --sqlite"│
│  3. cfg check: if (URL set) && !cfg(postgres) → exit(2)       │
│  4. dispatch:                                                 │
│       URL set   → open_graph_from_postgres(url)               │
│       SQLite    → open_graph(&db_path)                        │
└──────────────────────────────────────────────────────────────┘
```

### Cargo feature resolution

```
workspace: rusqlite = "0.31" (bundled)         # stays
cognicode-core:                                   cognicode-db:
  default = ["postgres"]                            default = []      # no rusqlite by default
  postgres  = ["dep:sqlx"]                          sqlite   = ["dep:rusqlite"]
  sqlite    = ["dep:rusqlite"]
cognicode-explorer:
  default = ["postgres"]                           # propagates core
  postgres  = ["cognicode-core/postgres", "dep:sqlx", "dep:async-trait"]
  sqlite    = ["cognicode-core/sqlite", "cognicode-db/sqlite"]
cognicode-mcp:                                    cognicode-quality:
  sqlite = ["cognicode-core/sqlite", "cognicode-db/sqlite"]   # downstream forwarding
```

### CI test gate

```
.github/workflows/ci.yml::test job
  services:
    postgres: { image: postgres:16, env: {POSTGRES_USER,POSTGRES_PASSWORD,POSTGRES_DB},
                ports: [5432:5432], options: --health-cmd pg_isready }
  env:
    TEST_DATABASE_URL: postgres://cognicode:cognicode@localhost:5432/cognicode
  steps:
    - cargo fmt --check
    - cargo clippy --workspace --all-targets -- -D warnings
    - cargo test --workspace
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `Cargo.toml` (root) | Modify | No change to `rusqlite` (stays workspace dep, used by `--features sqlite`). |
| `crates/cognicode-core/Cargo.toml` | Modify | Add `sqlite = ["dep:rusqlite"]`; change `default = ["persistence", "postgres"]`. |
| `crates/cognicode-explorer/Cargo.toml` | Modify | Add `sqlite = ["cognicode-core/sqlite", "cognicode-db?/sqlite"]`; change `default = ["postgres"]`; add `cognicode-db?/sqlite` propagation. |
| `crates/cognicode-db/Cargo.toml` | Modify | `default = []`; add `sqlite = ["dep:rusqlite"]`; mark `rusqlite.workspace = true, optional = true`. |
| `crates/cognicode-db/src/lib.rs` | Modify | Wrap SQLite `mod` declarations in `#[cfg(feature = "sqlite")]`. |
| `crates/cognicode-core/src/interface/mcp/handlers/aix_handlers.rs` | Modify | Wrap `fn open_db` and all `rusqlite::params!`/`rusqlite::Connection::open` call sites in `#[cfg(feature = "sqlite")]`. |
| `crates/cognicode-mcp/Cargo.toml` | Modify | Add `sqlite = ["cognicode-core/sqlite", "cognicode-db?/sqlite"]`. |
| `crates/cognicode-quality/Cargo.toml` | Modify | Same forwarding as `cognicode-mcp`. |
| `crates/cognicode-explorer/src/bin/api.rs` | Modify | Add `--sqlite` flag (cfg-gated `#[cfg(feature = "sqlite")]`); add URL resolution block (precedence); pass resolved URL to `open_graph_from_postgres` or fall back to SQLite. |
| `crates/cognicode-explorer/src/bin/mcp.rs` | Modify | Same as `api.rs`. |
| `crates/cognicode-explorer/src/service.rs` | Modify | No signature change. `open_workspace` already returns metadata; ensure it surfaces `GraphStatus::Ready` for both paths. |
| `crates/cognicode-explorer/tests/integration.rs` | Modify | First line: `#![cfg(feature = "sqlite")]`. |
| `crates/cognicode-explorer/tests/explorer_graph_foundation.rs` | Modify | First line: `#![cfg(feature = "sqlite")]`. |
| `crates/cognicode-explorer/tests/cli_precedence.rs` | Create | TDD: assert CLI precedence (--postgres > DATABASE_URL > --sqlite > fail). |
| `docker-compose.yml` | Create | PG 16 service, healthcheck, named volume. |
| `.env.example` | Create | `DATABASE_URL`, `TEST_DATABASE_URL`. |
| `.gitignore` | Modify | Add `.env` line if not present. |
| `.github/workflows/ci.yml` | Modify | Add `services.postgres`, set `TEST_DATABASE_URL` env, switch `cargo test --all` → `cargo test --workspace` (already equivalent), add `cargo fmt --check` and `cargo clippy` steps. |
| `justfile` | Modify | Add `dev-pg` and `test-pg` recipes. |
| `crates/cognicode-core/tests/sqlite_default_excluded.rs` | Create | TDD: `cargo tree` check that no `rusqlite` line appears in the default build. |
| `crates/cognicode-db/tests/sqlite_default_excluded.rs` | Create | Same shape for `cognicode-db`. |

## Interfaces / Contracts

### URL resolution helper (new, lives in `cognicode-explorer/src/cli_dispatch.rs`)

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum Backend { Postgres(String), Sqlite }

#[cfg(feature = "postgres")]
pub fn resolve_backend(args: &Args, sqlite_flag: bool) -> Result<Backend, String> {
    if let Some(url) = &args.postgres {        // explicit flag wins
        return Ok(Backend::Postgres(url.clone()));
    }
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if !url.is_empty() { return Ok(Backend::Postgres(url)); }
    }
    #[cfg(feature = "sqlite")]
    if sqlite_flag { return Ok(Backend::Sqlite); }
    Err("DATABASE_URL not set and no --sqlite flag provided".into())
}
```

### CLI flag set per binary

| Flag | Feature gate | Default |
|------|--------------|---------|
| `--postgres <URL>` | `#[cfg(feature = "postgres")]` | absent (None) |
| `--sqlite` | `#[cfg(feature = "sqlite")]` | false |

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | URL resolution precedence | `cli_dispatch::tests` — 6 cases for the 4 scenarios + 2 edge cases (empty env, both flags). |
| Unit | Default build excludes rusqlite | `tests/sqlite_default_excluded.rs` runs `cargo metadata` against the workspace and asserts no `rusqlite` line is reachable in the default-feature resolution. |
| Unit | Service gating | `service::tests::open_workspace_returns_ready_for_pg` — mock `PostgresRepository`; `service::tests::open_workspace_returns_ready_for_sqlite` — uses tmp DB. |
| Integration | `--postgres` flag (unchanged) | Reuse existing `explorer-postgres-bridge` contract tests. |
| Integration | `--sqlite` opt-in | `tests/cli_precedence.rs::with_sqlite_flag_loads_local_db` (requires `--features sqlite`). |
| E2E | docker-compose stack reachable | `scripts/dev_pg_healthcheck.sh` (new) — `pg_isready` loop, called by `just dev-pg`. |
| CI | fmt + clippy + test | `.github/workflows/ci.yml` — three jobs, all green on `main`. |

## Migration / Rollout

**Phased** (matches the proposal's success criteria; each step is independently shippable):

1. **Feature gate only** — `sqlite` feature added, `cognicode-core` default stays as-is. Verify `cargo build --no-default-features --features sqlite` matches pre-change build.
2. **Flip defaults** — `default = ["postgres"]` on `cognicode-core` and `cognicode-explorer`. Verify `cargo build` succeeds without `rusqlite`.
3. **Flip CLI** — `DATABASE_URL` precedence. Verify both binaries.
4. **Wire CI** — add PG service, run tests.
5. **Wire docker-compose** — local dev recipe.

Rollback at any point: `cargo build --no-default-features --features sqlite`. No data migration; SQLite remains opt-in.

## Open Questions

- **None blocking.** The proposal's rollback plan (`--no-default-features --features sqlite`) covers any combination of partial rollout.

## Risks & Mitigations (carried from proposal)

| Risk | Mitigation in this design |
|------|---------------------------|
| `aix_handlers.rs` raw `rusqlite` breaks when `sqlite` off | Per-call-site `#[cfg(feature = "sqlite")]` plus a `compile_error!` guard at the top of the file: `#![cfg_attr(not(feature = "sqlite"), allow(unused_imports))]` plus a stub `open_db` returning `Err("sqlite feature disabled")`. |
| `cognicode-mcp`/`cognicode-quality` break without `sqlite` | Forwarding features (`sqlite = ["cognicode-core/sqlite", "cognicode-db?/sqlite"]`) propagate the gate. |
| No existing CI infra for PG | Extend the existing `test` job in `.github/workflows/ci.yml` with a `services.postgres` block (already in `ci.yml`). |
| Developer friction requiring local PG | `just dev-pg` brings the stack up; `--features sqlite` restores the legacy build for users who want it. |
