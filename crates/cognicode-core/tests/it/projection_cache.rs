//! The projection of a snapshot is built once and shared by every reader.
//!
//! The cache is keyed by the identity of the published `Arc<CallGraph>`: a
//! second call on the same snapshot gets the projection (and the adjacency it
//! carries) the first one built, a call on another snapshot gets its own, and
//! a projection a reader holds never changes under it.

use std::sync::{Arc, Barrier};
use std::thread;

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::ports::{CallGraphProjectionPort, EdgeSide};
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::graph::project_call_graph;
use cognicode_core::infrastructure::graph::projection_cache::ProjectionCache;
use cognicode_core::infrastructure::graph::snapshot_cache::SnapshotCacheLimits;

/// A chain `f0 -> f1 -> ... -> f{n-1}` closed into a cycle.
fn ring(nodes: usize) -> CallGraph {
    let mut graph = CallGraph::new();
    let ids: Vec<_> = (0..nodes)
        .map(|n| {
            graph.add_symbol(Symbol::new(
                format!("f{n}"),
                SymbolKind::Function,
                Location::new(format!("file{n}.rs"), 1, 1),
            ))
        })
        .collect();
    for n in 0..nodes {
        graph
            .add_dependency(&ids[n], &ids[(n + 1) % nodes], DependencyType::Calls)
            .expect("both endpoints exist");
    }
    graph
}

fn same_projection(
    a: &Arc<dyn CallGraphProjectionPort>,
    b: &Arc<dyn CallGraphProjectionPort>,
) -> bool {
    std::ptr::eq(Arc::as_ptr(a).cast::<()>(), Arc::as_ptr(b).cast::<()>())
}

#[test]
fn every_call_on_a_snapshot_shares_one_projection_and_one_adjacency() {
    let cache = ProjectionCache::default();
    let graph = Arc::new(ring(40));

    let first = cache.projection_for(&graph);
    let second = cache.projection_for(&graph);

    assert!(same_projection(&first, &second));
    assert!(std::ptr::eq(
        first.compact_adjacency(),
        second.compact_adjacency()
    ));
    let counters = cache.counters();
    assert_eq!((counters.builds, counters.hits), (1, 1));
    assert_eq!(counters.entries, 1);
}

#[test]
fn the_shared_projection_answers_what_a_fresh_one_answers() {
    let cache = ProjectionCache::default();
    let graph = Arc::new(ring(40));
    let shared = cache.projection_for(&graph);
    let fresh = project_call_graph(&graph);

    assert_eq!(
        shared.compact_adjacency().out_lists(),
        fresh.compact_adjacency().out_lists()
    );
    assert_eq!(
        shared.compact_adjacency().in_lists(),
        fresh.compact_adjacency().in_lists()
    );
    assert_eq!(
        shared
            .compact_adjacency()
            .reach(0, EdgeSide::Incoming, usize::MAX, true),
        fresh
            .compact_adjacency()
            .reach(0, EdgeSide::Incoming, usize::MAX, true)
    );
    assert_eq!(shared.build_adjacency(), fresh.build_adjacency());
}

#[test]
fn a_changed_graph_is_another_snapshot_with_its_own_projection() {
    let cache = ProjectionCache::default();
    let published = Arc::new(ring(40));
    let held = cache.projection_for(&published);
    let held_edges = held.compact_adjacency().edge_count();

    // A change publishes a new graph; the one a reader holds is not edited.
    let mut changed = ring(40);
    let ids: Vec<_> = changed.symbol_ids().map(|(id, _)| id.clone()).collect();
    changed
        .add_dependency(&ids[0], &ids[7], DependencyType::Imports)
        .expect("both endpoints exist");
    let changed = Arc::new(changed);
    let next = cache.projection_for(&changed);

    assert!(!same_projection(&held, &next));
    assert_eq!(held.compact_adjacency().edge_count(), held_edges);
    assert_eq!(next.compact_adjacency().edge_count(), held_edges + 1);
    // The first snapshot still answers from its own projection.
    assert!(same_projection(&held, &cache.projection_for(&published)));
}

#[test]
fn an_equal_graph_in_another_allocation_is_another_snapshot() {
    let cache = ProjectionCache::default();
    let a = Arc::new(ring(10));
    let b = Arc::new(ring(10));
    assert!(!same_projection(
        &cache.projection_for(&a),
        &cache.projection_for(&b)
    ));
    assert_eq!(cache.counters().builds, 2);
}

#[test]
fn a_retired_snapshot_leaves_the_cache_and_its_projection_goes_with_it() {
    let cache = ProjectionCache::default();
    let retired = Arc::new(ring(10));
    drop(cache.projection_for(&retired));
    assert_eq!(cache.counters().entries, 1);
    drop(retired);

    let live = Arc::new(ring(10));
    drop(cache.projection_for(&live));
    let counters = cache.counters();
    assert_eq!(counters.entries, 1, "only the live snapshot is held");
    assert_eq!(counters.evictions, 1);
}

#[test]
fn the_cache_keeps_two_snapshots_and_evicts_the_least_recently_used() {
    let cache = ProjectionCache::default();
    let graphs: Vec<Arc<CallGraph>> = (3..6).map(|n| Arc::new(ring(n))).collect();
    for graph in &graphs {
        drop(cache.projection_for(graph));
    }
    let counters = cache.counters();
    assert_eq!(counters.entries, 2);
    assert_eq!(counters.evictions, 1);

    // The oldest was dropped: asking again rebuilds it.
    drop(cache.projection_for(&graphs[0]));
    assert_eq!(cache.counters().builds, 4);
}

#[test]
fn a_snapshot_over_the_symbol_budget_is_projected_per_call_and_not_retained() {
    let cache = ProjectionCache::new(SnapshotCacheLimits {
        max_entries: 2,
        max_symbols: 5,
    });
    let graph = Arc::new(ring(8));
    let first = cache.projection_for(&graph);
    let second = cache.projection_for(&graph);

    assert!(!same_projection(&first, &second));
    let counters = cache.counters();
    assert_eq!(
        (counters.bypasses, counters.builds, counters.entries),
        (2, 2, 0)
    );
    // Same answers as the retained kind.
    assert_eq!(first.compact_adjacency(), second.compact_adjacency());
}

#[test]
fn a_keyed_snapshot_cannot_be_edited_in_place() {
    let cache = ProjectionCache::default();
    let mut graph = Arc::new(ring(10));
    drop(cache.projection_for(&graph));
    assert!(
        Arc::get_mut(&mut graph).is_none(),
        "the cache's weak pointer must forbid in-place edits"
    );
}

#[test]
fn concurrent_first_calls_on_one_snapshot_build_it_once() {
    let cache = Arc::new(ProjectionCache::default());
    let graph = Arc::new(ring(2_000));
    let start = Arc::new(Barrier::new(8));

    let projections: Vec<_> = (0..8)
        .map(|_| {
            let (cache, graph, start) =
                (Arc::clone(&cache), Arc::clone(&graph), Arc::clone(&start));
            thread::spawn(move || {
                start.wait();
                cache.projection_for(&graph)
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|handle| handle.join().expect("reader finished"))
        .collect();

    assert_eq!(cache.counters().builds, 1);
    assert!(
        projections
            .windows(2)
            .all(|pair| same_projection(&pair[0], &pair[1]))
    );
}

#[test]
fn readers_of_a_shared_projection_do_not_change_it_or_each_other() {
    let cache = Arc::new(ProjectionCache::default());
    let graph = Arc::new(ring(500));
    let projection = cache.projection_for(&graph);
    let before = projection.compact_adjacency().clone();

    let answers: Vec<_> = (0..8usize)
        .map(|reader| {
            let projection = Arc::clone(&projection);
            thread::spawn(move || {
                // A reader that gives up early, and one that panics, leave
                // the adjacency as it was.
                if reader == 3 {
                    let _ = projection
                        .compact_adjacency()
                        .reach(0, EdgeSide::Outgoing, 2, false);
                    return None;
                }
                if reader == 5 {
                    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        projection
                            .compact_adjacency()
                            .reach(10_000, EdgeSide::Outgoing, 2, false)
                    }));
                    assert!(panicked.is_err());
                    return None;
                }
                Some(
                    projection
                        .compact_adjacency()
                        .reach(0, EdgeSide::Outgoing, usize::MAX, false),
                )
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|handle| handle.join().expect("reader finished"))
        .collect();

    let full: Vec<_> = answers.into_iter().flatten().collect();
    assert_eq!(full.len(), 6);
    assert!(full.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(full[0].len(), 499);
    assert_eq!(*projection.compact_adjacency(), before);
}
