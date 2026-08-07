# Tasks: explorer-bridge-postgres

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~160-200 (1 new file ~30, 2 bin mods ~25, Cargo.toml ~8, 1 new test file ~80) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | auto-chain |
| Chain strategy | size-exception |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | PG bridge: feature gate, helper, CLI flags on api/mcp, contract test | PR 1 | Single PR — well under 400 LOC, additive, no trait changes |

## Phase 1: Foundation (Cargo.toml + lib.rs)

- [ ] 1.1 Add `postgres` feature in `crates/cognicode-explorer/Cargo.toml` — `postgres = ["cognicode-core/postgres", "dep:sqlx", "dep:async-trait"]`
- [ ] 1.2 Add optional deps in `Cargo.toml`: `sqlx = { workspace = true, optional = true }`, `async-trait = { workspace = true, optional = true }` (dev-dep `async-trait` already present, keep)
- [ ] 1.3 Add `#[cfg(feature = "postgres")] pub mod postgres_bridge;` to `crates/cognicode-explorer/src/lib.rs` (after line 14, after `pub mod service;`)
- [ ] 1.4 Verify `cargo check -p cognicode-explorer` (no feature) still succeeds — sqlx absent

## Phase 2: Core Helper (`postgres_bridge.rs`)

- [ ] 2.1 Create `crates/cognicode-explorer/src/postgres_bridge.rs` with module-level doc comment referencing design decision "Helper location"
- [ ] 2.2 Implement `pub async fn open_graph_from_postgres(database_url: &str) -> anyhow::Result<Arc<CallGraph>>` — calls `PostgresRepository::new(url)`, then `load_call_graph()`, returns `Arc::new(graph)` for `Some`, `Arc::new(CallGraph::new())` for `None`, propagates `Err` with `"open_graph_from_postgres: connect: {e}"` / `"open_graph_from_postgres: load: {e}"` prefix
- [ ] 2.3 Verify `cargo check -p cognicode-explorer --features postgres` compiles — helper reachable

## Phase 3: CLI Wiring — API binary

- [ ] 3.1 In `crates/cognicode-explorer/src/bin/api.rs`: add `#[cfg(feature = "postgres")] use cognicode_explorer::postgres_bridge::open_graph_from_postgres;` at top
- [ ] 3.2 Add `#[cfg(feature = "postgres")] #[arg(long)] postgres: Option<String>,` field to `Args` struct (after `listen`)
- [ ] 3.3 Replace `let graph = open_graph(&db_path)?;` in `main()` with `#[cfg(feature = "postgres")] let graph = if let Some(url) = &args.postgres { open_graph_from_postgres(url).await? } else { open_graph(&db_path)? };` plus `#[cfg(not(feature = "postgres"))] let graph = open_graph(&db_path)?;`
- [ ] 3.4 Verify `cargo check -p cognicode-explorer --bin cognicode-explorer-api --features postgres` compiles

## Phase 4: CLI Wiring — MCP binary

- [ ] 4.1 In `crates/cognicode-explorer/src/bin/mcp.rs`: add `#[cfg(feature = "postgres")] use cognicode_explorer::postgres_bridge::open_graph_from_postgres;` at top
- [ ] 4.2 Add `#[cfg(feature = "postgres")] #[arg(long)] postgres: Option<String>,` field to `Args` struct (after `cwd`)
- [ ] 4.3 Replace `let graph = open_graph(&db_path)?;` in `main()` with the same `#[cfg]`-gated PG/SQLite branch used in api.rs
- [ ] 4.4 Verify `cargo check -p cognicode-explorer --bin cognicode-explorer-mcp --features postgres` compiles

## Phase 5: Contract Test

- [ ] 5.1 Create `crates/cognicode-explorer/tests/pg_bridge_contract.rs` with `#[cfg(all(test, feature = "postgres"))]` gate at module level
- [ ] 5.2 Add `pg_test!` macro definition (mirror the one in `cognicode-core/src/infrastructure/persistence/postgres_repository.rs:680`) so `#[sqlx::test]` is not a hard dep — adapt to spin up a per-test PG pool with schema migrations
- [ ] 5.3 Test `open_graph_roundtrips_populated_db`: seed ≥5 symbols, ≥3 dep types, all 3 provenance variants `(Extracted, Inferred, Ambiguous)`, confidences `{0.0, 0.5, 1.0}`; call `open_graph_from_postgres(url)`; wrap in `CallGraphRepository`; assert `assert_eq!(g_source, g_loaded)` bit-exact; verify lookups match
- [ ] 5.4 Test `open_graph_returns_empty_for_empty_pool`: zero rows in both tables → `Ok(Arc::new(CallGraph::new()))`, counts 0/0
- [ ] 5.5 Test `open_graph_propagates_connect_error`: pass `postgres://invalid:invalid@127.0.0.1:1/nope` → `Err` with message containing `"open_graph_from_postgres: connect:"`
- [ ] 5.6 Test `metadata_aware_callees_roundtrip`: seed 3 edges `(Extracted,1.0)`, `(Inferred,0.7)`, `(Ambiguous,0.3)`; wrap result in `CallGraphRepository`; call `callees_with_metadata(&source_id)`; assert every `(DependencyType, Provenance, f64)` tuple matches source
- [ ] 5.7 Test `parallel_tests_are_isolated`: two `pg_test!` functions with disjoint seeds run in parallel, each sees only its own rows
- [ ] 5.8 Verify `cargo test -p cognicode-explorer --features postgres --test pg_bridge_contract` — all 5 tests pass

## Phase 6: Verification & Build Matrices

- [ ] 6.1 Verify `cargo check -p cognicode-explorer` (default, no feature) — succeeds, `sqlx` absent from dep graph
- [ ] 6.2 Verify `cargo check -p cognicode-explorer --features postgres` — succeeds, helper reachable
- [ ] 6.3 Verify `cargo clippy -p cognicode-explorer --features postgres --all-targets` — no new warnings
- [ ] 6.4 Run existing regression: `cargo test -p cognicode-explorer --test integration` (SQLite path) — passes byte-identical to pre-change
- [ ] 6.5 Manual smoke (api): launch `cognicode-explorer-api --postgres postgres://...` against a seeded test DB, `GET /symbols/...` returns PG-sourced data
- [ ] 6.6 Manual smoke (mcp): launch `cognicode-explorer-mcp --postgres postgres://...`, send a JSON-RPC symbol-resolve request via stdio, assert PG-sourced response

## Phase 7: Cleanup

- [ ] 7.1 Confirm `cargo build -p cognicode-explorer` produces no `sqlx` in `cargo tree` output (sanity check default build stays sqlx-free)
- [ ] 7.2 Confirm `--postgres` flag is rejected by clap when binary built WITHOUT feature (edge case: `feature absent + --postgres passed → clap rejects`)
- [ ] 7.3 No docs changes needed (helper is internal; binaries' `--help` picks up the flag automatically when feature is on)
