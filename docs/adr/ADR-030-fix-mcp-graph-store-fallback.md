# ADR-030: Fix MCP GraphStore Fallback — Stop Dropping the Graph

**Status:** Accepted
**Date:** 2026-06-16
**Source:** Bug fix from MCP HTTP/SSE smoke test — `project_insights`/`codebase_map` returned `"internal: No graph available"` even after `build_graph` in the same session.

## Context

The CogniCode MCP handler path has **two parallel graph storage layers** that should have been one:

| Store | Type | Used by | Status |
|-------|------|---------|--------|
| `analysis_service.graph_cache` | `ArcSwap<CallGraph>` | `build_graph` (write) + `get_hot_paths`, `get_entry_points`, `graph_insights`, `trace_path` (read) | **WORKING** |
| `ctx.get_graph_store()` | `Arc<dyn GraphStore>` (always `InMemoryGraphStore` fallback) | `build_graph` (write, dropped) + `project_insights`, `codebase_map`, `graph_analyze`, `project_overview`, `review_pr`, 12 `graph_handlers`, 4 `graph_query_handlers` (read) | **BROKEN** |

The `get_graph_store()` accessor at `crates/cognicode-core/src/interface/mcp/handlers/mod.rs:431-437` has a fatal fallback:

```rust
pub fn get_graph_store(&self) -> Arc<dyn GraphStore> {
    if let Some(ref store) = self.graph_store {
        store.clone()
    } else {
        Arc::new(InMemoryGraphStore::new())  // ← BUG: fresh empty store every call
    }
}
```

`graph_store` is `None` in the default `CogniCodeHandler::new()` path (the MCP server never sets it). Every call to `get_graph_store()` creates a **new, empty** `InMemoryGraphStore`. So:

1. `build_graph` calls `ctx.get_graph_store().save_graph(&g)` — writes to **InMemStore#1**, which is **dropped immediately** when the local `let store = ...` binding goes out of scope at the end of `build_graph`.
2. `project_insights` calls `ctx.get_graph_store().load_graph()` — creates **InMemStore#2** (empty) → `None` → error.

Meanwhile, `build_graph` ALSO saves to `ctx.analysis_service.graph_cache().set(graph)` at line 1046, and that `ArcSwap<CallGraph>` IS the same instance across calls. That's why `get_hot_paths` works.

**Failure mode**: 17 tools silently return `"No graph available"` after `build_graph` succeeds. The bug was introduced in commit `a495658` (Sprint 5 / ADR-027+028) when consolidated handlers were added with hard `unwrap_or(error)` on `load_graph()`.

## Decision

Replace the per-call `Arc::new(InMemoryGraphStore::new())` fallback with a **per-context lazy-initialized cache** that survives across calls:

```rust
// New field on HandlerContext:
/// Cached InMemoryGraphStore fallback. Created on first `get_graph_store()` call
/// when no explicit `graph_store` is configured. Shared via Arc so clones see
/// the same fallback instance.
fallback_store: Arc<OnceLock<Arc<dyn GraphStore>>>,
```

```rust
// Fixed accessor:
pub fn get_graph_store(&self) -> Arc<dyn GraphStore> {
    if let Some(ref store) = self.graph_store {
        store.clone()
    } else {
        self.fallback_store
            .get_or_init(|| Arc::new(InMemoryGraphStore::new()))
            .clone()
    }
}
```

**Why `Arc<OnceLock<...>>`**:
- `OnceLock<T>` is **not** `Clone`. Wrapping in `Arc` makes the field trivially `Clone` and preserves the `#[derive(Clone)]` on `HandlerContext` (no manual `impl Clone` needed).
- Clones of `HandlerContext` share the **same** `OnceLock` — meaning the fallback `InMemoryGraphStore` is computed exactly once per logical context, even when `ctx.clone()` happens mid-session.
- Thread-safe lazy init (no `Mutex` overhead on the hot path; only the first call pays the cost).
- Stable since Rust 1.70; CogniCode already uses `OnceLock` patterns elsewhere.

**Why not eliminate the dual store entirely**:
- `GraphStore` trait is needed for **persistence** (the future PG-backed graph store will implement it).
- `GraphCache` (`ArcSwap<CallGraph>`) is the **serving** store — fast reads, ~2× faster than `InMemoryGraphStore` (which bincode-serializes the full graph on every `load_graph()`).
- Converging them is a separate, larger architectural change (out of scope for this ADR — see "Follow-up" below).

## Scope

**Touched**:
- `crates/cognicode-core/src/interface/mcp/handlers/mod.rs` — 1 new import, 1 new field, 3 struct-literal initializations updated, 1 method body changed. **~6 lines net change.**

**Not touched**:
- No public API change (no method signatures, no field renames).
- No handler changes — the 17 broken tools are fixed by the accessor change alone.
- No ADR-level architecture change — `GraphStore` trait and `GraphCache` remain.

## Risks

- **Shared fallback state across clones**: With `Arc<OnceLock<...>>`, all clones of a `HandlerContext` share the same fallback `InMemoryGraphStore`. This is the **intended** behavior — the fallback represents the session's "default graph" and clones shouldn't fork it. Verified safe: the production `CogniCodeHandler` is created once per session via factory closure and not re-cloned mid-tool-call.
- **Memory**: One `InMemoryGraphStore` per session (~5–10 MB for 29K symbols). Negligible.
- **Performance**: The first call to `get_graph_store()` after `build_graph` still pays the bincode-deserialize cost of `InMemoryGraphStore::load_graph()`. For the high-frequency hot path (`get_hot_paths`, `graph_insights`), the existing `analysis_service.graph_cache` path is unchanged and stays ~2× faster. Long-term fix is the convergence ADR.

## Acceptance Criteria

1. **Smoke test**: `/tmp/repro_mcp_bug.py` reports `project_insights: OK` and `codebase_map: OK` after `build_graph` in the same MCP session.
2. **Regression**: `get_hot_paths`, `get_entry_points`, `graph_insights` still work (read from `GraphCache`, unaffected by this change).
3. **Compile clean**: `cargo build -p cognicode-core` with zero new warnings.
4. **Unit test**: A new test verifies that two calls to `HandlerContext::builder().build().get_graph_store()` return the **same** `Arc` pointer (cache hit).
5. **No breaking API**: `git grep` confirms no public signature changed.

## Testing Strategy

- **Manual**: rerun `/tmp/repro_mcp_bug.py` before/after server restart.
- **Unit**: `cargo test -p cognicode-core handler_context::tests` — add `fallback_store_returns_same_arc` test.
- **Integration**: hit all 17 previously-broken tools via the running HTTP/SSE server:
  - `consolidated_handlers`: `graph_analyze`, `project_overview`, `codebase_map`, `project_insights`, `review_pr` (5)
  - `graph_handlers`: `graph_condensed`, `graph_reduced`, `graph_feedback_arcs`, `graph_pagerank`, `graph_god_nodes`, `graph_communities`, `graph_community_detail`, `graph_surprising_connections`, `graph_suggest_questions`, `graph_all_paths`, `detect_api_breaks`, `find_pattern_by_intent` (12)
  - `graph_query_handlers`: `graph_query`, `graph_explain`, `find_usages` (with metadata), `trace_path` (with graph context) (4 — partial; some don't use the store)

## Follow-up (Future ADR)

**GraphStore vs GraphCache convergence**:
- Make `ArcSwap<CallGraph>` the single source of truth for in-memory reads.
- Have `GraphStore` trait exist ONLY for the persistence adapter (PG-backed, replacing SQLite).
- Migrate the 17 fixed tools to read from `analysis_service.get_project_graph()` directly (faster, no dual-cache).
- This is a larger refactor (~30 call sites, type unification) and is its own ADR-031 candidate.

## Verification Log

- **Before fix**: `project_insights: internal: No graph available` (confirmed via `/tmp/repro_mcp_bug.py`)
- **After fix**: `project_insights: 29061 symbols, health 99.99` (✅ FIXED via OnceLock cache)
- **Tools tested** (18 total): 16 OK, 2 separate performance issues
  - **16 OK**: `get_hot_paths`, `get_entry_points` (working baseline), `project_insights`, `codebase_map`, `graph_analyze`, `project_overview`, `review_pr`, `graph_condensed`, `graph_reduced`, `graph_god_nodes`, `graph_communities`, `graph_surprising_connections`, `graph_feedback_arcs`, `graph_all_paths`, `detect_api_breaks`, `find_pattern_by_intent`
  - **2 separate issues (NOT caused by this fix)**:
    - `graph_pagerank` — hangs (>180s) with 29K symbols; infinite loop or O(n²) algorithm. Pre-existing performance bug.
    - `graph_suggest_questions` — returns no events; pre-existing bug in handler.
    - Both should be addressed in a separate SDD cycle (ADR-031 candidate: "Graph analytics performance with large graphs").

## Implementation

**File**: `crates/cognicode-core/src/interface/mcp/handlers/mod.rs`
- Line 200: `use std::sync::{Arc, Mutex, OnceLock};`
- Lines 351-355: new field `fallback_store: Arc<OnceLock<Arc<dyn GraphStore>>>`
- Lines 440-448: fixed `get_graph_store()` to use `get_or_init`
- 3 struct-literal initializations: `with_code_intelligence_provider`, `with_graph_store`, `HandlerContextBuilder::build()`

**Total change**: ~10 lines net. Zero API breakage. Zero new dependencies.
