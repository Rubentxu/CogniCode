# sqlite-feature-gate Specification

## Purpose

The SQLite backend (rusqlite, `SqliteGraphStore`, SQLite adapters) MUST be opt-in via a `sqlite` Cargo feature. Default builds MUST NOT pull `rusqlite` or link SQLite. This isolates the legacy backend from the default PostgreSQL pipeline.

## Requirements

### Requirement: `sqlite` feature flag exists per crate

`cognicode-core`, `cognicode-db`, and `cognicode-explorer` MUST each define a `sqlite` feature. The feature MUST enable `rusqlite` (with `bundled`) on the affected crates. The feature MUST NOT be in `default`.

#### Scenario: Default build excludes rusqlite

- GIVEN a clean workspace
- WHEN `cargo tree --format "{p} {f}"` runs against `cognicode-core` with no features
- THEN no line contains `rusqlite`

#### Scenario: Opt-in build pulls rusqlite

- GIVEN a clean workspace
- WHEN `cargo build -p cognicode-core --features sqlite` runs
- THEN `rusqlite` is present in the dep graph

### Requirement: All `rusqlite` usage is `#[cfg(feature = "sqlite")]`-gated

Every `use rusqlite::…`, every `SqliteGraphStore` struct, every SQLite adapter function, and every `pub` symbol that references `rusqlite` types MUST be guarded by `#[cfg(feature = "sqlite")]` (or the equivalent `#[cfg(all(..., feature = "sqlite"))]`). This applies across `cognicode-core`, `cognicode-db`, and `cognicode-explorer`.

#### Scenario: Core compiles without sqlite feature

- GIVEN the workspace after the gate
- WHEN `cargo check -p cognicode-core` runs (default features only)
- THEN compilation succeeds and no `rusqlite` symbol is reachable

#### Scenario: `aix_handlers.rs` raw rusqlite is gated

- GIVEN `crates/cognicode-core/src/interface/mcp/handlers/aix_handlers.rs` contains raw `rusqlite` usage
- WHEN `cargo check -p cognicode-core` runs without `--features sqlite`
- THEN the file's `rusqlite` blocks are excluded by cfg and the build succeeds

### Requirement: SQLite-only integration tests are feature-gated

`crates/cognicode-explorer/tests/integration.rs` and `crates/cognicode-explorer/tests/explorer_graph_foundation.rs` MUST be guarded with `#![cfg(feature = "sqlite")]` at the file level (or equivalent per-test cfg). Running `cargo test --workspace` with default features MUST NOT compile these files.

#### Scenario: Default test run skips SQLite tests

- GIVEN default features
- WHEN `cargo test --workspace --no-run` runs
- THEN neither `integration.rs` nor `explorer_graph_foundation.rs` is compiled

#### Scenario: Opt-in test run compiles SQLite tests

- GIVEN `--features sqlite` on `cognicode-explorer`
- WHEN `cargo test -p cognicode-explorer --features sqlite --no-run` runs
- THEN both files are compiled into the test binary

### Requirement: `cognicode-db` exposes `sqlite` feature without forcing it

`crates/cognicode-db/Cargo.toml` MUST declare `sqlite = ["dep:rusqlite"]` as opt-in. The crate's `lib.rs` MUST gate all SQLite module declarations behind `#[cfg(feature = "sqlite")]`. The crate MUST remain usable by `cognicode-core` even when `cognicode-db` itself is built without `sqlite`.

#### Scenario: cognicode-db default build has no rusqlite

- GIVEN a clean workspace
- WHEN `cargo build -p cognicode-db` runs (default features)
- THEN compilation succeeds and no `rusqlite` symbol is in the compiled crate

### Requirement: Feature propagation through dependent crates

`cognicode-mcp` and `cognicode-quality` (and any other workspace crate depending on `cognicode-core` or `cognicode-explorer`) MUST propagate the `sqlite` feature forward so consumers can opt in. Their `Cargo.toml` MUST list `sqlite = ["…/cognicode-core:sqlite"]` style feature forwarding.

#### Scenario: Downstream can opt in to sqlite end-to-end

- GIVEN a downstream consumer
- WHEN built with `--features cognicode-mcp/sqlite`
- THEN the full chain (`cognicode-mcp` → `cognicode-explorer` → `cognicode-core` → `cognicode-db`) resolves `rusqlite` and compiles

## TDD RED Gate (must fail before this spec is implemented)

The following tests MUST exist and fail until the gate is in place:

1. `cargo test -p cognicode-core --no-default-features` exits 0, AND `cargo tree -p cognicode-core | grep -q rusqlite` exits 1.
2. `cargo test --workspace --no-run` (default features) produces a test binary that contains NO `SqliteGraphStore` symbol.
3. A unit test `test_default_build_excludes_rusqlite` lives in each affected crate's `tests/` directory under `#[cfg(not(feature = "sqlite"))]` and asserts the symbol is absent via `cargo metadata` or build-script introspection.

## Acceptance Criteria

1. `cargo build` (default) → 0 errors, 0 warnings about unused `sqlite` cfg gates, no `rusqlite` in `cargo tree`.
2. `cargo build --no-default-features --features sqlite` → byte-identical compilation to pre-change state.
3. `cargo test --workspace` (default) → PG tests run when `TEST_DATABASE_URL` is set, skip gracefully when unset; SQLite tests are not compiled.
4. `cargo test -p cognicode-explorer --features sqlite` → SQLite tests compile and pass.

## Edge Cases

- **Workspace has stale `Cargo.lock`** — `cargo build` will resolve fresh; `rusqlite` enters the lockfile only when `--features sqlite` is passed.
- **Downstream crate forgets to forward feature** — build fails with "crate `X` does not have feature `sqlite`"; surfaced loudly, not silently.
- **Both `sqlite` and `postgres` enabled** — both backends compile; CLI flag (`--sqlite` vs `--postgres`/env) selects at runtime. No implicit preference.

## Out of Scope (locked)

SQLite-to-PostgreSQL data migration · `cognicode-db` split into separate crates · removing `rusqlite` from the workspace entirely (it remains, gated) · PG version compatibility beyond 16.
