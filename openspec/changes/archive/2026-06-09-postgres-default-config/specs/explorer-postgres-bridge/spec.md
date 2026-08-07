# Delta for explorer-postgres-bridge

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

## ADDED Requirements

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

## Edge Cases

- **`DATABASE_URL` set to an empty string** — treated as unset; binary fails fast.
- **`--postgres` and `--sqlite` both passed** — clap-level conflict; binary rejects the invocation.
- **PG unreachable at startup with `DATABASE_URL` set** — helper returns `Err`; binary exits non-zero. No retry, no silent fallback.
- **PG feature off + `DATABASE_URL` set** — explicit error: "postgres feature not enabled". No silent SQLite fallback (would mask the misconfiguration).
- **Empty PG (both tables empty) + `DATABASE_URL`** — `Ok(Arc::new(CallGraph::new()))`; binaries serve empty.
- **Symbols but no edges (or vice versa) + `DATABASE_URL`** — `load_call_graph` returns `Some(graph)`.
