# Tasks: PostgreSQL Default Configuration

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 350–500 (Cargo.toml ×4, 1 helper file, 2 binaries, aix_handlers gates, 2 test files, infra ×3) |
| 400-line budget risk | Medium |
| Chained PRs recommended | Yes |
| Suggested split | PR 1: feature gate only · PR 2: flip defaults + CLI · PR 3: CI + docker-compose |
| Delivery strategy | auto-chain |
| Chain strategy | stacked-to-main |

```
Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: Medium
```

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Add `sqlite` opt-in feature across crates; aix_handlers gates; SQLite tests gated. SQLite is still default. | PR 1 | Stack 1, additive — no behavior change. |
| 2 | Flip defaults to `["postgres"]`; introduce `cli_dispatch::resolve_backend`; flip CLI in both binaries. | PR 2 | Depends on PR 1. SQLite remains opt-in. |
| 3 | docker-compose + .env.example + CI service + justfile recipes. | PR 3 | Depends on PR 2. No code change beyond env. |

## Phase 1: Foundation — Feature Gate (PR 1)

- [ ] 1.1 Write `crates/cognicode-core/tests/sqlite_default_excluded.rs` (RED) — `cargo metadata` snapshot asserts no `rusqlite` in default-feature resolution. Test must FAIL because core currently has `rusqlite` as non-optional.
- [ ] 1.2 Write `crates/cognicode-db/tests/sqlite_default_excluded.rs` (RED) — same shape for `cognicode-db`. Test must FAIL.
- [ ] 1.3 In `crates/cognicode-core/Cargo.toml`: mark `rusqlite` `optional = true`; add `sqlite = ["dep:rusqlite"]` feature. Keep `default = ["persistence"]` (PG flip is PR 2).
- [x] 1.4 In `crates/cognicode-core/src/interface/mcp/handlers/aix_handlers.rs`: gate `fn open_db` body and every `rusqlite::params!` / `Connection::open` call site with `#[cfg(feature = "sqlite")]`. Provide a stub `open_db` under `#[cfg(not(feature = "sqlite"))]` returning `Err("sqlite feature disabled")`.
- [ ] 1.5 In `crates/cognicode-db/Cargo.toml`: `default = []`; mark `rusqlite` optional; add `sqlite = ["dep:rusqlite"]`.
- [ ] 1.6 In `crates/cognicode-db/src/lib.rs`: wrap SQLite `mod` declarations in `#[cfg(feature = "sqlite")]`.
- [ ] 1.7 In `crates/cognicode-explorer/Cargo.toml`: add `sqlite = ["cognicode-core/sqlite", "cognicode-db?/sqlite"]`.
- [ ] 1.8 In `crates/cognicode-mcp/Cargo.toml` and `crates/cognicode-quality/Cargo.toml`: add `sqlite = ["cognicode-core/sqlite", "cognicode-db?/sqlite"]`.
- [ ] 1.9 In `crates/cognicode-explorer/tests/integration.rs` and `tests/explorer_graph_foundation.rs`: add `#![cfg(feature = "sqlite")]` as first line.
- [ ] 1.10 GREEN: re-run tests from 1.1 and 1.2 — `cargo test -p cognicode-core --no-default-features` exits 0; `cargo tree | grep -q rusqlite` exits 1.
- [ ] 1.11 REFACTOR: confirm `cargo build --no-default-features --features sqlite` byte-identical to pre-change build.

## Phase 2: Core Implementation — Default Flip + CLI (PR 2)

- [ ] 2.1 Write `crates/cognicode-explorer/src/cli_dispatch.rs` with `enum Backend { Postgres(String), Sqlite }` and `pub fn resolve_backend(args_postgres: Option<&str>, sqlite_flag: bool) -> Result<Backend, String>`. Implement 4 precedence cases (RED: tests below fail because helper does not exist).
- [ ] 2.2 Write `crates/cognicode-explorer/src/cli_dispatch.rs::tests` covering 6 cases: --postgres wins · DATABASE_URL wins · --sqlite opts out · empty env treated as unset · both flags → clap conflict surfaced upstream · no flag + no env → Err.
- [ ] 2.3 In `crates/cognicode-explorer/Cargo.toml`: change `default = ["postgres"]`; add `default-run = ["postgres"]` if needed for the binaries. Keep `sqlite` opt-in.
- [x] 2.4 In `crates/cognicode-core/Cargo.toml`: change `default = ["persistence", "postgres"]`.
- [ ] 2.5 In `crates/cognicode-explorer/src/bin/api.rs`: import `resolve_backend`; add `#[cfg(feature = "sqlite")] #[arg(long)] sqlite: bool` field; replace the existing cfg-dispatch with a single call to `resolve_backend(&args.postgres, args.sqlite)`; PG-feature-off + URL-set → exit(2) "postgres feature not enabled".
- [ ] 2.6 In `crates/cognicode-explorer/src/bin/mcp.rs`: same edit as 2.5.
- [ ] 2.7 GREEN: run `cli_dispatch::tests` (6/6 pass); run `cargo build` and assert no `rusqlite` in dep graph; `cargo build --no-default-features --features sqlite` still works.
- [ ] 2.8 Write `crates/cognicode-explorer/tests/cli_precedence.rs` (integration, runs only with `--features sqlite`): 4 sub-tests covering --postgres + DATABASE_URL → --postgres wins; DATABASE_URL alone → PG; --sqlite alone → SQLite; nothing → exit code 2.
- [ ] 2.9 REFACTOR: ensure `--help` text on both binaries includes the precedence line (manually edit the `///` doc comments on the args).
- [ ] 2.10 GREEN: re-run full `cargo test --workspace`; assert `cognicode-explorer-postgres-bridge` contract tests (5 sym / 3 dep / 3 prov / confidences 0.0,0.5,1.0) still pass.

## Phase 3: Integration — CI + Local Dev (PR 3)

- [ ] 3.1 Create `docker-compose.yml` at workspace root: `postgres:16-alpine`, env `POSTGRES_USER=cognicode`, `POSTGRES_PASSWORD=cognicode`, `POSTGRES_DB=cognicode`, port 5432, named volume `cognicode_pg_data`, healthcheck `pg_isready -U cognicode`.
- [ ] 3.2 Create `.env.example` with `DATABASE_URL` and `TEST_DATABASE_URL` pointing at `postgres://cognicode:cognicode@localhost:5432/cognicode`.
- [ ] 3.3 Add `.env` to `.gitignore` (create if missing).
- [ ] 3.4 In `.github/workflows/ci.yml::test` job: add `services.postgres` block (image `postgres:16`, env, ports, options `--health-cmd pg_isready --health-interval 10s --health-timeout 5s --health-retries 5`); add env `TEST_DATABASE_URL: postgres://cognicode:cognicode@localhost:5432/cognicode`; add `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` steps; switch `cargo test --all` to `cargo test --workspace`.
- [ ] 3.5 In `justfile`: add `dev-pg` recipe (docker compose up -d, wait loop on `pg_isready`, print DATABASE_URL); add `test-pg` recipe (export TEST_DATABASE_URL, run `cargo test --workspace`).
- [ ] 3.6 Write `crates/cognicode-explorer/tests/test_gating_skips_when_no_env.rs` (RED): assert that when `TEST_DATABASE_URL` is unset the test prints a skip notice and exits 0. Test must FAIL before 3.4 lands.
- [ ] 3.7 GREEN: re-run `cargo test --workspace` locally with stack up; verify `cargo test --workspace` with stack down (skip path); run `just dev-pg` and `just test-pg` manually.
- [ ] 3.8 Verify CI: push branch, watch `.github/workflows/ci.yml::test` go green end-to-end (fmt + clippy + test + PG service healthy).
- [ ] 3.9 REFACTOR: tighten docker-compose healthcheck `interval` to 5s for faster local feedback; update justfile comment to mention `set dotenv-load` already exports `.env`.

## Phase 4: Cleanup

- [ ] 4.1 Update `openspec/specs/explorer-postgres-bridge/spec.md` archive annotation referencing the CLI flip (post-archive).
- [ ] 4.2 Remove any temporary `Cargo.lock` entries left by feature gating (run `cargo update`).
- [ ] 4.3 Update `docs/` developer onboarding with `just dev-pg` quick-start.
- [ ] 4.4 Commit each PR with work-unit message: `feat(db): gate sqlite behind opt-in feature`, `feat(cli): default DATABASE_URL over --postgres`, `chore(infra): postgres ci pipeline + docker-compose`.
