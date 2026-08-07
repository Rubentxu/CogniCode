# Design: explorer-bridge-postgres

## Technical Approach

In-Memory Bridge: at binary startup, `PostgresRepository::new(url).await?` → `load_call_graph().await?` → `Arc<CallGraph>` → `CallGraphRepository::new(graph)`. Zero trait changes. Zero adapter changes. The `PgPool` is dropped after `load_call_graph` returns — the explorer holds only the `Arc<CallGraph>`.

## Architecture Decisions

### Decision: Helper location — `cognicode-explorer` lib, not `cognicode-core`

**Choice**: `pub mod postgres_bridge` inside `cognicode-explorer/src/` under `#[cfg(feature = "postgres")]`
**Alternatives**: Put helper in `cognicode-core`; duplicate in each binary
**Rationale**: The helper depends on `ExplorerError` (explorer's error type) and is only consumed by explorer binaries. Core must not depend on explorer. Duplicating would violate DRY across `api.rs`/`mcp.rs`. The `mod.rs` gate keeps it unreachable without the feature.

### Decision: Error conversion — `RepositoryError` → `ExplorerError` via `anyhow`

**Choice**: `PostgresRepository::new/load_call_graph` → `RepositoryError` → `.map_err(|e| anyhow!("open_graph_from_postgres: {e}"))?` → bubbles as `anyhow::Error` → `ExplorerError::Anyhow` at binary level
**Alternatives**: Add `From<RepositoryError> for ExplorerError`; use `thiserror` variant
**Rationale**: Adding `From<RepositoryError>` would couple `cognicode-explorer` to `cognicode-core`'s repository error type. The `anyhow` bridge matches the existing pattern in both binaries (they return `anyhow::Result` from `main`). Minimal surface area.

### Decision: CLI flag — conditional clap arg via `#[cfg]`

**Choice**: `#[cfg(feature = "postgres")] #[arg(long)] postgres: Option<String>` on `Args` struct
**Alternatives**: Runtime feature detection; always-present flag with runtime error
**Rationale**: `#[cfg]`-gating removes the flag entirely when the feature is off — clap rejects `--postgres` as unknown before any code runs. Zero cost when disabled. Matches the spec edge case "feature absent + `--postgres` passed → clap rejects".

### Decision: No retry, no fallback — fail fast

**Choice**: `--postgres` set + PG unreachable → `Err` → binary exits non-zero
**Alternatives**: Retry with backoff; silent fallback to SQLite
**Rationale**: The operator explicitly chose PG. Falling back silently hides misconfiguration. Retry is out of scope (spec: "No retry"). Fail fast matches Unix convention.

## Data Flow

```
Binary startup (tokio::main)
│
├─ #[cfg(postgres)] --postgres URL present?
│   YES ──→ open_graph_from_postgres(url).await?
│           │
│           ├─ PostgresRepository::new(url) ──→ PgPool (owned)
│           ├─ repo.load_call_graph() ──→ Option<CallGraph>
│           ├─ pool dropped (no retain)
│           └─ Ok(Arc<CallGraph>) or Err → exit(1)
│
└─ NO (default) ──→ open_graph(&db_path) ──→ existing SQLite path (byte-identical)

Both paths ──→ Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph))
           ──→ ExplorerService::with_all(repo, reader, cwd, search, quality)
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/Cargo.toml` | Modify | Add `[features] postgres = ["cognicode-core/postgres", "dep:sqlx", "dep:async-trait"]` |
| `crates/cognicode-explorer/src/lib.rs` | Modify | Add `#[cfg(feature = "postgres")] pub mod postgres_bridge;` |
| `crates/cognicode-explorer/src/postgres_bridge.rs` | Create | `open_graph_from_postgres()` helper (~25 lines) |
| `crates/cognicode-explorer/src/bin/api.rs` | Modify | Add `--postgres` flag to `Args`, PG branch in `main()` |
| `crates/cognicode-explorer/src/bin/mcp.rs` | Modify | Add `--postgres` flag to `Args`, PG branch in `main()` |
| `crates/cognicode-explorer/tests/pg_bridge_contract.rs` | Create | `#[cfg(all(test, feature = "postgres"))]` contract test with `#[sqlx::test]` |

## Interfaces / Contracts

### `postgres_bridge.rs`

```rust
#[cfg(feature = "postgres")]
use std::sync::Arc;
#[cfg(feature = "postgres")]
use cognicode_core::domain::aggregates::CallGraph;
#[cfg(feature = "postgres")]
use cognicode_core::infrastructure::persistence::PostgresRepository;

/// Load a `CallGraph` from PostgreSQL into memory.
///
/// Connects, runs migrations, loads the full graph, drops the pool.
/// Returns `Ok(Arc<CallGraph::new()>)` for empty DB, `Err` for failures.
#[cfg(feature = "postgres")]
pub async fn open_graph_from_postgres(
    database_url: &str,
) -> anyhow::Result<Arc<CallGraph>> {
    let repo = PostgresRepository::new(database_url).await
        .map_err(|e| anyhow::anyhow!("open_graph_from_postgres: connect: {e}"))?;
    match repo.load_call_graph().await
        .map_err(|e| anyhow::anyhow!("open_graph_from_postgres: load: {e}"))?
    {
        Some(graph) => Ok(Arc::new(graph)),
        None => Ok(Arc::new(CallGraph::new())),
    }
}
```

### CLI `Args` (api.rs — identical pattern in mcp.rs)

```rust
#[cfg(feature = "postgres")]
#[arg(long)]
postgres: Option<String>,
```

### `main()` branching (both binaries)

```rust
#[cfg(feature = "postgres")]
let graph = if let Some(url) = &args.postgres {
    open_graph_from_postgres(url).await?
} else {
    open_graph(&db_path)?
};
#[cfg(not(feature = "postgres"))]
let graph = open_graph(&db_path)?;
```

### Cargo.toml feature definition

```toml
[features]
postgres = ["cognicode-core/postgres", "dep:sqlx", "dep:async-trait"]

[dependencies]
# ... existing ...
sqlx = { workspace = true, optional = true }
async-trait = { workspace = true, optional = true }

[dev-dependencies]
# ... existing ...
sqlx = { workspace = true }
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `open_graph_from_postgres` — populated DB | `#[sqlx::test]` seed 7 sym / 12 edge, call helper, `assert_eq!(g_source, g_loaded)`, verify all `(provenance, confidence)` bit-exact |
| Unit | `open_graph_from_postgres` — empty DB | `#[sqlx::test]` with empty pool, assert `Ok(Arc<CallGraph::new())`, counts 0/0 |
| Unit | `open_graph_from_postgres` — connect failure | Pass invalid URL, assert `Err` contains `"open_graph_from_postgres: connect:"` |
| Unit | `MetadataAwareRepository` round-trip | Seed 3 edges `(Extracted,1.0)`, `(Inferred,0.7)`, `(Ambiguous,0.3)`, wrap in `CallGraphRepository`, verify `callees_with_metadata` matches |
| Contract | PG → `CallGraphRepository` round-trip | Seed ≥5 sym, ≥3 dep types, all 3 provenance, confidences `{0.0, 0.5, 1.0}`, `assert_eq!(g_source, g_loaded)`, every lookup matches |
| Contract | Parallel test isolation | Two `#[sqlx::test]` with disjoint seeds, run in parallel, each sees only its own rows |
| Build | Default build sqlx-free | `cargo check -p cognicode-explorer` succeeds, no `sqlx` in dep graph |
| Build | Feature build exposes helper | `cargo check -p cognicode-explorer --features postgres` succeeds, helper reachable |
| Regression | SQLite path unchanged | Binary without `--postgres` follows exact same `open_graph(&db_path)` code path |
| Integration | API binary with `--postgres` | Launch with `--postgres`, `GET /symbols/...` returns PG-sourced data |
| Integration | MCP binary with `--postgres` | Launch with `--postgres`, MCP handler resolves PG-sourced symbols via stdio |

## Migration / Rollout

No migration required. PG tables are consumed read-only. Rollback = remove `--postgres` wiring and `#[cfg(feature = "postgres")]` blocks from binaries; delete `postgres_bridge.rs` and `Cargo.toml` feature entry.

## Open Questions

- [ ] Should `sqlx` in `dev-dependencies` be unconditional or also feature-gated? (Current design: unconditional — tests only compile with `--features postgres` via `#[cfg(all(test, feature = "postgres"))]` gate, so the dep is harmless without the feature.)
