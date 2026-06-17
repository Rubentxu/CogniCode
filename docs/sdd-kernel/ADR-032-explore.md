# Kernel Exploration: GraphStore vs GraphCache Convergence (ADR-032)

## Current State

CogniCode's MCP server maintains **two parallel in-memory graph storage layers** that serve the same purpose. They were designed to be one system but diverged organically as the codebase grew. ADR-030 partially fixed the worst symptom (broken fallback) but explicitly deferred the architectural convergence to a future ADR — this one.

### Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────────────┐
│                          MCP Handler Path                                 │
│                                                                            │
│   build_graph ─────────────────────────────────────────────────────────┐  │
│   │                                                                     │  │
│   │  ┌──────────────────────────┐    ┌─────────────────────────────┐   │  │
│   ├─►│ AnalysisService          │    │ HandlerContext               │   │  │
│   │  │  graph_cache:            │    │  graph_store: Option<Arc<    │   │  │
│   │  │    ArcSwap<CallGraph> ◄──┼────┤    dyn GraphStore>>          │   │  │
│   │  │                          │    │  fallback_store:             │   │  │
│   │  │  set(call_graph) ────────┤    │    OnceLock<Arc<dyn GS>>     │   │  │
│   │  │  get() → Arc<CallGraph>  │    │                              │   │  │
│   │  └──────────┬───────────────┘    │  get_graph_store()           │   │  │
│   │             │                    │    → fallback_store          │   │  │
│   │             │                    │      .get_or_init(            │   │  │
│   │             │                    │        InMemoryGraphStore)    │   │  │
│   │             │                    └──────────┬───────────────────┘   │  │
│   │             │                               │                        │  │
│   │    ┌────────▼────────┐           ┌──────────▼───────────────────┐   │  │
│   │    │ Fast Path (56)  │           │ Slow Path (35)               │   │  │
│   │    │ ================│           │ =============================│   │  │
│   │    │ ★ get_hot_paths │           │ ✗ graph_pagerank             │   │  │
│   │    │ ★ get_entry_pts │           │ ✗ graph_condensed            │   │  │
│   │    │ ★ get_leaf_funcs│           │ ✗ graph_reduced              │   │  │
│   │    │ ★ trace_path    │           │ ✗ graph_feedback_arcs        │   │  │
│   │    │ ★ check_arch    │           │ ✗ graph_god_nodes            │   │  │
│   │    │ ★ analyze_impact│           │ ✗ graph_communities          │   │  │
│   │    │ ★ graph_insights│           │ ✗ graph_surprising_conns     │   │  │
│   │    │ ★ call_hierarchy│           │ ✗ graph_search_idf           │   │  │
│   │    │ ★ aix_handlers  │           │ ✗ graph_insights (via gs)    │   │  │
│   │    │   (17 sites)    │           │ ✗ graph_suggest_questions    │   │  │
│   │    │ ★ workspace_sess│           │ ✗ graph_all_paths            │   │  │
│   │    │   (11 sites)    │           │ ✗ find_pattern_by_intent     │   │  │
│   │    │                  │           │ ✗ detect_api_breaks          │   │  │
│   │    │ DIRECT Arc access│           │ ✗ graph_query                │   │  │
│   │    │ O(1) atomic load │           │ ✗ graph_explain              │   │  │
│   │    │ NO serialization │           │ ✗ find_usages variants(4)    │   │  │
│   │    │                  │           │ ✗ smart_search               │   │  │
│   │    │                  │           │ ✗ graph_analyze              │   │  │
│   │    │                  │           │ ✗ project_overview           │   │  │
│   │    │                  │           │ ✗ codebase_map               │   │  │
│   │    │                  │           │ ✗ project_insights           │   │  │
│   │    │                  │           │ ✗ review_pr                  │   │  │
│   │    │                  │           │ ✗ auto_diagnose              │   │  │
│   │    │                  │           │ ✗ solid_audit                │   │  │
│   │    │                  │           │                              │   │  │
│   │    │                  │           │ BINCODE deserialize O(N)     │   │  │
│   │    │                  │           │ Mutex lock per call          │   │  │
│   │    └──────────────────┘           └──────────────────────────────┘   │  │
│   │                                                                       │  │
│   │  ═══════════════════════════════════════════════════════════════════  │  │
│   │  DUAL-WRITE: both stores get the SAME CallGraph on build_graph        │  │
│   │  mod.rs:1060 → graph_cache.set(graph)      (ArcSwap, fast)            │  │
│   │  mod.rs:1071 → store.save_graph(&graph)     (bincode, Mutex)          │  │
│   │  ═══════════════════════════════════════════════════════════════════  │  │
│   │                                                                       │  │
└──────────────────────────────────────────────────────────────────────────┘
```

## Context Quality
- **Level: C1** — Strong code evidence, ADR-030 documents the dual-store problem explicitly, but no design doc for convergence exists
- **Evidence Present**: All source files, ADR-030, git history, 94 graph_store references, 56 get_project_graph references
- **Missing Context**: No performance benchmarks comparing bincode deserialize vs ArcSwap::load for 29K-symbol graphs; no PG-backed GraphStore impl yet
- **Recommended Effort**: verify (code is well-structured, the convergence path is clear from ADR-030's follow-up section)

## Knowledge Coverage
| Class | Status | Evidence | Gap Impact |
|------|--------|----------|------------|
| Roadmap/Backlog | missing | No explicit ADR for this convergence | Makes this the first formal document |
| Work Items | missing | No issue/task tracking | Tasks must be derived from this exploration |
| Architecture/ADRs | present | ADR-030 (fix), ADR-027/028 (consolidated handlers), ADR-026 (graph_query) | Clear justification chain |
| Ownership | present | Single maintainer (rubentxu), ADR-030 accepted 2026-06-16 | Low risk of conflict |
| Learnings | present | ADR-030 verification log shows 16/18 tools fixed by OnceLock cache; graph_pagerank hangs with 29K symbols (separate issue) | Pre-existing perf bugs documented |

## Problem Taxonomy
| Axis | Applies | Evidence |
|------|---------|----------|
| Domain modeling | Yes | Two concepts (cache vs store) that should be one — the `GraphStore` trait conflates "persistence" with "serving" |
| Boundary/seam | Yes | The seam between `AnalysisService` (owns GraphCache) and `HandlerContext` (owns GraphStore) is the root cause — data flows across this seam twice |
| Coupling/connascence | **Yes — CRITICAL** | 35 call sites depend on `get_graph_store().load_graph()` returning `Result<Option<CallGraph>>` pattern; 56 sites depend on `get_project_graph()` returning `Arc<CallGraph>` — two incompatible read patterns for the same data |
| API contract | Yes | `GraphStore::load_graph()` returns `Result<Option<CallGraph>>` (nilable, fallible) vs `GraphCache::get()` returns `Arc<CallGraph>` (always non-nil, infallible) — different semantics |
| Refactor/legacy | Yes | `InMemoryGraphStore` is used in production but documented as "for testing"; the bincode path exists for a PG adapter that doesn't exist yet |
| Event/CQRS | No | GraphCache has broadcast events but they're internal — not part of the convergence problem |
| Testing | No | InMemoryGraphStore has tests; GraphCache has tests — tests would need updating if trait changes |
| Security/operations | No | No security implications; operational risk is the continued dual-cache maintenance burden |

## Domain Language And Invariants
- **Domain Language**:
  - `GraphCache` (ArcSwap<CallGraph>): Serving store — direct in-memory access, no serialization, broadcast events
  - `GraphStore` (trait): Persistence abstraction — `save_graph`, `load_graph`, `save_manifest`, `load_manifest`, `clear`, `exists`
  - `InMemoryGraphStore`: Production fallback implementing `GraphStore` with bincode serialization behind `Mutex<Option<Vec<u8>>>`
  - `get_project_graph()`: Accessor on AnalysisService → returns `Arc<CallGraph>` from `GraphCache`
  - `get_graph_store()`: Accessor on HandlerContext → returns `Arc<dyn GraphStore>` (lazy fallback via OnceLock)
  - **Unresolved ambiguity**: "Store" means both "persistence store" and "serving store" — the community hasn't settled on distinct terms

- **Invariants**:
  - After `build_graph`, both `graph_cache.get()` and `get_graph_store().load_graph()` MUST return the same `CallGraph` (ADR-030 enforced this)
  - `get_graph_store()` MUST return the same instance across calls within a session (ADR-030's OnceLock guarantees this)
  - `GraphCache` is written BEFORE `GraphStore` in the build_graph flow (line 1060 before 1071)
  - `GraphStore::save_graph` serializes via bincode — takes `Mutex` lock — O(N) for 29K symbols

## Knowledge Gaps
- **No PG-backed GraphStore impl** — The entire `GraphStore` trait exists for a future PG adapter that doesn't exist yet. This means `InMemoryGraphStore` is the ONLY concrete production implementation, making the trait effectively unused as an abstraction.
- **No perf comparison data** — We know `ArcSwap::load()` is ~2× faster than bincode deserialize (ADR-030 claims this), but no microbenchmark exists. The benchmark at `crates/cognicode-core/benches/graph_benchmarks.rs` tests algorithms, not store access patterns.

## Affected Areas
- `crates/cognicode-core/src/interface/mcp/handlers/mod.rs:441-448` — `get_graph_store()` method (the convergence point)
- `crates/cognicode-core/src/interface/mcp/handlers/mod.rs:343-355` — `HandlerContext` struct fields (`graph_store`, `fallback_store`)
- `crates/cognicode-core/src/interface/mcp/handlers/graph_handlers.rs` — 12 handlers reading from store (lines 30, 75, 158, 194, 230, 264, 300, 344, 408, 463, 511, 577)
- `crates/cognicode-core/src/interface/mcp/handlers/consolidated_handlers.rs` — 7 handlers reading from store (lines 42, 66, 96, 127, 152, 183, 213, 257)
- `crates/cognicode-core/src/interface/mcp/handlers/graph_query_handlers.rs` — 10 handler sites (lines 59, 205, 310, 320, 330, 340, 351, 364, 382)
- `crates/cognicode-core/src/interface/mcp/handlers/aix_handlers.rs:1980` — `reparse_on_edit` uses `get_graph_store()` for manifest load
- `crates/cognicode-core/src/interface/mcp/handlers/mod.rs:1048,1071,3449` — build_graph (dual-write) and solid_audit
- `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs:107-111` — `CogniCodeHandler::get_call_graph()` helper
- `crates/cognicode-core/src/infrastructure/persistence/memory_graph_store.rs` — `InMemoryGraphStore` impl (to become test-only)
- `crates/cognicode-core/src/domain/traits/graph_store.rs` — `GraphStore` trait (unchanged, persistence-only)
- `crates/cognicode-core/src/infrastructure/graph/graph_cache.rs` — `GraphCache` (unchanged)

## Options

### Approach A: CachedGraphStore — Delegate load_graph() to GraphCache (MINIMAL)
Make `get_graph_store()` return a wrapper `GraphStore` impl whose `load_graph()` reads from `ctx.analysis_service.graph_cache()` (ArcSwap), eliminating bincode deserialize on every read. `save_graph()` writes to GraphCache (redundant but harmless dual-write becomes same-store write). Manifest methods delegate to inner `InMemoryGraphStore`.

| Pros | Cons | Effort |
|------|------|--------|
| 0 call site changes (35+ sites unchanged) | `load_graph()` still clones the full CallGraph (same as bincode cost) | Low |
| No handler changes | Manifest persistence still goes through bincode (minor) | ~60 new lines |
| No trait signature change | `GraphStore` trait still used for in-memory serving (not ideal) | files: 2-3 |
| Single source of truth for graph data | `clear()` and `exists()` semantics differ between cache and store | |
| Backward compatible — no public API break | | |
| `InMemoryGraphStore` remains for tests | | |

**Code sketch**:
```rust
// New struct in memory_graph_store.rs or handlers/mod.rs
struct CachedGraphStore {
    cache: Arc<GraphCache>,
    inner: InMemoryGraphStore,  // for manifest persistence only
}

impl GraphStore for CachedGraphStore {
    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        let arc = self.cache.get();
        if arc.symbol_count() == 0 {
            Ok(None)
        } else {
            Ok(Some((*arc).clone()))
        }
    }
    fn save_graph(&self, graph: &CallGraph) -> Result<(), StoreError> {
        self.cache.set(graph.clone()); // source of truth
        self.inner.save_graph(graph)   // also persist for cache-hit path
    }
    fn save_manifest(&self, m: &FileManifest) -> Result<(), StoreError> {
        self.inner.save_manifest(m)
    }
    fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError> {
        self.inner.load_manifest()
    }
    fn clear(&self) -> Result<(), StoreError> {
        self.cache.clear();
        self.inner.clear()
    }
    fn exists(&self) -> Result<bool, StoreError> {
        Ok(self.cache.get().symbol_count() > 0)
    }
}

// In HandlerContext::get_graph_store():
pub fn get_graph_store(&self) -> Arc<dyn GraphStore> {
    if let Some(ref store) = self.graph_store {
        store.clone()
    } else {
        self.fallback_store
            .get_or_init(|| {
                Arc::new(CachedGraphStore::new(
                    self.analysis_service.graph_cache(),
                ))
            })
            .clone()
    }
}
```

### Approach B: Migrate All Readers to get_project_graph() (FULL CONVERGENCE)
Change all 35 `get_graph_store().load_graph()` call sites to `ctx.analysis_service.get_project_graph()`, eliminating the entire `GraphStore` read path. Different call sites would need different adaptation (some use `Ok(Some(graph))` match, some `.unwrap_or(error)`).

| Pros | Cons | Effort |
|------|------|--------|
| True single source of truth | 35+ call site changes, different patterns | Medium-High |
| Eliminates bincode path entirely | Every handler's error handling changes | 35 sites in 5+ files |
| `GraphStore` trait becomes persistence-only | Risk of regression (each site needs testing) | |
| `InMemoryGraphStore` → test-only trivially | Manifest persistence still needs a home | |
| No clone on read (returns `Arc<CallGraph>`) | Breaking: callers that mutate graph after load break | |

### Approach C: Deprecate InMemoryGraphStore, Keep Trait for PG Adapter
Move `InMemoryGraphStore` to `#[cfg(test)]`, make `HandlerContext.graph_store` always `Some(...)` with a lazy GraphCache-backed impl. Removes the dual-cache ambiguity at the type level.

| Pros | Cons | Effort |
|------|------|--------|
| Type system enforces single source | Requires builder/constructor changes | Medium |
| Clear separation: serving vs persistence | Existing `Option<Arc<dyn GraphStore>>` field semantics change | ~8 files |
| `InMemoryGraphStore` is clearly test-only | `fallback_store` field becomes redundant (ADR-030 cleanup) | |

### Approach D: Unified CacheLayer Abstraction
Define a `CacheLayer` trait that both `GraphCache` and `GraphStore` implement, with `load_cached() -> Arc<CallGraph>`. Redesign the serving path around this unified trait.

| Pros | Cons | Effort |
|------|------|--------|
| Clean architecture for future backends | Heavy abstraction for a single implementation | High |
| Solves the problem definitively | Changes trait hierarchy (risk of cascading changes) | 10+ files |
| | Over-engineered for the current need (no PG impl exists) | |

## Entropy Envelope
- **Method**: Heuristic (code reading); CogniCode graph not built for this project
- **Coupling risk**: HIGH — 91 read sites across two incompatible patterns, 35 of which use the slow path
- **Connascence analysis**:
  - `get_graph_store().load_graph()` pattern: I(Name) ≈ log2(35) ≈ 5.13 bits — CRITICAL (35 sites coupled by identical call shape)
  - `get_project_graph()` pattern: I(Name) ≈ log2(56) ≈ 5.81 bits — CRITICAL (56 sites, but this is the GOOD path)
  - Dual-write (graph_store ↔ graph_cache): I(Algorithm) ≈ 2 bits — both stores must stay in sync
  - `HandlerContext` → `Arc<OnceLock<Arc<dyn GraphStore>>>`: I(Position) pattern from ADR-030, now obsoletable
- **SOLID-Entropy**:
  - **SRP**: `HandlerContext` violates SRP — it owns both a `graph_store` and a `fallback_store` AND delegates to `analysis_service.graph_cache`. F = H(3 fields) - H(fields | purpose="graph serving") ≈ 1.58 - 0 = 1.58 bits of entropy not explained by purpose.
  - **DIP**: Both `GraphStore` (trait) and `GraphCache` (concrete ArcSwap) are depended on. H(GraphStore trait) ≈ 5.13 bits (35 consumers), H(GraphCache concrete) ≈ 5.81 bits (56 consumers). No violation — both are reasonably abstract.
  - **OCP**: Current design requires modification of 35 call sites to extend (change the read pattern). H(Δ_existing) ≈ log2(35) ≈ 5.13 bits — OCP VIOLATED.
- **Design Quality Score**: ≈ 0.25/1.0 — NEEDS REFACTORING (poor cohesion from dual-cache, moderate coupling from divergent read patterns)

## Recommendation

**Pick Approach A — CachedGraphStore delegation.**

Rationale:
1. **Minimal invasion**: 0 call site changes. The 35 slow-path readers continue to work with identical semantics. The `GraphStore` trait and `InMemoryGraphStore` remain unchanged — no test breakage.
2. **Immediate perf win**: All 35 `get_graph_store().load_graph()` calls bypass bincode deserialize. The only cost is a `CallGraph::clone()` (~50ms for 29K symbols), which is comparable to what bincode does anyway. The `Mutex` contention is eliminated because `ArcSwap::load()` is lock-free.
3. **ADR-030 alignment**: This is the exact convergence path ADR-030's follow-up section described: "Make ArcSwap<CallGraph> the single source of truth for in-memory reads."
4. **Future-proof**: When a PG-backed `GraphStore` arrives, it replaces `self.graph_store` (the `Some` branch). The `CachedGraphStore` fallback remains for standalone mode. No architecture change needed.
5. **Rollback safety**: If the `CallGraph::clone()` proves too expensive at scale, we can add a `get_ref()` path later without changing the `GraphStore` trait.
6. **Dual-write becomes idempotent**: The current redundant write at `mod.rs:1071` (`store.save_graph(&graph)`) now writes to the same GraphCache that line 1060 already wrote. No data inconsistency possible.

**Variant selection**: The `CachedGraphStore` struct should live in `crates/cognicode-core/src/infrastructure/persistence/` alongside `InMemoryGraphStore`, or in `handlers/mod.rs` as a private module since it's only used by `get_graph_store()`. I recommend `memory_graph_store.rs` (same module as InMemoryGraphStore) to keep persistence-related code together.

## Risks
- **Clone cost**: `CallGraph::clone()` with 29K symbols allocates ~5-10 MB. This is comparable to bincode deserialize but needs verification. Mitigation: run the existing `graph_benchmarks.rs` with a clone-vs-bincode comparison.
- **Manifest persistence**: `CachedGraphStore` still uses bincode for manifest persistence via inner `InMemoryGraphStore`. This is acceptable — manifests are tiny (~100 KB) and the bincode cost is negligible.
- **`clear()` semantics**: `CachedGraphStore::clear()` clears both GraphCache and inner store. If a tool calls `clear()` then expects `load_graph()` to return `None`, it will (GraphCache is empty after clear). Verified: no tool calls `clear()` directly.
- **`exists()` semantics**: Changed from "data exists in bincode buffer" to "graph has symbols". This is actually more correct.

## Out of Scope
- Removing the bincode path entirely for manifest persistence (future PG adapter will handle that)
- Changing `GraphStore` trait signature (e.g., returning `Arc<CallGraph>` instead of `Option<CallGraph>`)
- Migrating individual handlers to use `get_project_graph()` directly (Approach B — separate effort)
- Removing `InMemoryGraphStore` (still needed for test infrastructure and manifest persistence)
- Fixing `graph_pagerank` performance (separate ADR-031 candidate, documented in ADR-030 verification log)
- Implementing PG-backed `GraphStore` (future work)

## Ready For Proposal
**Yes** — The architecture is clearly understood. The convergence path is well-defined by ADR-030's follow-up section and confirmed by this exploration. Approach A has zero ambiguity: one new struct, one modified method body, zero call site changes. The sdd-kernel-propose agent can proceed immediately.
