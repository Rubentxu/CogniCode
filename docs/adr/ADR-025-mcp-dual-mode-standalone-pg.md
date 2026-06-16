# ADR-025: MCP Dual-Mode — Standalone In-Memory + PG-Connected Pipeline

**Status:** Accepted  
**Date:** 2026-06-15  
**Source:** Impact analysis of ADR-017/018/019 on cognicode-mcp

## Context

The `cognicode-mcp` binary serves AI agents via MCP (Model Context Protocol).
Today it operates purely in-memory: `CogniCodeHandler::new(cwd)` creates an
`InMemoryGraphStore`, `build_graph` parses files with tree-sitter (6 languages,
match-arms), and all 32+ analysis tools read from `GraphStore::load_graph()`.

The ingest pipeline (ADRs 017-024) introduces fundamental changes that break
this model:

1. **LanguageConfig replaces match-arms** (ADR-018) — `build_graph` must use
   the new generic extractor, not `AnalysisService::build_project_graph()`.
2. **Per-file PgUpsert replaces global save** (ADR-017) — `GraphStore::
   save_graph(&CallGraph)` is incompatible with per-file transactional upsert.
3. **symbols/call_edges become VIEWs** (ADR-019) — only works with PG.
4. **New edge types** (imports, references, inherits, contains) don't fit in
   the code-only `CallGraph`.
5. **Pipeline is PG-only** — the MCP must still work without PG for agents
   that want quick ephemeral intelligence.

Additionally, a **dual-state bug** exists today: `handle_build_graph` writes
to `GraphCache` (line 1023: `ctx.analysis_service.graph_cache().set(graph)`),
but all analysis tools read from `GraphStore::load_graph()`. These are two
different stores that can desynchronize.

## Decision

The MCP server operates in **two modes**, determined at startup:

### Mode A: Standalone (default, no PG)

```
cognicode-mcp --cwd /project
```

- `build_graph` uses the **LanguageConfig generic extractor** (same as the
  pipeline) but stores results in the in-memory `GraphCache` (ArcSwap).
- No `GraphStore` involved — `build_graph` populates `GraphCache` directly.
- All analysis tools read from `GraphCache::get()` (ArcSwap, lock-free).
- Ephemeral: graph lost when the process exits.
- Supports all 36+ languages (same LanguageConfig as the pipeline).
- New edge types stored as `GraphEdge` in a `GenericGraph` alongside the
  `CallGraph` projection in the cache.

### Mode B: PG-Connected (DATABASE_URL or --postgres)

```
DATABASE_URL=postgres://... cognicode-mcp --cwd /project
```

- `build_graph` delegates to the **ingest pipeline** (Scan → Extract →
  PgUpsert → Resolve → Cluster → Analyze → Report → Refresh).
- The pipeline populates PG, then loads the `CallGraph` into `GraphCache`.
- All analysis tools read from `GraphCache::get()` — same as Mode A.
- **NEW tool: `scan_workspace`** — async version that returns a job_id
  (for large projects). `build_graph` remains as the synchronous fallback.
- **NEW tool: `get_graph_report`** — returns the auto-generated GraphReport
  from the pipeline's Report stage.
- Persistent: graph survives process restart (loaded from PG on startup).
- Shared with the Explorer (same PG tables).

### Unification: all tools read from GraphCache

Fix the dual-state bug by making **all analysis tools read from `GraphCache`**,
not `GraphStore`. `GraphStore` is only used by `build_graph` for cache
management in standalone mode.

```rust
// BEFORE (buggy dual-state):
fn handle_get_call_hierarchy(ctx: &HandlerContext) {
    let graph = ctx.get_graph_store().load_graph()?;  // ← GraphStore
    // ...
}

// AFTER (unified):
fn handle_get_call_hierarchy(ctx: &HandlerContext) {
    let graph = ctx.analysis_service.graph_cache().get();  // ← ArcSwap
    // ...
}
```

### GraphStore trait: deprecated for reads, kept for standalone cache

The `GraphStore` trait (`save_graph`, `load_graph`, `save_manifest`,
`load_manifest`) is:
- **Kept** for standalone mode (Mode A) as the in-memory cache backing.
- **Not used** in PG mode (Mode B) — PG IS the store.
- **Not extended** with per-file operations — the pipeline handles that
  directly via `sqlx` transactions.

New constructor:

```rust
impl CogniCodeHandler {
    /// Mode A: standalone, in-memory
    pub fn new(project_root: PathBuf) -> Self { ... }

    /// Mode B: PG-connected, shares graph with Explorer
    pub fn with_postgres(
        project_root: PathBuf,
        database_url: &str,
    ) -> anyhow::Result<Self> { ... }
}
```

### Tool impact matrix

| Tool | Mode A change | Mode B change | Notes |
|------|---------------|---------------|-------|
| `build_graph` | **Rewrite**: LanguageConfig extractor + GraphCache | **Delegate**: to pipeline | Most impacted tool |
| `get_call_hierarchy` | Read from GraphCache (fix dual-state) | Same | Minimal change |
| `analyze_impact` | Read from GraphCache | Same | Minimal change |
| `check_architecture` | Read from GraphCache | Same | Minimal change |
| `get_entry_points` | Read from GraphCache | Same | Minimal change |
| `get_hot_paths` | Read from GraphCache | Same | Minimal change |
| `trace_path` | Read from GraphCache | Same | Minimal change |
| `export_mermaid` | Read from GraphCache | Same | Minimal change |
| `graph_pagerank` | Read from GraphCache | Same | Minimal change |
| `graph_communities` | Read from GraphCache | Same | Minimal change |
| `graph_god_nodes` | Read from GraphCache | Same | Minimal change |
| `graph_insights` | Read from GraphCache | Same | Minimal change |
| `get_file_symbols` | **No change** | No change | Works on tree-sitter directly |
| `get_outline` | **No change** | No change | Works on tree-sitter directly |
| `get_symbol_code` | **No change** | No change | Reads source files |
| `get_complexity` | **No change** | No change | Works on tree-sitter directly |
| `semantic_search` | **No change** | No change | Works on symbol index |
| `find_usages` | **No change** | No change | Works on symbol index |
| `go_to_definition` | **No change** | No change | LSP, not graph |
| `hover` | **No change** | No change | LSP, not graph |
| `find_references` | **No change** | No change | LSP, not graph |
| `read_file` | **No change** | No change | File I/O |
| `search_content` | **No change** | No change | File I/O |
| `list_files` | **No change** | No change | File I/O |
| `write_file` | **No change** | No change | File I/O |
| `edit_file` | **No change** | No change | File I/O |
| `safe_refactor` | Read from GraphCache | Same | Minimal change |
| `build_lightweight_index` | **Deprecated** (manifest replaces it) | **Removed** | Replaced by Scan stage |
| `query_symbol_index` | Read from GraphCache | Same | Minimal change |
| `find_dead_code` | Read from GraphCache | Same | Minimal change |
| `build_call_subgraph` | Read from GraphCache | Same | Minimal change |
| `get_per_file_graph` | **No change** | No change | Works on tree-sitter |
| `merge_graphs` | **Deprecated** (PG is single source) | **Removed** | Replaced by PG |
| **NEW** `scan_workspace` | N/A (Mode B only) | **New**: async pipeline trigger | Returns job_id |
| **NEW** `get_graph_report` | N/A (Mode B only) | **New**: fetch GraphReport | From Report stage |

### MCP binary changes

```rust
// main.rs (Mode detection)
let handler = if let Some(pg_url) = postgres_url {
    // Mode B: PG-connected
    CogniCodeHandler::with_postgres(args.cwd, &pg_url)?
} else {
    // Mode A: standalone in-memory
    CogniCodeHandler::new(args.cwd)
};
```

## Rationale

- **Standalone mode is critical.** AI agents (Claude, Cursor, Windsurf) use
  `cognicode-mcp` for quick code intelligence without infrastructure. Forcing
  PG breaks this use case. Mode A preserves it.
- **PG mode enables sharing.** When PG is available, the MCP and Explorer share
  the same graph. An agent's `build_graph` makes the graph available in the
  Explorer instantly.
- **Fixing dual-state is overdue.** `build_graph` writes to GraphCache but tools
  read from GraphStore — this is a latent bug. Unifying on GraphCache fixes it
  and makes both modes use the same read path.
- **LanguageConfig is shared.** Both modes use the same extractor code. No
  duplication. The only difference is WHERE the graph lives after extraction.

## Consequences

- `build_graph` is rewritten to use LanguageConfig instead of AnalysisService
  match-arms. The AnalysisService is deprecated for MCP use.
- 20+ analysis tools change their read path from `GraphStore::load_graph()`
  to `GraphCache::get()`. This is a mechanical change but touches many files.
- `build_lightweight_index` and `merge_graphs` are deprecated.
- Two new tools (`scan_workspace`, `get_graph_report`) are Mode B only.
- The MCP binary gains `--postgres` flag and `DATABASE_URL` env support
  (same pattern as the Explorer API binary).
- Mode B MCP startup loads the graph from PG (same as Explorer's
  `open_graph_from_postgres`).

## Alternatives Considered

- **PG-only MCP (drop standalone):** rejected — breaks the primary use case of
  AI agents that don't have PG. CogniCode's value proposition includes
  zero-infrastructure code intelligence.
- **Keep AnalysisService for MCP, pipeline for Explorer:** rejected — two
  extraction code paths diverge over time. LanguageConfig must be shared.
- **New trait `IngestStore` alongside `GraphStore`:** rejected — adds complexity.
  The MCP doesn't need per-file operations; it either builds in-memory (Mode A)
  or delegates to the pipeline (Mode B). `GraphStore` for standalone cache +
  direct `sqlx` for PG mode is sufficient.
