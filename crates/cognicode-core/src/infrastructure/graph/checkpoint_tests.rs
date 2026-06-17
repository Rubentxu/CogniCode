//! Unit tests for `CheckpointId` and `VersionedGraphCache` (ADR-035 PR-1).

use crate::domain::aggregates::call_graph::CallGraph;
use crate::infrastructure::graph::checkpoint::{CheckpointId, VersionedGraphCache};
use std::sync::Arc;

#[test]
fn test_checkpoint_id_monotonic() {
    // CheckpointId::next() is pure arithmetic and strictly increasing.
    let a = CheckpointId(1);
    let b = a.next();
    let c = b.next();
    assert!(b > a);
    assert!(c > b);
    assert_eq!(b, CheckpointId(2));
    assert_eq!(c, CheckpointId(3));
}

#[test]
fn test_checkpoint_id_none_is_invalid() {
    // id 0 is the reserved "no checkpoint" sentinel.
    assert!(!CheckpointId::NONE.is_valid());
    assert!(CheckpointId(1).is_valid());
    assert!(CheckpointId(u64::MAX).is_valid());
}

#[test]
fn test_checkpoint_id_display() {
    // Display is stable and machine-parseable.
    assert_eq!(format!("{}", CheckpointId(0)), "checkpoint:0");
    assert_eq!(format!("{}", CheckpointId(42)), "checkpoint:42");
}

#[test]
fn test_versioned_cache_new_is_empty() {
    let cache = VersionedGraphCache::new(2);
    assert!(cache.head().is_none(), "fresh cache should have no head");
    assert_eq!(cache.head_id(), CheckpointId::NONE);
    assert!(!cache.contains(CheckpointId(1)));
}

#[test]
fn test_versioned_cache_insert_returns_monotonic_ids() {
    let mut cache = VersionedGraphCache::new(3);
    let id1 = cache.insert(Arc::new(CallGraph::new()));
    let id2 = cache.insert(Arc::new(CallGraph::new()));
    assert_eq!(id1, CheckpointId(1));
    assert_eq!(id2, CheckpointId(2));
    assert!(id2 > id1);
}

#[test]
fn test_versioned_cache_head_is_latest() {
    let mut cache = VersionedGraphCache::new(3);
    let id1 = cache.insert(Arc::new(CallGraph::new()));
    let id2 = cache.insert(Arc::new(CallGraph::new()));
    let id3 = cache.insert(Arc::new(CallGraph::new()));
    assert_eq!(cache.head_id(), id3);
    assert_ne!(cache.head_id(), id1);
    assert_ne!(cache.head_id(), id2);
    assert!(cache.head().is_some());
}

#[test]
fn test_versioned_cache_get_at_returns_pinned() {
    let mut cache = VersionedGraphCache::new(3);
    // id 1 is a graph with one symbol
    let mut g1 = CallGraph::new();
    g1.add_symbol(crate::domain::aggregates::symbol::Symbol::new(
        "first",
        crate::domain::value_objects::SymbolKind::Function,
        crate::domain::value_objects::Location::new("first.rs", 1, 1),
    ));
    cache.insert(Arc::new(g1));
    // id 2 is an empty graph
    cache.insert(Arc::new(CallGraph::new()));

    // Pin to id 1 — the older graph with the symbol must survive.
    let pinned = cache.get_at(CheckpointId(1)).expect("id 1 should still be in the ring");
    assert_eq!(pinned.symbol_count(), 1);

    // And the head is the empty id 2.
    let head = cache.head().expect("head should be present");
    assert_eq!(head.symbol_count(), 0);
}

#[test]
fn test_versioned_cache_retention_evicts_oldest() {
    let mut cache = VersionedGraphCache::new(2);
    cache.insert(Arc::new(CallGraph::new()));
    cache.insert(Arc::new(CallGraph::new()));
    cache.insert(Arc::new(CallGraph::new()));
    // After 3 inserts with retention=2, id 1 must be evicted.
    assert!(!cache.contains(CheckpointId(1)));
    assert!(cache.contains(CheckpointId(2)));
    assert!(cache.contains(CheckpointId(3)));
}

#[test]
fn test_versioned_cache_get_at_evicted_returns_none() {
    let mut cache = VersionedGraphCache::new(2);
    cache.insert(Arc::new(CallGraph::new()));
    cache.insert(Arc::new(CallGraph::new()));
    cache.insert(Arc::new(CallGraph::new()));
    // id 1 is gone — get_at must return None.
    assert!(cache.get_at(CheckpointId(1)).is_none());
    // ids 2 and 3 are still pinned.
    assert!(cache.get_at(CheckpointId(2)).is_some());
    assert!(cache.get_at(CheckpointId(3)).is_some());
}

#[test]
fn test_versioned_cache_contains() {
    let mut cache = VersionedGraphCache::new(2);
    cache.insert(Arc::new(CallGraph::new()));
    cache.insert(Arc::new(CallGraph::new()));
    assert!(cache.contains(CheckpointId(1)));
    assert!(cache.contains(CheckpointId(2)));
    assert!(!cache.contains(CheckpointId(99)));
    assert!(!cache.contains(CheckpointId::NONE));
}

#[test]
#[should_panic(expected = "retention must be at least 1")]
fn test_versioned_cache_zero_retention_panics() {
    // Documented invariant: retention < 1 is a programming error.
    let _ = VersionedGraphCache::new(0);
}
