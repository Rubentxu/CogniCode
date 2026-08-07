# Delta for explorer-postgres-bridge (OBSOLETE — 2026-08-04)

> **Status: OBSOLETE** — PostgreSQL removed from stack (ADR-026, e29-7).
> The bridge from explorer to Postgres no longer exists. Archive this spec.

> New capability from `sdd/explorer-bridge-postgres/proposal` (In-Memory Bridge). Modified by `sdd/postgres-default-config` to invert the default: PG via `DATABASE_URL` is now the default, SQLite is opt-in.

## Purpose

> **Reconciliation note (2026-08-01)**: the `save_call_graph` /
> `load_call_graph` inherent methods on `PostgresRepository` referenced in
> this spec were the **pre-Phase-0 surface**. The e29-0-define-new-ports +
> e29-0-refactor-call-sites changes relocated them behind the
> `CallGraphStore` domain port (with the `_ws` suffix):
>
> - `PostgresRepository::save_call_graph(&self, graph)` →
>   `CallGraphStore::save_call_graph_ws(&self, graph, ws)`
> - `PostgresRepository::load_call_graph(&self)` →
>   `CallGraphStore::load_call_graph_ws(&self, ws, rev)` or
>   `CallGraphStore::load_call_graph_current(&self, ws)`
>
> The **contract** (workspace-scoped, atomic per revision, idempotent re-save)
> is unchanged — only the port path changed. The pre-Phase-0
> `PostgresRepository` inherent method names remain in the concrete adapter
> as pass-through delegates to the new port.


Wire `cognicode-explorer-api` and `cognicode-explorer-mcp` to load a `CallGraph` from PG at binary startup, wrap in `CallGraphRepository`, serve all 4 ports with no trait or method changes. PostgreSQL is the default; SQLite is opt-in.

## ADDED Requirements

### Requirement: `open_graph_from_postgres` helper

`pub async fn open_graph_from_postgres(database_url: &str) -> Result<Arc<CallGraph>, ExplorerError>` MUST live in `cognicode-explorer` under `#[cfg(feature = "postgres")]`, reachable from both binaries. Body: `PostgresRepository::new(url).await?` → `load_call_graph().await?` → `Arc::new(graph)` for `Some`, `Arc::new(CallGraph::new())` for `None`, propagate `Err`. MUST NOT retain `PgPool`.

#### Scenario: Populated PG wraps `Some(graph)` bit-exact

- GIVEN PG with 7 sym / 12 edge mixed-provenance `CallGraph`
- WHEN the helper awaits
- THEN `Ok(Arc<CallGraph>)`, counts `7`/`12`, every `(provenance, confidence)` round-trips bit-exact

#### Scenario: Empty PG yields empty graph

- GIVEN zero rows in both tables
- WHEN the helper awaits
- THEN `Ok(Arc::new(CallGraph::new()))`, counts `0`/`0`

#### Scenario: PG connect failure surfaces as `Err`

- GIVEN unreachable `database_url`
- WHEN the helper awaits
- THEN `Err(ExplorerError::…("open_graph_from_postgres: connect: …"))` and process exits non-zero

### Requirement: `--postgres <URL>` CLI flag on both binaries

Both binaries MUST accept `--postgres <DATABASE_URL>` under `#[cfg(feature = "postgres")]`. Present → call `open_graph_from_postgres(url).await?` and skip SQLite. Absent → falls through to `DATABASE_URL` precedence.

#### Scenario: `--postgres` on API binary serves PG data

- GIVEN populated PG
- WHEN API binary launches with `--postgres postgres://u:p@h/db`
- THEN `GET /symbols/…` returns PG-sourced data (asserted via known FQN); SQLite never opens

#### Scenario: `--postgres` on MCP binary serves PG data

- GIVEN populated PG
- WHEN MCP binary launches with `--postgres postgres://u:p@h/db`
- THEN `ExplorerMcpHandler` resolves PG-sourced symbols/edges on stdio (asserted via known FQN)

#### Scenario: Flag absent falls through to DATABASE_URL

- GIVEN `DATABASE_URL` set in environment
- WHEN binary launches with no `--postgres`
- THEN `open_graph_from_postgres(DATABASE_URL)` runs; SQLite never opens

### Requirement: `postgres` feature gate on the crate

`crates/cognicode-explorer/Cargo.toml` MUST define `postgres` feature enabling `cognicode-core/postgres`, `sqlx` (`postgres`, `runtime-tokio`, `macros`), required `tokio` extras. New code MUST be `#[cfg(feature = "postgres")]`-gated. Default build MUST NOT pull `sqlx`.

#### Scenario: Default build stays sqlx-free

- GIVEN clean workspace
- WHEN `cargo check -p cognicode-explorer` runs (no feature)
- THEN build succeeds, `sqlx` absent, helper unreachable

#### Scenario: Feature-enabled build exposes the helper

- GIVEN `--features postgres`
- WHEN `cargo check -p cognicode-explorer --features postgres` runs
- THEN helper is reachable and both binaries accept `--postgres`

### Requirement: `MetadataAwareRepository` metadata preserved through bridge

Wrapped graph MUST surface edge metadata (provenance + confidence) bit-exact through `MetadataAwareRepository`.

#### Scenario: `callees_with_metadata` round-trips per-edge pairs

- GIVEN PG with edges `(Extracted,1.0)`, `(Inferred,0.7)`, `(Ambiguous,0.3)`
- WHEN `CallGraphRepository::callees_with_metadata(&id)` is called on the wrapped graph
- THEN every tuple matches the source pair (order unspecified)

### Requirement: Contract test — PG → `CallGraphRepository` round-trip

Test under `#[cfg(all(test, feature = "postgres"))]` MUST seed PG (≥5 sym, ≥3 dep types, all 3 provenance variants, confidences `{0.0, 0.5, 1.0}`), call helper, wrap in `CallGraphRepository`, assert `assert_eq!(g_source, g_loaded)` and every lookup matches PG. MUST use `#[sqlx::test]`.

#### Scenario: Round-trip equality passes

- GIVEN the fixture
- WHEN the test runs
- THEN `assert_eq!(g_source, g_loaded)` passes; all lookups return matching `Symbol`/`EdgeMetadata`

#### Scenario: Parallel tests stay isolated

- GIVEN two `#[sqlx::test]` functions with disjoint seeds
- WHEN they run in parallel
- THEN each sees only its own rows; both assertion sets pass

## MODIFIED Requirements

### Requirement: CLI default is PostgreSQL via `DATABASE_URL`

Both `cognicode-explorer-api` and `cognicode-explorer-mcp` binaries MUST default to the PostgreSQL path. The startup precedence MUST be: (1) explicit `--postgres <URL>` flag wins if present, (2) else `DATABASE_URL` env var is used, (3) else `--sqlite` flag triggers the legacy SQLite path. With no flags and no env var, the binary MUST fail fast with a clear error.

(Previously: `--postgres <URL>` was the explicit opt-in; SQLite was the default and the `--postgres` flag selected the PG path.)

#### Scenario: DATABASE_URL selects PG on api binary

- GIVEN `DATABASE_URL=postgres://u:p@localhost:5432/db` in env
- WHEN the API binary launches with no extra flags
- THEN the helper `open_graph_from_postgres(url).await?` runs, SQLite never opens, and the API serves PG-sourced data

#### Scenario: DATABASE_URL selects PG on mcp binary

- GIVEN `DATABASE_URL=postgres://u:p@localhost:5432/db` in env
- WHEN the MCP binary launches with no extra flags
- THEN `ExplorerMcpHandler` resolves PG-sourced symbols/edges on stdio

#### Scenario: --sqlite flag opts out of PG default

- GIVEN `DATABASE_URL` is unset and workspace has valid `.cognicode/cognicode.db`
- WHEN either binary launches with `--sqlite`
- THEN the legacy `open_graph(&db_path)` path runs and `sqlx` never connects

#### Scenario: No env and no flag fails fast

- GIVEN `DATABASE_URL` unset and no `--sqlite` flag
- WHEN either binary launches
- THEN the process exits non-zero with "DATABASE_URL not set and no --sqlite flag provided"

#### Scenario: --postgres flag overrides DATABASE_URL

- GIVEN `DATABASE_URL=postgres://wrong/db` and `--postgres postgres://right/db` both present
- WHEN the binary launches
- THEN the explicit `--postgres` URL takes precedence (documented precedence order)

### Requirement: `open_workspace()` is PG-aware

`crates/cognicode-explorer/src/service.rs::open_workspace()` MUST dispatch on the same precedence as the CLI flag: explicit flag > `DATABASE_URL` env > `--sqlite` opt-in. The function signature MUST remain `pub async fn open_workspace(...) -> Result<Arc<CallGraph>, ExplorerError>` so call sites do not change.

(Previously: the function always opened SQLite via `open_graph(&db_path)`.)

#### Scenario: open_workspace reads DATABASE_URL

- GIVEN `DATABASE_URL` set
- WHEN `open_workspace()` is called from a test harness
- THEN the function returns an `Arc<CallGraph>` sourced from PG; SQLite is untouched

#### Scenario: open_workspace honors --sqlite

- GIVEN `--sqlite` flag set in the call context
- WHEN `open_workspace()` runs
- THEN the legacy SQLite path executes and the function returns an `Arc<CallGraph>` sourced from the local DB file

### Requirement: Pre-existing --postgres behavior is preserved

The `--postgres <URL>` flag MUST continue to work exactly as before. Its behavior is unchanged; only the default precedence shifts. The flag is still required to be gated behind `#[cfg(feature = "postgres")]` in the explorer crate.

(Previously: --postgres was the only way to enable the PG path. Now it is the highest-precedence override.)

#### Scenario: --postgres on API serves PG data (unchanged)

- GIVEN populated PG
- WHEN API binary launches with `--postgres postgres://u:p@h/db`
- THEN `GET /symbols/…` returns PG-sourced data; SQLite never opens

#### Scenario: --postgres on MCP serves PG data (unchanged)

- GIVEN populated PG
- WHEN MCP binary launches with `--postgres postgres://u:p@h/db`
- THEN `ExplorerMcpHandler` resolves PG-sourced symbols/edges on stdio

### Requirement: Default build still ships without sqlx (unchanged)

`crates/cognicode-explorer/Cargo.toml` MUST keep `postgres` as opt-in OR as default — this change does not require the default build to gain `sqlx` if it already excludes it. The new behavior is achieved by reading `DATABASE_URL` only when the `postgres` feature is on. If `postgres` is off and `DATABASE_URL` is set, the binary MUST exit with a clear "postgres feature not enabled" error rather than silently fall back to SQLite.

(Previously: explicit feature gating determined availability; now default-precedence logic also reads from env.)

#### Scenario: Default build without postgres feature rejects DATABASE_URL

- GIVEN the `postgres` feature is OFF
- WHEN a binary launches with `DATABASE_URL` set
- THEN the process exits non-zero with "postgres feature not enabled; rebuild with --features postgres"

#### Scenario: --sqlite still works when postgres feature is off

- GIVEN the `postgres` feature is OFF
- WHEN a binary launches with `--sqlite` and no env
- THEN the legacy SQLite path runs (the `postgres` feature is not required for the SQLite path)

## ADDED Requirements (postgres-default-config)

### Requirement: CLI flag/env documentation

Both binary `--help` output MUST document the precedence: `--postgres > DATABASE_URL > --sqlite`. The help text MUST explicitly state that no flag and no env var is a fatal startup error.

#### Scenario: --help shows precedence order

- GIVEN either binary
- WHEN `--help` is passed
- THEN the output contains a line: "Precedence: --postgres > DATABASE_URL > --sqlite"

## Acceptance Criteria

1. With `DATABASE_URL` set, both binaries serve PG data with no flags.
2. With `--postgres <url>` and `DATABASE_URL` set, `--postgres` wins.
3. With `--sqlite` and no env, both binaries serve SQLite data.
4. With no flag and no env, both binaries fail fast with a clear error.
5. Existing `explorer-postgres-bridge` contract tests (5 sym / 3 dep types / 3 provenance / confidences 0.0, 0.5, 1.0) still pass on the PG path.
6. `cargo check -p cognicode-explorer` (no feature) — CLI
7. `cargo check -p cognicode-explorer --features postgres` — CLI
8. Helper round-trips 5+/3+/3-provenance fixture bit-exact — contract test
9. `MetadataAwareRepository` returns identical metadata from SQLite or PG — contract test

## Edge Cases

- **`DATABASE_URL` set to an empty string** — treated as unset; binary fails fast.
- **`--postgres` and `--sqlite` both passed** — clap-level conflict; binary rejects the invocation.
- **PG unreachable at startup with `DATABASE_URL` set** — helper returns `Err`; binary exits non-zero. No retry, no silent fallback.
- **PG feature off + `DATABASE_URL` set** — explicit error: "postgres feature not enabled". No silent SQLite fallback (would mask the misconfiguration).
- **Empty PG (both tables empty) + `DATABASE_URL`** — `Ok(Arc::new(CallGraph::new()))`; binaries serve empty.
- **Symbols but no edges (or vice versa) + `DATABASE_URL`** — `load_call_graph` returns `Some(graph)`.
- **Pool exhaustion / query timeout** — `Err` propagates; binary exits non-zero. No silent fallback when `--postgres` is set.
- **Migrations on every startup** — idempotent (`IF NOT EXISTS`), no schema drift.

## Out of Scope (locked)

New `PostgresExplorerRepository` adapter · `SymbolRepository` / `MetadataAwareRepository` trait changes · write-path from explorer (read-only by design) · incremental / live queries (full-graph load only) · schema changes · FTS5/quality adapter rewiring for PG (FTS5 stays SQLite-only) · `ltree` / `pgvector` / bincode sidecar persistence · PR size budget ≤ 400 added+deleted lines (chained PRs if forecast crosses threshold).
