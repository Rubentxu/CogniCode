# Proposal: Explorer Bridge to PostgreSQL

## Intent

The CogniCode Explorer (4 ports, 4 adapters) is entirely SQLite/in-memory — no PostgreSQL wiring exists. Meanwhile, `cognicode-core` has `PostgresRepository` with full read+write over `symbols` + `call_edges`, including canonical `load_call_graph()` → `CallGraph` roundtrip. The gap is purely binary-wiring: the explorer binaries (`api`, `mcp`) need a PostgreSQL load path so the MCP/API servers can serve indexed PG data.

## Scope

### In Scope
- Add `postgres` feature gate + optional `sqlx` dep to `cognicode-explorer/Cargo.toml`
- New helper `open_graph_from_postgres(database_url) -> Arc<CallGraph>` — loads full graph into memory
- `--postgres` CLI flag on both binaries (`api.rs`, `mcp.rs`) to select PG path
- Contract test: roundtrip from PG `load_call_graph` through `CallGraphRepository`

### Out of Scope
- New adapter types (`PostgresExplorerRepository`) — premature
- `SymbolRepository` or `MetadataAwareRepository` trait changes
- Write-path from explorer (explorer is read-only by design)
- Incremental/live queries — full-graph load only
- Schema changes — `symbols` + `call_edges` are sufficient

## Capabilities

### New Capabilities
- `explorer-postgres-bridge`: Wire CogniCode Explorer to PostgreSQL via in-memory `CallGraph` bridge. Load full graph at binary startup, wrap in existing `CallGraphRepository`, serve through all 4 existing ports.

### Modified Capabilities
None. Pure additive extension — zero existing specs change.

## Approach

**In-Memory Bridge** (~98 lines). At binary startup (`tokio::main`), call `PostgresRepository::load_call_graph()`, wrap result in `Arc<CallGraph>`, and pass to the existing `CallGraphRepository`. All 4 explorer adapters remain unchanged — `CallGraphRepository` already implements both `SymbolRepository` and `MetadataAwareRepository`.

Rationale: `load_call_graph()` already reconstructs the canonical `CallGraph` (provenance + confidence preserved). `CallGraphRepository` already wraps it. Zero sync/async mismatch (block at init where tokio is available). OCP-compliant: new code only, no trait or method modifications.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/Cargo.toml` | Modified | +`postgres` feature, optional `sqlx` + `cognicode-core/postgres` |
| `crates/cognicode-explorer/src/bin/api.rs` | Modified | +`--postgres` flag, PG load branch |
| `crates/cognicode-explorer/src/bin/mcp.rs` | Modified | +`--postgres` flag, PG load branch |
| New shared helper | New | `open_graph_from_postgres()` — ~20 lines |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Full-graph memory for >10K symbols | Low (MVP) | Monitor; defer incremental queries to Approach B if needed |
| PG unavailable at CLI invocation | Medium | Graceful `Result` — if `--postgres` but no PG, exit with human-readable error |
| SQLite path regression | Low | PG path behind `#[cfg(feature = "postgres")]`; SQLite default unchanged |

## Rollback Plan

Remove `--postgres` flag wiring and `#[cfg(feature = "postgres")]` blocks from both binaries. Delete the helper function. No data migration needed — PG tables are consumed read-only.

## Dependencies

- `cognicode-core` `postgres` feature (already shipped, feature-gated)
- `sqlx` with `postgres` + `runtime-tokio` (already in workspace)

## Success Criteria

- [ ] `--postgres postgres://...` on `cognicode` binary loads graph and serves all explorer endpoints
- [ ] `--postgres` on `cognicode-mcp` serves MCP tools with PG data
- [ ] Contract test passes: `load_call_graph()` → `CallGraphRepository` → symbol/edge resolution matches PG source
- [ ] Default SQLite path unchanged — regression tests pass
- [ ] `MetadataAwareRepository` metadata (provenance, confidence) preserved through bridge
