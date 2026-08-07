# ci-postgres-pipeline Specification

## Purpose

Provide a reproducible CI pipeline and local development environment for PostgreSQL 16. The pipeline MUST start a PG 16 service, expose a `TEST_DATABASE_URL`, and run `cargo test --workspace` with default features.

## Requirements

### Requirement: `docker-compose.yml` defines a PG 16 service

The workspace root MUST contain a `docker-compose.yml` declaring a `postgres` service: `postgres:16-alpine`, environment `POSTGRES_USER=cognicode`, `POSTGRES_PASSWORD=cognicode`, `POSTGRES_DB=cognicode`, port `5432:5432`, named volume `cognicode_pg_data`, and `healthcheck` using `pg_isready -U cognicode`.

#### Scenario: Local dev stack starts cleanly

- GIVEN Docker installed
- WHEN `docker compose up -d postgres` runs
- THEN the container reaches `healthy` within 30s and `psql postgres://cognicode:cognicode@localhost:5432/cognicode -c '\l'` lists `cognicode` database

#### Scenario: Restart preserves data

- GIVEN the container ran once and seeded data
- WHEN `docker compose down && docker compose up -d postgres` runs
- THEN the `cognicode` database still exists and seeded rows are intact

### Requirement: `.env.example` documents required env vars

The workspace root MUST contain `.env.example` with `DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode` and `TEST_DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode`. Real `.env` MUST be in `.gitignore`.

#### Scenario: Dev onboarding reads .env.example

- GIVEN a fresh clone
- WHEN `cp .env.example .env` runs
- THEN `cargo run` and `cargo test` pick up the URL via `std::env::var`

### Requirement: `.github/workflows/ci.yml` runs the workspace test matrix

The CI workflow MUST: (a) trigger on `push` and `pull_request` to `main`; (b) run on `ubuntu-latest`; (c) start a `postgres:16` service with the same env vars as docker-compose; (d) cache `~/.cargo` and the workspace `target/`; (e) run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. The workflow MUST export `TEST_DATABASE_URL` pointing at the service.

#### Scenario: Push to main triggers the workflow

- GIVEN a commit on `main`
- WHEN GitHub Actions runs
- THEN the workflow executes all four jobs (fmt, clippy, test, build) and passes

#### Scenario: PG service is reachable from the runner

- GIVEN the workflow's `services.postgres` block
- WHEN any `cargo test` step runs
- THEN `TEST_DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode` connects to the live service (healthcheck passes before the test step)

#### Scenario: Cached build shortens re-runs

- GIVEN a previous successful run cached `target/`
- WHEN a new commit triggers the workflow
- THEN the second run completes in <60% of the cold-run wall time

### Requirement: `justfile` exposes `dev-pg` and `test-pg` recipes

The workspace `justfile` MUST define:
- `dev-pg`: starts the docker-compose stack, waits for healthcheck, prints `DATABASE_URL`.
- `test-pg`: exports `TEST_DATABASE_URL`, runs `cargo test --workspace` with default features.

#### Scenario: `just dev-pg` reaches healthy state

- GIVEN Docker running
- WHEN `just dev-pg` runs
- THEN within 30s the recipe prints "ready" and exits 0

#### Scenario: `just test-pg` runs the PG test set

- GIVEN `just dev-pg` already brought up the stack
- WHEN `just test-pg` runs
- THEN PG-backed tests execute; SQLite-only tests are skipped (feature absent); exit code 0 on success

### Requirement: Test gating respects `TEST_DATABASE_URL`

Tests that require PG MUST check `TEST_DATABASE_URL` at the top of each test function (or via a `#[ignore]` + manual run pattern). If unset, the test MUST `eprintln!` a skip message and return `Ok(())` rather than failing.

#### Scenario: PG test skips when env var absent

- GIVEN no `TEST_DATABASE_URL` in env
- WHEN `cargo test --workspace` runs
- THEN PG-dependent tests print a skip notice and the overall test run exits 0

#### Scenario: PG test runs when env var present

- GIVEN `TEST_DATABASE_URL=postgres://…` set
- WHEN `cargo test --workspace` runs against a live PG
- THEN PG-dependent tests execute their assertions

## TDD RED Gate (must fail before this spec is implemented)

1. No `docker-compose.yml` exists → `docker compose config` exits non-zero.
2. No `.github/workflows/ci.yml` exists → first CI push fails with "no workflow found".
3. A test `test_justfile_recipes_defined` parses `justfile` and asserts both `dev-pg` and `test-pg` recipes exist.
4. A test `test_ci_workflow_declares_pg_service` parses `.github/workflows/ci.yml` as YAML and asserts the `services.postgres` block with `image: postgres:16`.

## Acceptance Criteria

1. `docker compose up -d` reaches healthy within 30s.
2. `cargo test --workspace` passes locally with the stack up.
3. CI workflow runs all checks and is green on `main`.
4. `just dev-pg` and `just test-pg` are documented in the `justfile` header.

## Edge Cases

- **Port 5432 already in use** — `docker compose up` fails loudly; user must stop the conflicting process.
- **CI cache corruption** — first run after corruption falls back to cold build; no silent failure.
- **PG 16 minor version drift** — `postgres:16-alpine` is floating; tests must not depend on patch-level behavior.
- **`.env` accidentally committed** — `.gitignore` blocks it; CI never reads it.

## Out of Scope (locked)

Multi-version PG matrix (PG 15, 17) · load testing in CI · deployment/release pipelines · container image publishing · staging environment.
