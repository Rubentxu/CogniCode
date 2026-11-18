//! A store that holds a graph hands out one shared snapshot, and keeps
//! handing out that very pointer until the graph changes.
//!
//! A cache keyed by the identity of the published snapshot can only hit if
//! the store gives every reader the same `Arc`, so each store is asked the
//! same questions: same pointer between writes, a different one after a
//! write, and the same pointer for every thread that asks at once.

use std::collections::HashMap;
use std::sync::{Arc, Barrier};

use cognicode_core::application::ports::SharedGraph;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::traits::graph_store::{GraphStore, StoreError};
use cognicode_core::domain::value_objects::file_manifest::FileManifest;
use cognicode_core::domain::value_objects::{CheckpointId, Location, SymbolKind};
use cognicode_core::infrastructure::graph::GraphCache;
use cognicode_core::infrastructure::persistence::{CachedGraphStore, InMemoryGraphStore};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_search_idf;
use cognicode_core::interface::mcp::schemas::GraphSearchIdfInput;

/// A graph whose size identifies it: `n` functions named `<prefix>_<i>`.
fn graph_of(prefix: &str, n: usize) -> CallGraph {
    let mut graph = CallGraph::new();
    for i in 0..n {
        graph.add_symbol(Symbol::new(
            format!("{prefix}_{i}"),
            SymbolKind::Function,
            Location::new(format!("{prefix}.rs"), i as u32 + 1, 0),
        ));
    }
    graph
}

/// What a store must do to be asked the same questions as the others: say
/// how a new snapshot is published to it.
trait Publisher: Send + Sync {
    fn store(&self) -> &dyn GraphStore;
    fn publish(&self, graph: CallGraph);
}

struct InMemory(InMemoryGraphStore);

impl Publisher for InMemory {
    fn store(&self) -> &dyn GraphStore {
        &self.0
    }
    fn publish(&self, graph: CallGraph) {
        self.0.save_graph(&graph).unwrap();
    }
}

struct Cached(Arc<GraphCache>, CachedGraphStore);

impl Cached {
    fn new() -> Self {
        let cache = Arc::new(GraphCache::new());
        let store = CachedGraphStore::new(cache.clone());
        Self(cache, store)
    }
}

impl Publisher for Cached {
    fn store(&self) -> &dyn GraphStore {
        &self.1
    }
    fn publish(&self, graph: CallGraph) {
        self.0.replace(graph);
    }
}

fn shared(store: &dyn GraphStore) -> Arc<CallGraph> {
    store
        .load_graph_shared()
        .expect("the store answers")
        .expect("the store holds a graph")
}

fn same_pointer_until_the_graph_changes(subject: &dyn Publisher) {
    subject.publish(graph_of("a", 3));
    let first = shared(subject.store());
    for _ in 0..10 {
        assert!(
            Arc::ptr_eq(&first, &shared(subject.store())),
            "no write happened, so every reader must get the snapshot the first one got"
        );
    }
    assert_eq!(first.symbol_count(), 3);

    subject.publish(graph_of("b", 5));
    let second = shared(subject.store());
    assert!(
        !Arc::ptr_eq(&first, &second),
        "a write publishes a new snapshot"
    );
    assert_eq!(second.symbol_count(), 5);
    assert_eq!(
        first.symbol_count(),
        3,
        "the retired snapshot is not touched"
    );
    assert!(Arc::ptr_eq(&second, &shared(subject.store())));
}

fn a_write_of_equal_content_is_still_a_new_snapshot(subject: &dyn Publisher) {
    subject.publish(graph_of("a", 3));
    let first = shared(subject.store());
    subject.publish(graph_of("a", 3));
    assert!(!Arc::ptr_eq(&first, &shared(subject.store())));
}

/// Readers hammer the store while a writer publishes versions of growing
/// size. Whatever version a reader sees, every reader that sees it holds
/// one and the same pointer, and a version never comes back after a newer
/// one was seen.
fn concurrent_readers_agree_on_the_pointer_of_each_version(subject: Arc<dyn Publisher>) {
    const VERSIONS: usize = 30;
    const READERS: usize = 8;
    subject.publish(graph_of("v", 1));
    let gate = Arc::new(Barrier::new(READERS + 1));

    let readers: Vec<_> = (0..READERS)
        .map(|_| {
            let (subject, gate) = (subject.clone(), gate.clone());
            std::thread::spawn(move || {
                gate.wait();
                let mut seen: Vec<(usize, Arc<CallGraph>)> = Vec::new();
                while seen.last().map(|(n, _)| *n) != Some(VERSIONS) {
                    let graph = shared(subject.store());
                    let size = graph.symbol_count();
                    if let Some((last, _)) = seen.last() {
                        assert!(size >= *last, "a retired version came back");
                    }
                    if seen.last().map(|(n, _)| *n) != Some(size) {
                        seen.push((size, graph));
                    }
                }
                seen
            })
        })
        .collect();

    gate.wait();
    for n in 2..=VERSIONS {
        subject.publish(graph_of("v", n));
        std::thread::yield_now();
    }

    let mut pointer_of_version: HashMap<usize, Arc<CallGraph>> = HashMap::new();
    for reader in readers {
        for (size, graph) in reader.join().unwrap() {
            let first = pointer_of_version
                .entry(size)
                .or_insert_with(|| graph.clone());
            assert!(
                Arc::ptr_eq(first, &graph),
                "two readers of version {size} hold different copies of it"
            );
        }
    }
}

fn many_threads_asking_at_once_share_one_pointer(subject: Arc<dyn Publisher>) {
    subject.publish(graph_of("a", 50));
    let gate = Arc::new(Barrier::new(16));
    let handles: Vec<_> = (0..16)
        .map(|_| {
            let (subject, gate) = (subject.clone(), gate.clone());
            std::thread::spawn(move || {
                gate.wait();
                shared(subject.store())
            })
        })
        .collect();
    let all: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert!(all.iter().all(|g| Arc::ptr_eq(g, &all[0])));
    subject.publish(graph_of("b", 51));
    let after = shared(subject.store());
    assert!(all.iter().all(|g| !Arc::ptr_eq(g, &after)));
}

#[test]
fn in_memory_store_hands_out_the_same_pointer_until_the_graph_changes() {
    same_pointer_until_the_graph_changes(&InMemory(InMemoryGraphStore::new()));
    a_write_of_equal_content_is_still_a_new_snapshot(&InMemory(InMemoryGraphStore::new()));
}

#[test]
fn cached_store_hands_out_the_same_pointer_until_the_graph_changes() {
    same_pointer_until_the_graph_changes(&Cached::new());
    a_write_of_equal_content_is_still_a_new_snapshot(&Cached::new());
}

#[test]
fn in_memory_store_shares_one_pointer_between_concurrent_readers() {
    many_threads_asking_at_once_share_one_pointer(Arc::new(InMemory(InMemoryGraphStore::new())));
    concurrent_readers_agree_on_the_pointer_of_each_version(Arc::new(InMemory(
        InMemoryGraphStore::new(),
    )));
}

#[test]
fn cached_store_shares_one_pointer_between_concurrent_readers() {
    many_threads_asking_at_once_share_one_pointer(Arc::new(Cached::new()));
    concurrent_readers_agree_on_the_pointer_of_each_version(Arc::new(Cached::new()));
}

#[test]
fn in_memory_store_hands_out_nothing_when_cleared_and_a_new_pointer_after() {
    let store = InMemoryGraphStore::new();
    store.save_graph(&graph_of("a", 2)).unwrap();
    let before = shared(&store);
    store.clear().unwrap();
    assert!(store.load_graph_shared().unwrap().is_none());
    store.save_graph(&graph_of("a", 2)).unwrap();
    assert!(!Arc::ptr_eq(&before, &shared(&store)));
}

#[test]
fn the_owned_and_the_shared_graph_of_a_store_are_the_same_graph() {
    let store = InMemoryGraphStore::new();
    store.save_graph(&graph_of("a", 4)).unwrap();
    let owned = store.load_graph().unwrap().unwrap();
    assert_eq!(owned.symbol_count(), shared(&store).symbol_count());
}

// ---------------------------------------------------------------------------
// The checkpoint id
// ---------------------------------------------------------------------------

/// A store that implements nothing but what the trait requires.
struct Bare;

impl GraphStore for Bare {
    fn save_graph(&self, _: &CallGraph) -> Result<(), StoreError> {
        Ok(())
    }
    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        Ok(Some(graph_of("bare", 1)))
    }
    fn save_manifest(&self, _: &FileManifest) -> Result<(), StoreError> {
        Ok(())
    }
    fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError> {
        Ok(None)
    }
    fn clear(&self) -> Result<(), StoreError> {
        Ok(())
    }
    fn exists(&self) -> Result<bool, StoreError> {
        Ok(true)
    }
}

#[test]
fn a_store_that_does_not_identify_its_snapshots_says_so_instead_of_panicking() {
    assert_eq!(Bare.current_checkpoint_id(), None);
}

#[test]
fn in_memory_checkpoint_id_changes_with_every_write_and_never_comes_back() {
    let store = InMemoryGraphStore::new();
    assert_eq!(store.current_checkpoint_id(), None, "a cold store has none");

    let mut seen: Vec<CheckpointId> = Vec::new();
    for round in 0..3 {
        store.save_graph(&graph_of("a", round + 1)).unwrap();
        seen.push(store.current_checkpoint_id().expect("a graph is held"));
        store.clear().unwrap();
        assert_eq!(store.current_checkpoint_id(), None);
        store.save_graph(&graph_of("b", round + 1)).unwrap();
        seen.push(store.current_checkpoint_id().expect("a graph is held"));
    }
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        seen.len(),
        "an id was handed out twice: {seen:?}"
    );
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "ids grow: {seen:?}");
}

#[test]
fn in_memory_checkpoint_id_ignores_what_is_not_the_graph() {
    let store = InMemoryGraphStore::new();
    store
        .save_manifest(&FileManifest::new(std::path::PathBuf::from("/p")))
        .unwrap();
    assert_eq!(store.current_checkpoint_id(), None, "no graph, no snapshot");
    store.save_graph(&graph_of("a", 1)).unwrap();
    let id = store.current_checkpoint_id().unwrap();
    store
        .save_manifest(&FileManifest::new(std::path::PathBuf::from("/q")))
        .unwrap();
    assert_eq!(
        store.current_checkpoint_id(),
        Some(id),
        "same graph, same id"
    );
}

#[test]
fn in_memory_checkpoint_at_hands_out_the_snapshot_of_the_current_id_only() {
    let store = InMemoryGraphStore::new();
    store.save_graph(&graph_of("a", 2)).unwrap();
    let old = store.current_checkpoint_id().unwrap();
    store.save_graph(&graph_of("a", 3)).unwrap();
    let current = store.current_checkpoint_id().unwrap();

    let pinned = store.checkpoint_at(current).unwrap().unwrap();
    assert!(Arc::ptr_eq(&pinned, &shared(&store)));
    assert!(matches!(
        store.checkpoint_at(old),
        Err(StoreError::CheckpointNotFound(id)) if id == old
    ));
}

#[test]
fn the_cached_store_checkpoint_id_changes_with_every_publication() {
    let subject = Cached::new();
    assert_eq!(subject.store().current_checkpoint_id(), None);
    let mut seen = Vec::new();
    for n in 1..=5 {
        subject.publish(graph_of("a", n));
        seen.push(subject.store().current_checkpoint_id().unwrap());
    }
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "ids grow: {seen:?}");
}

// ---------------------------------------------------------------------------
// What a per-snapshot cache sees
// ---------------------------------------------------------------------------

async fn ask(ctx: &HandlerContext, query: &str) -> serde_json::Value {
    handle_graph_search_idf(
        ctx,
        GraphSearchIdfInput {
            query: query.to_string(),
            max_results: 5,
        },
    )
    .await
    .expect("the search tool answers")
}

#[tokio::test]
async fn the_search_tool_builds_the_statistics_once_per_snapshot_on_the_in_memory_store() {
    let store = Arc::new(InMemoryGraphStore::new());
    store.save_graph(&graph_of("alpha", 20)).unwrap();
    let ctx = HandlerContext::builder()
        .with_graph_store_arc(store.clone())
        .build();

    let first = ask(&ctx, "alpha_3").await;
    for _ in 0..5 {
        assert_eq!(ask(&ctx, "alpha_3").await, first);
    }
    let counters = ctx.ranking_cache_counters();
    assert_eq!(counters.builds, 1, "six calls over one snapshot, one build");
    assert_eq!(counters.hits, 5);

    store.save_graph(&graph_of("beta", 20)).unwrap();
    let after = ask(&ctx, "beta_3").await;
    assert_eq!(after["results"][0]["name"], "beta_3");
    let _ = ask(&ctx, "beta_4").await;
    let counters = ctx.ranking_cache_counters();
    assert_eq!(
        counters.builds, 2,
        "a write is one new build, not one per call"
    );
    assert_eq!(counters.hits, 6);
}
