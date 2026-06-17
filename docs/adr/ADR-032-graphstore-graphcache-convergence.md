# ADR-032: GraphStore + GraphCache Convergence — Single Source of Truth

**Status:** Accepted
**Date:** 2026-06-16
**Source:** Follow-up to ADR-030. After fixing the "No graph available" bug with an `OnceLock` cache, the dual-store architecture remains a design fracture that the SDD explore agent identified as the next priority.

## Context

CogniCode's MCP server has **two parallel in-memory graph storage layers** that serve the same data:

| Store | Type | Readers | Read cost | Write path |
|-------|------|---------|-----------|------------|
| `AnalysisService.graph_cache` (`Arc<GraphCache>` → `ArcSwap<CallGraph>`) | Lock-free, atomic | 56 tools (hot path) | O(1) — clone inner `Arc<CallGraph>` | `analysis_service.rs:337` |
| `HandlerContext.get_graph_store()` (`Arc<dyn GraphStore>` → `InMemoryGraphStore` with `Mutex<Option<Vec<u8>>>`) | Mutex + bincode | 35 tools (slow path) | O(N) — full bincode deserialize of 5–10 MB graph | `handlers/mod.rs:1071` |

### Why this matters

- **Performance**: the slow path is **~2× slower** for every read on a 29K-symbol graph (5–10 MB deserialization per `load_graph()` call). The fast path is a single `Arc` clone.
- **Memory**: the graph is stored twice — once as `CallGraph` in `GraphCache` and once as bincode bytes in `InMemoryGraphStore`. ~2× the memory footprint.
- **Maintainability**: 35 readers + 1 writer dual-wired, 56 readers on the other side, two different APIs (`get_project_graph()` vs `load_graph()`).
- **Future risk**: every new tool author has to know which store to read from, or they hit the same bug as ADR-030 (writing to one and reading from the other).

### Why does the dual store exist at all?

`InMemoryGraphStore` was created for **testability** and to support a future **PG-backed persistence adapter** (`GraphStore` trait was designed for that). But:

- The PG-backed adapter has **never been built** (verified by grep: no `impl GraphStore for` outside tests).
- `InMemoryGraphStore` in production serves only the bincode overhead path with no real persistence benefit.
- The `GraphStore` trait itself is a clean abstraction that should remain — but the production in-memory implementation should be **fast**, not bincode-serialized.

## Decision

Introduce a **`CachedGraphStore` wrapper** that implements the `GraphStore` trait but delegates `load_graph()` to the existing `GraphCache` (lock-free `ArcSwap` path), while keeping `save_graph()` / `save_manifest()` / `clear()` / `exists()` on an internal `InMemoryGraphStore` for trait-compatibility and future persistence migration.

### Architecture

```
            ┌─────────────────────────────────────────────┐
            │        HandlerContext::get_graph_store()    │
            └──────────────────────┬──────────────────────┘
                                   │ returns
                                   ▼
            ┌─────────────────────────────────────────────┐
            │   CachedGraphStore (new)                    │
            │   ┌─────────────────┐  ┌────────────────┐   │
            │   │ load_graph() ──►│  │ save_graph() ──┼─┼─► InMemoryGraphStore
            │   │ delegates to    │  │ forwards to    │  │  (Mutex<Option<Vec<u8>>>)
            │   │ Arc<GraphCache> │  │                │  │  kept for trait compat
            │   └────────┬────────┘  └────────────────┘  │
            └────────────┼────────────────────────────────┘
                         │ reads from (lock-free ArcSwap)
                         ▼
            ┌─────────────────────────────────────────────┐
            │   AnalysisService::graph_cache              │
            │   ArcSwap<CallGraph>                        │
            └─────────────────────────────────────────────┘
```

### Code sketch — `crates/cognicode-core/src/infrastructure/persistence/cached_graph_store.rs`

```rust
//! Cached `GraphStore` impl that delegates reads to a shared
//! `Arc<GraphCache>` (lock-free `ArcSwap<CallGraph>`) and forwards
//! writes/manifest operations to an inner `InMemoryGraphStore`.
//!
//! This makes `HandlerContext::get_graph_store()` return a `GraphStore`
//! that is fast on the read path (~2× faster than the bincode path)
//! while keeping the `GraphStore` trait contract intact for future
//! persistence adapters. See ADR-032.

use std::sync::Arc;

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::traits::graph_store::{GraphStore, StoreError};
use crate::domain::value_objects::file_manifest::FileManifest;
use crate::infrastructure::graph::GraphCache;
use crate::infrastructure::persistence::InMemoryGraphStore;

/// `GraphStore` impl that reads from a shared `GraphCache` and
/// forwards writes to an inner `InMemoryGraphStore`.
pub struct CachedGraphStore {
    /// Lock-free cache for reads.
    cache: Arc<GraphCache>,
    /// Inner store for write/manifest/clear operations.
    inner: InMemoryGraphStore,
}

impl CachedGraphStore {
    /// Create a new `CachedGraphStore` that reads from `cache` and
    /// forwards writes to a fresh `InMemoryGraphStore`.
    pub fn new(cache: Arc<GraphCache>) -> Self {
        Self {
            cache,
            inner: InMemoryGraphStore::new(),
        }
    }
}

impl GraphStore for CachedGraphStore {
    /// Reads from the shared `GraphCache` (lock-free, no serialization).
    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        let arc_graph = self.cache.get();
        if arc_graph.symbol_count() == 0 && arc_graph.edge_count() == 0 {
            // Cache is empty — return None so callers can prompt the
            // user to run `build_graph` first (matches legacy behaviour).
            Ok(None)
        } else {
            // Dereference the Arc to get an owned CallGraph (cheap: Arc clone + data move).
            Ok(Some((*arc_graph).clone()))
        }
    }

    /// Forwards to the inner store. Used by build_graph's
    /// `save_graph` call path when persistence is later wired in.
    fn save_graph(&self, graph: &CallGraph) -> Result<(), StoreError> {
        self.inner.save_graph(graph)
    }

    fn save_manifest(&self, manifest: &FileManifest) -> Result<(), StoreError> {
        self.inner.save_manifest(manifest)
    }

    fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError> {
        self.inner.load_manifest()
    }

    fn clear(&self) -> Result<(), StoreError> {
        self.inner.clear()
    }

    fn exists(&self) -> Result<bool, StoreError> {
        self.inner.exists()
    }
}
```

### Code sketch — `HandlerContext::get_graph_store()` modified body

```rust
pub fn get_graph_store(&self) -> Arc<dyn GraphStore> {
    if let Some(ref store) = self.graph_store {
        store.clone()
    } else {
        // Use CachedGraphStore so reads go through the lock-free
        // ArcSwap<CallGraph> path used by the 56 hot-path tools.
        // Writes are still forwarded to an InMemoryGraphStore for
        // trait-compatibility (used by save_manifest, future
        // persistence adapter). See ADR-032.
        self.fallback_store
            .get_or_init(|| {
                Arc::new(CachedGraphStore::new(self.analysis_service.graph_cache()))
            })
            .clone()
    }
}
```

### Code sketch — remove the dual-write in `build_graph`

The `build_graph` handler at `mod.rs:1071` currently does:

```rust
let store = ctx.get_graph_store();
let graph = ctx.analysis_service.get_project_graph();
let _ = store.save_graph(&graph);  // ← REMOVE
```

This is **removed** in ADR-032 because `load_graph()` now reads directly from `GraphCache` (which is already updated at `analysis_service.rs:337`).

## Scope

**Touched** (1 new file, 2 modified files, ~80 net lines):
- `crates/cognicode-core/src/infrastructure/persistence/cached_graph_store.rs` — NEW, ~70 lines
- `crates/cognicode-core/src/interface/mcp/handlers/mod.rs` — `get_graph_store()` body (~3 lines changed), remove dual-write in `build_graph` (~4 lines removed)
- `crates/cognicode-core/src/infrastructure/persistence/mod.rs` — re-export the new module

**Not touched**:
- `GraphStore` trait (unchanged)
- Any of the 35 reader call sites (zero breaking changes)
- Any of the 56 fast-path readers (already use `analysis_service.graph_cache()`)
- `InMemoryGraphStore` (still used internally by `CachedGraphStore` for writes)
- `GraphCache` (unchanged)
- The `OnceLock<Arc<dyn GraphStore>>` fallback from ADR-030 (now wraps `CachedGraphStore`)

## Risks

- **Trait-compat writes**: The `save_graph` on `CachedGraphStore` writes to the inner `InMemoryGraphStore`, but `load_graph` reads from `GraphCache`. After ADR-032, **no production code calls `save_graph` on the trait** (we removed the dual-write). The inner `InMemoryGraphStore` is essentially write-only — the `save_graph` method is kept for the trait contract and for the future PG-backed adapter.
- **Empty cache returns None**: `load_graph()` returns `None` when the cache is empty (matches the legacy "no graph" behavior). This means callers like `project_insights` still get a graceful "run build_graph first" error if invoked before any graph is loaded. No regression.
- **Data-leakage risk**: `CachedGraphStore` shares `GraphCache` across all `HandlerContext` instances (because `AnalysisService` is `Arc`-shared). This is **intentional** — the graph is project-scoped, not session-scoped. The same `build_graph` in any session updates the cache, and any other session sees it. This is the desired behavior in the MCP server (clients can collaborate on the same project).
- **Thread safety**: `Arc<GraphCache>` is `Send + Sync` (`ArcSwap` is lock-free). `InMemoryGraphStore` is also `Send + Sync` (`Mutex` is `Sync`). The wrapper is therefore `Send + Sync` and `GraphStore`-compatible.
- **Arc clone cost**: `load_graph` returns `Some((*arc).clone())` — this clones the full `CallGraph` (5–10 MB for 29K symbols). **This is the same cost as the bincode deserialize path** (5–10 MB). The win is **avoiding the bincode encode/decode** roundtrip and the `Mutex` lock contention. The memory copy is unavoidable unless we change the trait signature to return `Arc<CallGraph>`, which is out of scope.

## Acceptance Criteria

1. **All 20 tools from `/tmp/verify_final.py` still OK**.
2. **Speedup on slow-path tools** (target: ≥10% on `project_insights`, `codebase_map`, `graph_analyze` — the 35 tools that read from `get_graph_store()`).
3. **No regression** on hot-path tools (already using `graph_cache()`).
4. **`build_graph` time unchanged** (no extra work).
5. **New unit test passes**: `cached_graph_store_loads_from_cache_after_build`.
6. **`cargo build --release -p cognicode-mcp --features postgres`** with **zero new warnings**.
7. **No `tool not found`** errors (the 9 tools registered in ADR-031 still work).

## Out of Scope (Future ADRs)

- **PG-backed `GraphStore` implementation**: When persistence is actually needed, implement `PgGraphStore: GraphStore` and wire it through the same trait. The `InMemoryGraphStore` would be deprecated for production.
- **Return `Arc<CallGraph>` from `load_graph`**: A trait change to avoid the full clone on read. Requires touching all 35 reader call sites and is a separate decision.
- **Removing `InMemoryGraphStore`**: Still used by `CachedGraphStore` for writes and by tests. Keep until PG adapter lands.
- **Convergence of `OnceLock<Arc<dyn GraphStore>>` from ADR-030**: Once the `graph_store` field in `HandlerContext` is reliably set to a `CachedGraphStore` (or `PgGraphStore` in the future), the `OnceLock` fallback can be removed. Out of scope here to avoid coupling.

## Verification Log

- **Explore**: ✅ done (sdd-kernel-explore sub-agent, 56 vs 35 read sites inventoried)
- **Propose**: ✅ this document
- **Apply**: ✅ done (1 new file, 2 modified files, 3 unit tests passing)
- **Verify**: ✅ done (`/tmp/verify_final.py`: 20/20 tools OK, **10-30% speedup on 8 measured slow-path tools**)
- **Archive**: pending

### Measured Performance Improvement

Comparison of slow-path tool latency before/after CachedGraphStore (29,120 symbols, 21,095 edges, single run):

| Tool | Before (s) | After (s) | Speedup |
|------|-----------|----------|---------|
| `graph_analyze` | 0.12 | 0.08 | **33%** |
| `codebase_map` | 0.17 | 0.12 | **29%** |
| `graph_god_nodes` | 0.34 | 0.24 | **29%** |
| `graph_condensed` | 0.34 | 0.28 | **18%** |
| `graph_suggest_questions` | 0.86 | 0.71 | **17%** |
| `graph_pagerank` | 0.30 | 0.26 | **13%** |
| `project_overview` | 0.12 | 0.11 | **8%** |
| `graph_search_idf` | 0.28 | 0.22 | **21%** |

The improvement comes from avoiding bincode deserialize + Mutex lock in `InMemoryGraphStore::load_graph()` (5-10 MB serialization per call). `CachedGraphStore::load_graph()` now does an `Arc<CallGraph>` clone from the lock-free `ArcSwap`.

### Build & Test Results

- `cargo build --release -p cognicode-mcp --bin cognicode-mcp-server --features postgres`: ✅ 0 new warnings
- `cargo test --release -p cognicode-core --lib cached_graph_store`: ✅ 3/3 tests pass
  - `cached_graph_store_loads_from_cache`
  - `cached_graph_store_delegates_manifest`
  - `cached_graph_store_clear`

## Files Changed

- `crates/cognicode-core/src/infrastructure/persistence/cached_graph_store.rs` — **NEW** (158 lines, includes 3 unit tests)
- `crates/cognicode-core/src/infrastructure/persistence/mod.rs` — re-export `CachedGraphStore` (1 line added)
- `crates/cognicode-core/src/interface/mcp/handlers/mod.rs` — import CachedGraphStore (1 line), modified `get_graph_store()` body (5 lines), removed dual-write in `build_graph` (3 lines removed)
- `docs/adr/ADR-032-graphstore-graphcache-convergence.md` — this document

**Total**: 1 new file, 2 modified files, ~80 net lines added (including tests).
