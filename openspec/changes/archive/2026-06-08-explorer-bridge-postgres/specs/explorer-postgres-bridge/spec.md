# Delta for explorer-postgres-bridge

> New capability from `sdd/explorer-bridge-postgres/proposal` (In-Memory Bridge). Additive — no existing specs change.

## Purpose

Wire `cognicode-explorer-api` and `cognicode-explorer-mcp` to load a `CallGraph` from PG at binary startup, wrap in `CallGraphRepository`, serve all 4 ports with no trait or method changes. SQLite stays default.

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

Both binaries MUST accept `--postgres <DATABASE_URL>` under `#[cfg(feature = "postgres")]`. Present → call `open_graph_from_postgres(url).await?` and skip SQLite. Absent → byte-identical to today.

#### Scenario: `--postgres` on API binary serves PG data

- GIVEN populated PG
- WHEN API binary launches with `--postgres postgres://u:p@h/db`
- THEN `GET /symbols/…` returns PG-sourced data (asserted via known FQN); SQLite never opens

#### Scenario: `--postgres` on MCP binary serves PG data

- GIVEN populated PG
- WHEN MCP binary launches with `--postgres postgres://u:p@h/db`
- THEN `ExplorerMcpHandler` resolves PG-sourced symbols/edges on stdio (asserted via known FQN)

#### Scenario: Flag absent preserves SQLite path

- GIVEN workspace with valid `.cognicode/cognicode.db`
- WHEN binary launches with no `--postgres`
- THEN `open_graph(&db_path)` runs unchanged; no `sqlx` connection opens

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

## Acceptance Criteria

1. `cargo check -p cognicode-explorer` (no feature) — CLI
2. `cargo check -p cognicode-explorer --features postgres` — CLI
3. Helper round-trips 5+/3+/3-provenance fixture bit-exact — contract test
4. Both binaries accept `--postgres` and serve PG data on existing ports — integration test
5. SQLite default path byte-identical to pre-change — regression test
6. `MetadataAwareRepository` returns identical metadata from SQLite or PG — contract test

## Edge Cases

- **PG unreachable at startup** — helper `Err`; binary exits non-zero. No retry.
- **Empty PG (both tables empty)** — `Ok(Arc::new(CallGraph::new()))`; binaries serve empty.
- **Feature absent + `--postgres` passed** — clap rejects unknown flag (fails fast).
- **Migrations on every startup** — idempotent (`IF NOT EXISTS`), no schema drift.
- **Pool exhaustion / query timeout** — `Err` propagates; binary exits non-zero. No silent fallback when `--postgres` is set.
- **Symbols but no edges (or vice versa)** — `load_call_graph` returns `Some(graph)` (`None` requires BOTH tables empty).

## Out of Scope (locked)

New `PostgresExplorerRepository` adapter · `SymbolRepository` / `MetadataAwareRepository` trait changes · write-path from explorer (read-only by design) · incremental / live queries (full-graph load only) · schema changes · FTS5/quality adapter rewiring for PG (FTS5 stays SQLite-only) · `ltree` / `pgvector` / bincode sidecar persistence · PR size budget ≤ 400 added+deleted lines (chained PRs if forecast crosses threshold).
