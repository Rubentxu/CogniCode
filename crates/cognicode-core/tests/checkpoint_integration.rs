//! Integration tests for ADR-035 graph checkpointing at the `GraphStore`
//! trait boundary.
//!
//! These tests exercise the end-to-end snapshot-isolation guarantee that
//! a `CachedGraphStore` provides when wrapped around a real
//! `GraphCache` with `retention = 2`. They cover:
//!
//! 1. The `current_checkpoint_id` advances monotonically with each
//!    `save_graph` (and matches the cache's `set()` return value).
//! 2. `checkpoint_at` returns a pinned snapshot for an id that is
//!    still in the retention window.
//! 3. `checkpoint_at` returns `Err(StoreError::CheckpointNotFound)`
//!    for an id that has been evicted.
//! 4. Concurrent readers see consistent snapshots — no torn reads —
//!    when many threads call `checkpoint_at` on the same id.
//!
//! The test uses `GraphCache::with_retention(2)` (the minimum that
//! keeps current + previous) so the eviction behaviour is easy to
//! trigger.

use std::collections::HashSet;
use std::sync::Arc;

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::traits::graph_store::{GraphStore, StoreError};
use cognicode_core::domain::value_objects::{Location, SymbolKind, CheckpointId};
use cognicode_core::infrastructure::graph::GraphCache;
use cognicode_core::infrastructure::persistence::CachedGraphStore;

/// Build a `CallGraph` whose symbol count is `n` and whose symbol
/// names are `s0`, `s1`, … `s{n-1}`. Used to assert that a pinned
/// snapshot is the right version and not a mix of two writes.
fn build_named_graph(prefix: &str, n: usize) -> CallGraph {
    let mut g = CallGraph::new();
    for i in 0..n {
        let s = Symbol::new(
            format!("{prefix}_{i}"),
            SymbolKind::Function,
            Location::new(format!("{prefix}.rs"), i as u32, 0),
        );
        g.add_symbol(s);
    }
    g
}

#[test]
fn insert_first_graph_sets_checkpoint_id_to_one() {
    let cache = Arc::new(GraphCache::with_retention(2));
    let store = CachedGraphStore::new(cache.clone());

    assert_eq!(store.current_checkpoint_id(), None);

    cache.set(build_named_graph("a", 3));
    let id = store.current_checkpoint_id().expect("id should exist");
    assert_eq!(id, CheckpointId(1));
}

#[test]
fn insert_second_graph_advances_checkpoint_id() {
    let cache = Arc::new(GraphCache::with_retention(2));
    let store = CachedGraphStore::new(cache.clone());

    let id1 = cache.set(build_named_graph("a", 1));
    let id2 = cache.set(build_named_graph("b", 1));
    assert_eq!(store.current_checkpoint_id(), Some(id2));
    assert!(id2 > id1, "ids must be strictly monotonic");
}

#[test]
fn checkpoint_at_pinned_id_returns_first_graph() {
    let cache = Arc::new(GraphCache::with_retention(2));
    let store = CachedGraphStore::new(cache.clone());

    // First graph has 5 symbols named a_0..a_4.
    let id1 = cache.set(build_named_graph("a", 5));
    // Second graph has 7 symbols named b_0..b_6.
    let _id2 = cache.set(build_named_graph("b", 7));

    // Pin to id1 — the previous version is still in the ring.
    let pinned = store
        .checkpoint_at(id1)
        .expect("id1 should still be in the ring")
        .expect("id1 should be Some, not None");
    assert_eq!(pinned.symbol_count(), 5);
    // Make sure the pinned snapshot is the *first* one, not a mix
    // with the second. We pick a name that only exists in the first.
    assert!(
        !pinned.find_by_name("a_0").is_empty(),
        "pinned snapshot should be the first graph (has a_0)"
    );
    assert!(
        pinned.find_by_name("b_0").is_empty(),
        "pinned snapshot must NOT contain b_0 from the second write"
    );
}

#[test]
fn third_insert_evicts_first_checkpoint() {
    let cache = Arc::new(GraphCache::with_retention(2));
    let store = CachedGraphStore::new(cache.clone());

    let id1 = cache.set(build_named_graph("a", 1));
    let _id2 = cache.set(build_named_graph("b", 1));
    let _id3 = cache.set(build_named_graph("c", 1));

    // id1 has been evicted (retention = 2, so only id2 and id3 remain).
    let result = store.checkpoint_at(id1);
    assert!(
        matches!(result, Err(StoreError::CheckpointNotFound(id)) if id == id1),
        "expected CheckpointNotFound({id1}), got {result:?}"
    );
}

#[test]
fn checkpoint_at_cold_cache_returns_none() {
    let cache = Arc::new(GraphCache::with_retention(2));
    let store = CachedGraphStore::new(cache);

    // Cold cache: nothing has been published.
    let result = store.checkpoint_at(CheckpointId(1)).unwrap();
    assert!(result.is_none(), "cold cache should return Ok(None)");
    assert_eq!(store.current_checkpoint_id(), None);
}

#[test]
fn concurrent_readers_see_consistent_snapshot() {
    // 10 threads, each calls checkpoint_at(2) 100 times concurrently.
    // All 1000 reads must succeed and return the same snapshot
    // (same symbol count, same first symbol name).
    let cache = Arc::new(GraphCache::with_retention(4));
    let store = CachedGraphStore::new(cache.clone());

    // Build three checkpoints: id1 (small), id2 (big), id3 (different).
    let _id1 = cache.set(build_named_graph("alpha", 10));
    let id2 = cache.set(build_named_graph("beta", 100));
    let _id3 = cache.set(build_named_graph("gamma", 50));

    // Sanity: id2 is still in the ring (retention 4, so 1, 2, 3, 4
    // would all be present after one more insert; here we have 1, 2, 3).
    assert!(store.checkpoint_at(id2).is_ok());

    let reader_count = 10;
    let reads_per_reader = 100;
    let barrier = Arc::new(std::sync::Barrier::new(reader_count));

    let mut handles = Vec::with_capacity(reader_count);
    for reader_id in 0..reader_count {
        let cache_for_thread = cache.clone();
        let barrier = barrier.clone();
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            let store = CachedGraphStore::new(cache_for_thread);
            let mut symbol_counts = HashSet::new();
            let mut first_names = HashSet::new();
            for _ in 0..reads_per_reader {
                let arc = store
                    .checkpoint_at(id2)
                    .expect("id2 should still be in the ring")
                    .expect("snapshot should be Some");
                symbol_counts.insert(arc.symbol_count());
                // The pinned snapshot must be the second version
                // (beta_0..beta_99) — never a torn mix with alpha
                // (1..10) or gamma (1..50).
                let hits = arc.find_by_name("beta_0");
                if let Some(sym) = hits.first() {
                    first_names.insert(sym.name().to_string());
                } else {
                    panic!("reader {reader_id}: pinned snapshot missing beta_0 — torn read?");
                }
            }
            (reader_id, symbol_counts, first_names)
        }));
    }

    let mut all_counts = HashSet::new();
    let mut all_names = HashSet::new();
    for handle in handles {
        let (reader_id, counts, names) = handle.join().expect("thread panicked");
        assert_eq!(counts.len(), 1, "reader {reader_id} saw multiple counts: {counts:?}");
        assert_eq!(names.len(), 1, "reader {reader_id} saw multiple first names: {names:?}");
        all_counts.extend(counts);
        all_names.extend(names);
    }
    // Every thread consistently observed the id2 snapshot.
    assert_eq!(all_counts.len(), 1);
    assert_eq!(all_counts.into_iter().next(), Some(100));
    assert_eq!(all_names.len(), 1);
    assert!(all_names.contains("beta_0"));
}
