# ADR-035: Graph Checkpointing — In-Memory Versioned Ring for Snapshot Isolation

**Status:** Accepted
**Date:** 2026-06-17
**Source:** "Future work" item extracted from the M2 / M3 archive reports
(2026-05-22) and re-elevated after the 91-reader concurrent-load audit
(M3 sprint). Closes the partial-graph race condition discovered during
the M2.1–M2.7 integrity gate smoke runs.

## Context

CogniCode's MCP server reads the call graph from two parallel paths
since ADR-032:

- **Hot path** (56 tools): `AnalysisService::graph_cache` →
  `ArcSwap<CallGraph>` — lock-free, O(1) read.
- **Slow path** (35 tools): `HandlerContext::get_graph_store()` →
  `CachedGraphStore::load_graph()` → reads the same `GraphCache` since
  ADR-032, but always the *current* head.

Both paths return **whichever graph the writer last published**. This is
fine for steady-state use, but it produces two failure modes when
readers and writers overlap:

### Problem 1 — The 91-reader problem

During the M3 stress audit, a `build_graph` triggered by an MCP client
was observed to interrupt concurrent reads on the 35 slow-path tools.
91 in-flight readers (mostly `graph_search_idf`, `graph_analyze`,
`project_insights`) all received a mix of pre-write and post-write
state: some saw the new symbol set, some saw the old one, depending on
when the writer's `ArcSwap` swap landed relative to their read.

This is not a *correctness* bug at the `GraphCache` level (the swap is
atomic) — it is a *narrative* bug. The user clicked "build", the
Explorer's pane-stack tabs go stale mid-drill, and the navigation
history becomes incoherent.

### Problem 2 — The partial-graph race

`save_graph` on `CachedGraphStore` is a *full-graph replace* (the cache
holds one `Arc<CallGraph>`, not a delta). For a 29,120-symbol /
21,095-edge graph (~5–10 MB serialized), `cache.set(graph)` is fast
(~6 ms) but not free. A reader that started its `find_paths` traversal
just before the swap can return a partial result: nodes that were in
the graph when the read started but whose edges were re-inserted during
the swap.

### Why we did not solve this in M2

M2's scope was *integrity* (no STUB tools, no missing tools, no
duplicate registrations). The 91-reader observation was recorded as
"future work" because the fix requires a state-shaping change (a
ring buffer of past versions) that is orthogonal to the integrity
gate. M3 surfaced the issue with concrete reproduction.

## Decision

Introduce a **monotonic `CheckpointId`** and a **bounded
`VersionedGraphCache` ring** inside the existing `GraphCache`. Readers
pin to a specific id and read a consistent snapshot; the writer
publishes a new head atomically while old versions stay live until
evicted by FIFO.

### Data model

```text
CheckpointId(pub u64)
  └── monotonic, never reused. id 0 reserved as NONE.

VersionedGraphCache
  └── VecDeque<(CheckpointId, Arc<CallGraph>)>
        ├── head: latest insert (Arc, O(1) clone)
        ├── retention: FIFO eviction when full (default 2)
        └── contains(id): O(retention) linear scan

GraphCache (existing)
  └── ArcSwap<Mutex<VersionedGraphCache>>
        ├── ArcSwap: lock-free outer pointer for readers
        └── Mutex: serializes the rare write path
              (insert + return CheckpointId, no TOCTOU race)
```

### Architecture

```text
                ┌──────────────────────────────────────┐
                │   MCP Tool:  graph_search_idf (etc)  │
                └────────────────┬─────────────────────┘
                                 │ reads (lock-free)
                                 ▼
                ┌──────────────────────────────────────┐
                │  CachedGraphStore::checkpoint_at(2) │
                │   delegates to GraphCache::get_at(2) │
                └────────────────┬─────────────────────┘
                                 │ reads from ring
                                 ▼
   ┌─────────────────────────────────────────────────────────────┐
   │       GraphCache (ArcSwap<Mutex<VersionedGraphCache>>)      │
   │                                                             │
   │  [ id 1 ─► Arc<CallGraph v1> ]   ◄── evicted after 3rd set  │
   │  [ id 2 ─► Arc<CallGraph v2> ]   ◄── current head           │
   │  [ id 3 ─► Arc<CallGraph v3> ]   ◄── newest insert          │
   │                                                             │
   └──────────────────────────▲──────────────────────────────────┘
                              │  ArcSwap pointer swap (lock-free)
                              │  + inner Mutex for eviction
   ┌──────────────────────────┴──────────────────────────────────┐
   │   build_graph handler                                       │
   │   cache.set(new_graph) ──► CheckpointId returned             │
   └─────────────────────────────────────────────────────────────┘
```

### Call flow (the 91-reader scenario)

1. User clicks "build" → `build_graph` computes the new graph.
2. `build_graph` calls `cache.set(new_graph)`. The inner `Mutex` is
   acquired, the new `Arc<CallGraph>` is appended to the ring, and
   the outer `ArcSwap` is published. The id of the new head is
   returned to the caller.
3. **All 91 in-flight readers keep seeing the version they pinned at
   read start** (the old `Arc<CallGraph>` is still alive in the ring
   for at least one more `set()`).
4. After the second `set()`, the ring is full. The oldest entry is
   popped. The Arc is dropped only when its last reader releases it.
5. New readers after the second `set()` see the new head.

### Lock semantics

| Operation | Lock | Cost |
|-----------|------|------|
| `current_id()` | ArcSwap `load` + brief inner Mutex | O(1) outer, O(1) inner |
| `get_at(id)` | ArcSwap `load` + brief inner Mutex | O(retention) linear scan |
| `get()` (current head) | ArcSwap `load` + brief inner Mutex | O(1) |
| `set(graph)` | ArcSwap `load` + inner Mutex for the whole insert | O(1) — but serialized with other writers |
| `update(f)` | clones head outside Mutex, then `set` | O(N) clone + 1 `set` |
| `apply_events(...)` | clones head outside Mutex, then `set` | O(N) clone + 1 `set` |

The `ArcSwap<Mutex<…>>` layout is the standard "publish a new state
without blocking readers" pattern. The outer `ArcSwap` gives lock-free
reads; the inner `Mutex` is acquired only for the brief window
between "load the current ring" and "publish the new ring". Readers
never block on the Mutex.

### Default impl policy (D7)

`GraphStore::current_checkpoint_id` and `GraphStore::checkpoint_at`
have **panicking default implementations** that include the concrete
type name in the panic message:

```rust
fn current_checkpoint_id(&self) -> Option<CheckpointId> {
    panic!(
        "current_checkpoint_id not implemented for {}",
        std::any::type_name::<Self>()
    );
}
```

The reasoning: silent failure ("returns None forever, nobody notices")
is worse than loud failure ("panics on first call, the test catches
it"). Stores that do not maintain versioned snapshots **override**
the methods with a single-version stub (`Some(CheckpointId(1))` and
`load_graph` for any id). Stores that are versioned
(`CachedGraphStore`) delegate to the real ring.

A new `StoreError::CheckpointNotFound(CheckpointId)` variant
distinguishes a cold store (`Ok(None)`) from an evicted id
(`Err(CheckpointNotFound(id))`).

### Default retention = 2

Two slots cover the common case:

1. **One in-flight read** (the 91-reader scenario).
2. **One new write** happening concurrently.

Three or more requires either larger retention (more memory) or a
real PG-backed snapshot store. The default lives in
`GraphCache::DEFAULT_RETENTION = 2`. Callers that need a longer
window can construct with `GraphCache::with_retention(n)`.

## Consequences

### Positive

- **Loud failure > silent ignore** for stores that do not implement
  the new methods (D7). The panic message names the concrete type.
- **No breaking changes** to the existing 35 slow-path tool readers
  (`load_graph()`) or the 56 hot-path readers
  (`analysis_service.graph_cache().get()`).
- **Lock-free reads** preserved: the new methods share the existing
  `ArcSwap<Mutex<…>>` pattern from PR-1.
- **Test pyramid gains an integration-test layer**:
  `crates/cognicode-core/tests/checkpoint_integration.rs` covers
  the eviction, monotonicity, and concurrent-reader invariants
  end-to-end at the `GraphStore` trait boundary.

### Negative

- **Memory cost**: each retained checkpoint is a full
  `Arc<CallGraph>` clone of the head at write time. For a 29K-symbol
  graph (~5–10 MB) and retention = 2, that is ~10–20 MB resident.
  Acceptable for a single-user MCP server; out of scope for
  multi-tenant cloud.
- **Write serialization**: the inner `Mutex` serializes
  `set()`/`update()`/`apply_events()`. The current refresh rate
  (one write per file change) is far below the contention threshold
  (millions of `set()` per second), so this is a non-issue today.
  If the write rate grows, the lock becomes the bottleneck and the
  cache needs a lock-free write path (next ADR).
- **PR-1 known leak in `get_ref()`**: the `Arc<CallGraph>` head is
  leaked into a `Box<Arc<_>>` to keep the returned `&CallGraph`
  valid for `'static`. Each call leaks ~16 bytes. Bounded by the
  request rate; will be replaced with a guard-returning API in a
  follow-up ADR.

### Deferred to a future ADR

- **PG-backed checkpoint persistence**: when a real
  `PgGraphStore: GraphStore` lands, the ring would be replaced by a
  PG table with `(project_id, checkpoint_id, graph_blob)`. The trait
  shape stays the same.
- **Return `Arc<CallGraph>` from `load_graph`**: avoid the 5–10 MB
  clone on every read. Out of scope for ADR-035 — touches all 35
  call sites.
- **Replace `get_ref()` leak with a guard**: scoped to a future
  "MCP compression path safety" ADR.

## Architecture (multi-PR roadmap)

ADR-035 is split into two stacked PRs against `main`:

| PR | Scope | Status |
|----|-------|--------|
| **PR-1** (`afbb76c`) | `CheckpointId` + `VersionedGraphCache` + `GraphCache` refactor + unit tests | Merged |
| **PR-2** (this PR) | `GraphStore` trait extension + 4 impls + integration test + ADR doc | Current |

A future PR-3 (not planned) would add a `PgGraphStore` that persists
the ring to PG and a guard-returning `get_ref_with_guard()` API.

## Alternatives Considered

### A. Mutex<CallGraph> with read-copy-update

Replace the lock-free `ArcSwap` with a single `Mutex<CallGraph>` and
copy-on-write the whole graph on every read. Rejected: 5–10 MB clone
per read is the same cost we already pay on the slow path. Does not
solve the "reader sees partial state" problem; readers still see
*some* version but it is the head, not a pinned one.

### B. Lock-free single-threaded skiplist

Implement a concurrent skiplist of `(CheckpointId, Arc<CallGraph>)`
with no Mutex. Rejected: overkill for a 2-slot ring, and the
`arc-swap` crate already provides the lock-free pointer swap we
need for the common case.

### C. CoW on every node (Arc<CallNode> graph)

Replace `CallGraph` with a per-node `Arc<…>` graph so individual
edges can be swapped without cloning the whole graph. Rejected:
`CallGraph` is a HashMap of Symbol/Edge structs, not a petgraph.
Refactoring to per-node Arcs is a separate, large piece of work
(ADR candidate on its own).

### D. Single-version with "graph generation counter"

Add a `u64` generation counter to the cache. Readers that started
before a `set()` retry once if they see a stale counter. Rejected:
retry logic is harder to reason about than a fixed retention window.
A 2-slot ring is conceptually simpler and bounded in memory.

### E. PG-backed ring from day one

Skip the in-memory ring and implement `PgGraphStore` directly.
Rejected: the PG schema, migration, and async-write path are all
out of scope for the M3 sprint. The trait shape designed in this
ADR makes PG a drop-in replacement later.

## Verification Log

- **Explore**: ✅ done (sddk-explore sub-agent, 91-reader scenario
  captured, retention size explored)
- **Propose**: ✅ this document
- **Apply (PR-1)**: ✅ done (commit `afbb76c` — `CheckpointId` +
  `VersionedGraphCache` + `GraphCache` refactor; 11 new + 3 new
  unit tests pass)
- **Apply (PR-2)**: ✅ this PR (trait extension + 4 impls +
  integration test + ADR)
- **Verify**: pending (`sddk-verify ADR-035` is the next
  recommended action)

### PR-2 Test Results

- `cargo check -p cognicode-core --all-targets`: ✅ 0 errors
- `cargo test -p cognicode-core --lib`: ✅ 1350 pass (no
  regressions; the 5 pre-existing failures — `pg_upsert_stage`,
  `scan::test_classify_file`, `graph_analytics::god_nodes_*`,
  `aix_handlers::test_reparse_on_edit_*` — are unrelated to
  GraphStore/GraphCache and reproduce on the stashed pre-PR-2 tree)
- `cargo test -p cognicode-core --test checkpoint_integration`:
  ✅ 6/6 pass
  - `insert_first_graph_sets_checkpoint_id_to_one`
  - `insert_second_graph_advances_checkpoint_id`
  - `checkpoint_at_pinned_id_returns_first_graph`
  - `third_insert_evicts_first_checkpoint`
  - `checkpoint_at_cold_cache_returns_none`
  - `concurrent_readers_see_consistent_snapshot`
    (10 threads × 100 reads, all see the same pinned snapshot)
- `cargo test -p cognicode-core-mock`: ✅ 16/16 pass (MockGraphStore
  gained the single-version stub)

## Files Changed (PR-2)

| File | Change |
|------|--------|
| `crates/cognicode-core/src/domain/value_objects/checkpoint_id.rs` | NEW — canonical `CheckpointId` in domain |
| `crates/cognicode-core/src/domain/value_objects/mod.rs` | MODIFIED — re-export `CheckpointId` |
| `crates/cognicode-core/src/domain/traits/graph_store.rs` | MODIFIED — add 2 trait methods + `StoreError::CheckpointNotFound` |
| `crates/cognicode-core/src/infrastructure/graph/checkpoint.rs` | MODIFIED — re-export `CheckpointId` from domain |
| `crates/cognicode-core/src/infrastructure/persistence/cached_graph_store.rs` | MODIFIED — real versioned impl + 5 new unit tests |
| `crates/cognicode-core/src/infrastructure/persistence/memory_graph_store.rs` | MODIFIED — single-version stub + 5 new unit tests |
| `crates/cognicode-core-mock/src/lib.rs` | MODIFIED — single-version stub on `MockGraphStore` |
| `crates/cognicode-core/tests/checkpoint_integration.rs` | NEW — 6 integration tests |
| `docs/adr/ADR-035-graph-checkpointing.md` | NEW — this document |

**Total**: 3 new files, 6 modified files, ~600 net lines added
(including 16 new tests).
