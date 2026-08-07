# Kernel Exploration: `graph_diff` and `graph_timeline` MCP Tools

## Current State

### Data Source: `graph_reports` Table

Defined in `m0010_pipeline_schema.sql` (lines 188–198):
```sql
CREATE TABLE IF NOT EXISTS graph_reports (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id  TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    report        JSONB NOT NULL,
    symbol_count  INTEGER NOT NULL DEFAULT 0,
    edge_count    INTEGER NOT NULL DEFAULT 0,
    health_score  REAL
);
```
Index: `idx_graph_reports_workspace` on `(workspace_id, created_at DESC)`.

### Pipeline Report Stage

`report_stage.rs` receives `AnalysisSummary` and persists it via:
```rust
let report_id = format!("{}/{}", workspace_id, timestamp_ms);
sqlx::query("INSERT INTO graph_reports (id, workspace_id, report, symbol_count, edge_count, health_score) VALUES (...)")
    .bind(&report_id) // ← TEXT, not UUID!
```

**⚠️ CRITICAL BUG**: The DDL declares `id UUID` but the Rust code inserts a composite TEXT string (`workspace/timestamp_ms`). This type mismatch would cause a runtime SQL error on insert. The DDL likely evolved to UUID while the code was not updated — or vice versa. Either the schema or the insert code is wrong. This MUST be reconciled before any diff/timeline work.

### `AnalysisSummary` Shape

From `analyzer.rs` (lines 14–24):
```rust
pub struct AnalysisSummary {
    pub god_nodes: Vec<GodNode>,          // { symbol, pagerank, fan_in, fan_out }
    pub surprising_connections: Vec<SurprisingConnection>, // { source, target, reason } — currently always empty
    pub dead_code: Vec<String>,
    pub hot_paths: Vec<HotPath>,          // { symbol, fan_in }
    pub health_score: f64,
    pub symbol_count: usize,
    pub edge_count: usize,
    pub community_count: usize,           // always 0 currently
}
```

### Existing Stub

`graph_query_handlers.rs` lines 344–353: `handle_get_graph_report` returns a hardcoded stub:
```
"GraphReport from pipeline will be available after a scan with analysis stages (Sprint 2)"
```
No DB query is performed. Output is `{ report: None, message: "..." }`.

### Handler Architecture

- ISP-segregated `ToolHandler` trait in `crates/cognicode-explorer/src/mcp/handler/mod.rs`
- Each tool family has its own module (e.g., `graph.rs`, `impact.rs`, `views.rs`)
- Handlers registered via `register_*_handlers()` called from `ExplorerMcpHandler::with_graph()`
- Response envelopes via `ok_envelope(tool_name, &payload)` / `err_envelope(tool_name, code, msg)` from `envelope.rs`
- `McpContext` carries facades (`PersistenceService`, `WorkspaceService`, etc.) but NO direct `PostgresRepository` access
- `PostgresRepository` lives in `cognicode-core` under `#[cfg(feature = "postgres")]` — not accessible from `cognicode-explorer` handlers directly

### No DB Access Path for Reports

`PersistenceServiceImpl` handles named views, ViewSpecs, exploration paths, and sessions. It does NOT expose `graph_reports` queries. Handlers today cannot read `graph_reports` without a new facade method or a new port.

## Context Quality

- **Level**: C3 — deep understanding exists
- **Evidence Present**:
  - `m0010_pipeline_schema.sql` — DDL
  - `analyzer.rs` — `AnalysisSummary` struct and compute logic
  - `report_stage.rs` — insert logic
  - `ingest/service.rs` — pipeline orchestration (lines 138–141)
  - `postgres_repository.rs` — pool access pattern
  - `mcp/handler/mod.rs` — `ToolHandler` trait + registry
  - `mcp/handler/graph.rs` — complete handler example (3 tools)
  - `mcp/envelope.rs` — response envelope builders
  - `mcp/context.rs` — `McpContext` with facades
  - `mcp/explorer.rs` — tool name constants and registration
  - `graph_query_handlers.rs` — existing stub
- **Missing Context**: None blocking
- **Recommended Effort**: verify (no new exploration needed — evidence is complete)

## Knowledge Coverage

| Class | Status | Evidence | Gap Impact |
|------|--------|----------|------------|
| Roadmap/Backlog | missing | No backlog referencing these tools | Low — feature request exploration |
| Work Items | missing | No Jira/issue tracking artifacts | Low |
| Architecture/ADRs | present | ADR-017 (pipeline), ADR-022 (notify), CONTEXT.md handler architecture | ADR-017 explains `graph_reports` design |
| Ownership | present | `cognicode-explorer` crate (MCP handlers), `cognicode-core` crate (pipeline) | Clear crate boundaries |
| Learnings | missing | No prior exploration | Low |

## Problem Taxonomy

| Axis | Applies | Evidence |
|------|---------|----------|
| Domain modeling | Yes | `AnalysisSummary` is the canonical domain object; JSONB in `report` column is denormalized |
| Boundary/seam | Yes | `cognicode-core` owns `PostgresRepository`; `cognicode-explorer` owns handlers. Must bridge via facade or port |
| Coupling/connascence | Yes | Name connascence: both tools depend on `graph_reports` table schema. I(Name) = log2(2 tools) ≈ 1 bit |
| API contract | Yes | New MCP tool schemas (arg_schema JSON) + envelope format |
| Refactor/legacy | Yes | Must fix `id` UUID/TEXT mismatch AND replace dead `handle_get_graph_report` stub |
| Event/CQRS | No | Not applicable |
| Testing | Yes | Handler integration tests follow existing graph handler test patterns |
| Security/operations | No | `graph_reports` is read-only for these tools |

## Domain Language and Invariants

- **Domain Terms**:
  - `AnalysisSummary` — post-analysis snapshot persisted as JSONB in `graph_reports.report`
  - `graph_reports` — time-series table of analysis snapshots, one row per completed ingest
  - `graph_diff` — semantic diff between two `AnalysisSummary` snapshots
  - `graph_timeline` — time-series extraction of scalar metrics from sequential `graph_reports`
- **Invariants**:
  - `symbol_count`, `edge_count`, `health_score` are extracted as top-level columns for indexing — they MUST match the values inside `report` JSONB (currently they do, as report_stage.rs binds both from the same `summary`)
  - `created_at` is server-assigned (PG `DEFAULT now()`) — monotonic per workspace
- **Unresolved Ambiguities**:
  - Is `id` supposed to be UUID or TEXT? DDL says UUID; code says `{workspace}/{timestamp}` string
  - Does `community_count` (always 0 in `run_analyze`) get populated later by cluster stage?

## Knowledge Gaps

- **`graph_reports.id` type mismatch** — DDL declares UUID but code inserts TEXT. Blocks write correctness and must be resolved before diff/timeline can rely on stable row identity.

## Affected Areas

- `crates/cognicode-core/src/infrastructure/persistence/m0010_pipeline_schema.sql` — may need `id` type fix (change to TEXT or align code to UUID)
- `crates/cognicode-core/src/application/ingest/report_stage.rs` — may need `id` generation fix (use `Uuid::new_v4()` or change DDL)
- `crates/cognicode-core/src/application/ingest/analyzer.rs` — `AnalysisSummary` is the diff payload source
- `crates/cognicode-explorer/src/mcp/handler/graph.rs` — new handler or sibling module for report tools
- `crates/cognicode-explorer/src/mcp/handler/mod.rs` — registry registration + re-export
- `crates/cognicode-explorer/src/mcp/explorer.rs` — tool name constants + registration call
- `crates/cognicode-explorer/src/mcp/context.rs` — may need new facade or port for `graph_reports` access
- `crates/cognicode-explorer/src/facades/persistence.rs` — may need new `get_graph_reports()` facade method
- `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs` — needs `query_graph_reports()` method(s)
- `crates/cognicode-core/src/interface/mcp/handlers/graph_query_handlers.rs` — replace dead `handle_get_graph_report` stub

## Options

| Option | Pros | Cons | Effort |
|--------|------|------|--------|
| **A: New `reports.rs` handler module in `cognicode-explorer`** | ISP-compliant; follows existing pattern (like `graph.rs`); clean separation | Requires new facade or port for `graph_reports` DB access | Medium |
| **B: Add to existing `graph.rs` handler module** | Less file churn; graph-adjacent tools | Violates SRP — `graph.rs` is about call-graph topology, not temporal reports | Low |
| **C: Implement in `cognicode-core` via `graph_query_handlers.rs`** | Replaces dead stub in-place; `PostgresRepository` already available there | Old-style handler (not ToolHandler trait); `cognicode-core` MCP module is legacy/consolidated | Low (quick fix) |
| **D: Hybrid — proper trait handler in `cognicode-explorer` + `PersistenceService` facade with `PostgresRepository` behind feature gate** | Clean architecture; ISP; follows all existing patterns; testable | Most work upfront | Medium |

**Recommendation: Option D (hybrid with facade)**.

Rationale:
- Option C (quick fix in `graph_query_handlers.rs`) is tempting but violates the ISP-segregated `ToolHandler` architecture that is the project standard now
- Option A requires a facade method, which aligns with the existing `PersistenceService` pattern
- The `PersistenceService` trait and `PersistenceServiceImpl` already exist; adding `get_graph_reports_range(workspace_id, since)` and `get_graph_reports_by_ids(ids)` is minimal
- `#[cfg(feature = "postgres")]` gates mean non-Postgres builds compile the stub (return "requires postgres feature")

## Entropy Envelope

- **Method**: heuristic (CogniCode graph unavailable)
- **Coupling risk**: low
  - `graph_diff` depends on `graph_reports` table schema (single source of truth)
  - `graph_timeline` depends on `(workspace_id, created_at)` index
  - Both depend on `AnalysisSummary` deserialization — already `Serialize`-derived
- **I(Name) for new tools**: log2(2) ≈ 1 bit — low coupling to report schema
- **OCP assessment**: Pure extension — new handler + new facade method. No existing code modified except for fixing the `id` bug and replacing the dead stub
- **Connascence**: Name connascence with `graph_reports` column names; Type connascence with `AnalysisSummary` struct. Both are low-severity.

## Recommendation

1. **Fix immediately** (prerequisite): Resolve the `graph_reports.id` UUID/TEXT mismatch. Choose one:
   - **Preferred**: Keep UUID in DDL, fix `report_stage.rs` to use `Uuid::new_v4()` instead of composite string
   - **Alternative**: Change DDL to `id TEXT` — simpler but loses type safety
   
2. **Implement via Option D**:
   - Add `load_graph_reports_range()` and `load_graph_reports_by_ids()` to `PostgresRepository` (behind `#[cfg(feature = "postgres")]`)
   - Add corresponding methods to `PersistenceService` trait + `PersistenceServiceImpl`
   - Create `crates/cognicode-explorer/src/mcp/handler/reports.rs`:
     - `GraphDiffHandler` — `graph_diff` tool
     - `GraphTimelineHandler` — `graph_timeline` tool
   - Register in `ExplorerMcpHandler::with_graph()` via `register_report_handlers()`
   - Replace dead `handle_get_graph_report` stub with a call through the facade

3. **MVP scope for `graph_diff`**:
   - Input: two `report_id`s (UUIDs) or `date_a`/`date_b` strings
   - Output: `{ delta: { health_score: { old, new }, symbol_count: { old, new }, edge_count: { old, new } }, added_god_nodes: [...], removed_god_nodes: [...], added_dead_code: [...], resolved_dead_code: [...] }`

4. **MVP scope for `graph_timeline`**:
   - Input: `workspace_id`, `days` (default 30)
   - Output: `{ points: [{ created_at, symbol_count, edge_count, health_score }], trends: { symbol_count: "up|down|stable", edge_count: "up|down|stable", health_score: "up|down|stable" } }`

## Ready For Proposal

**No** — The `graph_reports.id` type mismatch is a blocking prerequisite. This bug must be resolved before either tool can be implemented. The exploration is otherwise complete and the architecture is clear.

Once the ID issue is resolved, this is ready for proposal with an effort estimate of **Medium** (2–3 implementation tasks: fix id bug → persistence facade → handler module).
