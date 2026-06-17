# SDD Kernel Archive Report — ADR-035 Graph Checkpointing

**Change**: ADR-035 Graph Checkpointing
**Date**: 2026-06-17
**Verdict**: PASS WITH WARNINGS
**Commits**: 2 (afbb76c PR-1 core infra, 2ee1f59 PR-2 API surface + ADR)

---

## Executive Summary

ADR-035 introduces checkpoint-based snapshot isolation for graph reads via a monotonic `CheckpointId` versioning system, a `VersionedGraphCache` ring buffer (FIFO, retention=2), and `GraphStore` trait extensions. Implementation is complete and all 16 new + 1350 existing tests pass.

---

## Verified Deliverables

| Deliverable | Status |
|-------------|--------|
| `CheckpointId` newtype (monotonic u64) in `domain/value_objects/` | ✅ |
| `VersionedGraphCache` VecDeque ring with FIFO retention (default 2) | ✅ |
| `GraphCache` refactored to wrap `ArcSwap<VersionedGraphCache>` | ✅ |
| `GraphStore` trait: `current_checkpoint_id()` + `checkpoint_at(id)` with default panicking impls | ✅ |
| `StoreError::CheckpointNotFound(CheckpointId)` variant | ✅ |
| 3 production impls: `CachedGraphStore`, `InMemoryGraphStore`, `MockGraphStore` | ✅ |
| 6 integration tests + 10 unit tests passing | ✅ |
| `docs/adr/ADR-035-graph-checkpointing.md` (329 lines) | ✅ |
| 35 existing `load_graph()` callers unchanged (backward compat) | ✅ |

---

## Warnings

| # | Warning | Resolution |
|---|---------|------------|
| W1 | Method name drift: `load_graph_at` (spec SC) vs `checkpoint_at` (impl) — semantics identical | Documented in ADR |
| W2 | `current_checkpoint_id` returns `Option<CheckpointId>` not `Result<CheckpointId, StoreError>` | Spec was imprecise; implementation correct |
| S1 | SC-9 `/ready` checkpoint binding not covered by tests | Future work |

---

## Files Changed

**4 new files**:
- `domain/value_objects/checkpoint_id.rs` — CheckpointId newtype
- `graph/cache/versioned_graph_cache.rs` — VersionedGraphCache ring buffer
- `graph/store/*.rs` (trait extensions + 3 impls)
- `docs/adr/ADR-035-graph-checkpointing.md`

**7 modified files**:
- `graph/cache/mod.rs` — GraphCache refactor
- `graph/store/*.rs` — trait + impls
- `Cargo.toml` — new dependencies
- Various test files

---

## Knowledge Updates (Durable)

1. **`CheckpointId`** is a new domain value object: monotonic u64, id=0 reserved as sentinel/uninitialized
2. **`VersionedGraphCache`** provides FIFO ring with configurable retention (default 2)
3. **`GraphStore` trait** has 2 new methods: `current_checkpoint_id()` and `checkpoint_at(id)`
4. **Snapshot isolation** for graph reads is now possible via `checkpoint_at(id)`
5. **ADR-035 resolves the "future work"** item from M2 archive

---

## Test Results

- **16 new tests**: PASS
- **1350 existing tests**: PASS
- **Total**: 1366 tests passing

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Method name drift causing confusion | Low | Low | Documented in ADR |
| SC-9 `/ready` binding gap | Low | Medium | Future work |
| CheckpointId=0 sentinel edge case | Low | Low | Reserved but not yet enforced |

---

## Next Recommended

1. **Address SC-9 `/ready` binding gap** — add integration test coverage for `/ready` checkpoint binding
2. **Enforce CheckpointId=0 as invalid** — add validation in `CheckpointId::new()`
3. **Persist checkpoint metadata** — current implementation is in-memory only; consider PG persistence for durability

---

*Archived by sdd-kernel-archive on 2026-06-17*
