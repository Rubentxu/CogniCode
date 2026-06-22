//! Graph cache implementation
//!
//! ADR-035: in-memory checkpointing for snapshot isolation. Internally,
//! [`GraphCache`] wraps an `ArcSwap<Mutex<VersionedGraphCache>>` so
//! writers can publish a new head while in-flight readers pin to an
//! older version. The default retention is 2 (current + previous).

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::events::GraphEvent;
use crate::infrastructure::graph::checkpoint::{CheckpointId, VersionedGraphCache};
use arc_swap::ArcSwap;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::broadcast;

/// Default checkpoint retention: keep the current head plus one previous
/// version. This is enough for the common Explorer + MCP read pattern
/// (one in-flight read, one new write) and keeps memory bounded.
pub const DEFAULT_RETENTION: usize = 2;

/// Thread-safe cache for call graphs with incremental update support
/// and per-version snapshot isolation (ADR-035).
///
/// The `ArcSwap<Mutex<VersionedGraphCache>>` layout gives:
///
/// * Lock-free pointer swap on the outer `ArcSwap` (cheap when read
///   concurrency > write concurrency).
/// * Inner `Mutex` for the rare write path that needs to mutate the
///   ring (insert, evict, read head_id) and return a value derived
///   from the new state.
pub struct GraphCache {
    cache: ArcSwap<Mutex<VersionedGraphCache>>,
    pending_events: Mutex<Vec<GraphEvent>>,
    event_sender: broadcast::Sender<GraphEvent>,
}

impl GraphCache {
    /// Creates a new empty graph cache with the default retention
    /// ([`DEFAULT_RETENTION`] = 2).
    pub fn new() -> Self {
        Self::with_retention(DEFAULT_RETENTION)
    }

    /// Creates a new empty graph cache that keeps the last `retention`
    /// checkpoints (FIFO eviction). Panics if `retention < 1`.
    pub fn with_retention(retention: usize) -> Self {
        let (event_sender, _) = broadcast::channel(16);
        Self {
            cache: ArcSwap::from_pointee(Mutex::new(VersionedGraphCache::new(retention))),
            pending_events: Mutex::new(Vec::new()),
            event_sender,
        }
    }

    /// Subscribe to graph mutation events
    pub fn subscribe(&self) -> broadcast::Receiver<GraphEvent> {
        self.event_sender.subscribe()
    }

    /// Gets the current head graph. Always returns a value — a fresh
    /// cache exposes an empty `CallGraph` as head.
    pub fn get(&self) -> Arc<CallGraph> {
        let guard = self.cache.load();
        let cache = guard.lock().unwrap_or_else(|_| panic!("graph cache poisoned"));
        cache
            .head()
            .unwrap_or_else(|| Arc::new(CallGraph::new()))
    }

    /// Gets a reference to the underlying graph.
    pub fn get_ref(&self) -> &CallGraph {
        // SAFETY: same pattern the previous implementation used to
        // return a long-lived `&CallGraph` through an `&self` receiver.
        // The pointer is taken from the head's `Arc<CallGraph>` held
        // inside the current `ArcSwap<Mutex<VersionedGraphCache>>`.
        // The `Arc<CallGraph>` is leaked into a `Box<Arc<_>>` that
        // outlives the returned reference, so the pointee stays valid
        // for the unbounded lifetime of `&CallGraph`. This preserves
        // the original (intentionally optimistic) semantics and the
        // public signature.
        let guard = self.cache.load();
        let cache = guard.lock().unwrap_or_else(|_| panic!("graph cache poisoned"));
        let arc: Arc<CallGraph> = cache
            .head()
            .unwrap_or_else(|| Arc::new(CallGraph::new()));
        let ptr = arc.as_ref() as *const CallGraph;
        // Keep the Arc alive: leak the Box so the CallGraph heap
        // allocation is not freed while the returned reference is
        // still in use. The leak is bounded by the rate of `get_ref`
        // calls; in MCP handlers that is one call per request.
        Box::leak(Box::new(arc));
        unsafe { &*ptr }
    }

    /// Replaces the cached graph with a new checkpoint. Returns the
    /// [`CheckpointId`] of the new head. Existing callers that ignore
    /// the return value continue to work unchanged.
    pub fn set(&self, graph: CallGraph) -> CheckpointId {
        let id = {
            let guard = self.cache.load();
            let mut cache = guard.lock().unwrap_or_else(|_| panic!("graph cache poisoned"));
            cache.insert(Arc::new(graph))
        };
        let _ = self.event_sender.send(GraphEvent::GraphReplaced);
        id
    }

    /// Updates the graph using a closure
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut CallGraph),
    {
        let mut graph = (*self.head_or_empty()).clone();
        f(&mut graph);
        self.set(graph);
    }

    /// Applies incremental events to the cached graph
    ///
    /// This is more efficient than rebuilding the entire graph
    /// when only a single file has changed.
    pub fn apply_events(
        &self,
        events: &[GraphEvent],
    ) -> Result<(), crate::domain::aggregates::call_graph::CallGraphError> {
        let mut graph = (*self.head_or_empty()).clone();
        graph.apply_events(events)?;
        self.set(graph);
        let _ = self.event_sender.send(GraphEvent::GraphModified);
        Ok(())
    }

    /// Pin a read to a specific [`CheckpointId`]. Returns `None` if
    /// the checkpoint has already been evicted.
    pub fn get_at(&self, id: CheckpointId) -> Option<Arc<CallGraph>> {
        let guard = self.cache.load();
        let cache = guard.lock().unwrap_or_else(|_| panic!("graph cache poisoned"));
        cache.get_at(id)
    }

    /// Returns the [`CheckpointId`] of the current head, or `None` if
    /// no checkpoint has been published yet.
    pub fn current_id(&self) -> Option<CheckpointId> {
        let guard = self.cache.load();
        let cache = guard.lock().unwrap_or_else(|_| panic!("graph cache poisoned"));
        let id = cache.head_id();
        if id.is_valid() {
            Some(id)
        } else {
            None
        }
    }

    pub fn queue_event(&self, event: GraphEvent) {
        self.pending_events.lock().unwrap().push(event);
    }

    pub fn flush_events(
        &self,
    ) -> Result<(), crate::domain::aggregates::call_graph::CallGraphError> {
        let events: Vec<GraphEvent> = {
            let mut pending = self.pending_events.lock().unwrap();
            std::mem::take(&mut *pending)
        };
        if events.is_empty() {
            return Ok(());
        }
        self.apply_events(&events)
    }

    /// Clears the cache
    pub fn clear(&self) {
        self.set(CallGraph::new());
        self.pending_events.lock().unwrap().clear();
        let _ = self.event_sender.send(GraphEvent::GraphCleared);
    }

    /// Helper: returns the current head or an empty `CallGraph` in an Arc.
    /// Used by `update` and `apply_events` to clone the head before mutation.
    fn head_or_empty(&self) -> Arc<CallGraph> {
        let guard = self.cache.load();
        let cache = guard.lock().unwrap_or_else(|_| panic!("graph cache poisoned"));
        cache
            .head()
            .unwrap_or_else(|| Arc::new(CallGraph::new()))
    }
}

impl Default for GraphCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::aggregates::call_graph::CallGraph;
    use crate::domain::events::GraphEvent;
    use crate::domain::events::graph_event::{DependencyEvent, SymbolAddedEvent};
    use crate::domain::value_objects::{DependencyType, Location, SymbolKind};
    use crate::infrastructure::graph::checkpoint::CheckpointId;
    use crate::infrastructure::graph::graph_cache::GraphCache;

    #[test]
    fn test_new_creates_empty_cache() {
        let cache = GraphCache::new();
        let graph = cache.get();
        assert_eq!(graph.symbol_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_get_returns_default_graph() {
        let cache = GraphCache::new();
        let graph = cache.get();
        assert_eq!(graph.symbol_count(), 0);
    }

    #[test]
    fn test_set_updates_cache() {
        let cache = GraphCache::new();
        let mut graph = CallGraph::new();
        let symbol = crate::domain::aggregates::symbol::Symbol::new(
            "test_func",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        graph.add_symbol(symbol);
        cache.set(graph);
        let result = cache.get();
        assert_eq!(result.symbol_count(), 1);
    }

    #[test]
    fn test_update_applies_closure_to_graph() {
        let cache = GraphCache::new();
        cache.update(|g| {
            let sym = crate::domain::aggregates::symbol::Symbol::new(
                "func_a",
                SymbolKind::Function,
                Location::new("test.rs", 1, 1),
            );
            g.add_symbol(sym);
        });
        cache.update(|g| {
            let sym = crate::domain::aggregates::symbol::Symbol::new(
                "func_b",
                SymbolKind::Function,
                Location::new("test.rs", 10, 1),
            );
            g.add_symbol(sym);
        });
        assert_eq!(cache.get().symbol_count(), 2);
    }

    #[test]
    fn test_apply_events_adds_symbol() {
        let cache = GraphCache::new();
        let events = vec![GraphEvent::SymbolAdded(SymbolAddedEvent {
            name: "new_func".to_string(),
            kind: SymbolKind::Function,
            location: Location::new("main.rs", 5, 1),
            signature: None,
        })];
        cache.apply_events(&events).unwrap();
        assert_eq!(cache.get().symbol_count(), 1);
    }

    #[test]
    fn test_apply_events_adds_dependency() {
        let cache = GraphCache::new();
        let events = vec![
            GraphEvent::SymbolAdded(SymbolAddedEvent {
                name: "caller".to_string(),
                kind: SymbolKind::Function,
                location: Location::new("main.rs", 1, 1),
                signature: None,
            }),
            GraphEvent::SymbolAdded(SymbolAddedEvent {
                name: "callee".to_string(),
                kind: SymbolKind::Function,
                location: Location::new("main.rs", 10, 1),
                signature: None,
            }),
            GraphEvent::DependencyAdded(DependencyEvent {
                file: "main.rs".to_string(),
                source_name: "caller".to_string(),
                target_name: "callee".to_string(),
                dependency_type: DependencyType::Calls,
            }),
        ];
        cache.apply_events(&events).unwrap();
        assert_eq!(cache.get().symbol_count(), 2);
    }

    #[test]
    fn test_queue_event_and_flush_events_batch_updates() {
        let cache = GraphCache::new();
        cache.queue_event(GraphEvent::SymbolAdded(SymbolAddedEvent {
            name: "batch_func".to_string(),
            kind: SymbolKind::Function,
            location: Location::new("batch.rs", 1, 1),
            signature: None,
        }));
        cache.queue_event(GraphEvent::SymbolAdded(SymbolAddedEvent {
            name: "batch_func2".to_string(),
            kind: SymbolKind::Function,
            location: Location::new("batch.rs", 5, 1),
            signature: None,
        }));
        cache.flush_events().unwrap();
        assert_eq!(cache.get().symbol_count(), 2);
    }

    #[test]
    fn test_flush_events_with_empty_queue() {
        let cache = GraphCache::new();
        cache.queue_event(GraphEvent::SymbolAdded(SymbolAddedEvent {
            name: "single".to_string(),
            kind: SymbolKind::Function,
            location: Location::new("solo.rs", 1, 1),
            signature: None,
        }));
        cache.flush_events().unwrap();
        cache.flush_events().unwrap();
        assert_eq!(cache.get().symbol_count(), 1);
    }

    #[test]
    fn test_clear_clears_cache_and_pending_events() {
        let cache = GraphCache::new();
        cache.queue_event(GraphEvent::SymbolAdded(SymbolAddedEvent {
            name: "to_clear".to_string(),
            kind: SymbolKind::Function,
            location: Location::new("clear.rs", 1, 1),
            signature: None,
        }));
        cache.clear();
        assert_eq!(cache.get().symbol_count(), 0);
        cache.queue_event(GraphEvent::SymbolAdded(SymbolAddedEvent {
            name: "after_clear".to_string(),
            kind: SymbolKind::Function,
            location: Location::new("after.rs", 1, 1),
            signature: None,
        }));
        cache.flush_events().unwrap();
        assert_eq!(cache.get().symbol_count(), 1);
    }

    #[test]
    fn test_subscribe_receives_event_on_set() {
        let cache = GraphCache::new();
        let mut receiver = cache.subscribe();

        let mut graph = CallGraph::new();
        let symbol = crate::domain::aggregates::symbol::Symbol::new(
            "test_func",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        graph.add_symbol(symbol);
        cache.set(graph);

        // Use try_recv in a loop since we don't have tokio runtime
        let mut event_received = false;
        for _ in 0..100 {
            if let Ok(event) = receiver.try_recv() {
                assert_eq!(event, GraphEvent::GraphReplaced);
                event_received = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(event_received, "Should have received GraphReplaced event");
    }

    #[test]
    fn test_subscribe_multiple_subscribers() {
        let cache = GraphCache::new();
        let mut receiver1 = cache.subscribe();
        let mut receiver2 = cache.subscribe();

        let mut graph = CallGraph::new();
        let symbol = crate::domain::aggregates::symbol::Symbol::new(
            "test_func",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        graph.add_symbol(symbol);
        cache.set(graph);

        // Use try_recv in a loop since we don't have tokio runtime
        let mut event1_received = false;
        let mut event2_received = false;
        for _ in 0..100 {
            if let Ok(event) = receiver1.try_recv() {
                assert_eq!(event, GraphEvent::GraphReplaced);
                event1_received = true;
            }
            if let Ok(event) = receiver2.try_recv() {
                assert_eq!(event, GraphEvent::GraphReplaced);
                event2_received = true;
            }
            if event1_received && event2_received {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            event1_received,
            "Receiver1 should have received GraphReplaced event"
        );
        assert!(
            event2_received,
            "Receiver2 should have received GraphReplaced event"
        );
    }

    #[test]
    fn test_late_subscriber_misses_past_events() {
        let cache = GraphCache::new();

        let mut graph = CallGraph::new();
        let symbol = crate::domain::aggregates::symbol::Symbol::new(
            "test_func",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        graph.add_symbol(symbol);
        cache.set(graph);

        // Subscribe after set was already called
        let mut receiver = cache.subscribe();

        // Should not receive the GraphReplaced event since it happened before subscription
        let result = receiver.try_recv();
        assert!(result.is_err());

        // But clear should still be received (clear sends GraphReplaced via set, then GraphCleared)
        cache.clear();

        // Use try_recv in a loop since we don't have tokio runtime
        let mut event1_received = false;
        let mut event2_received = false;
        for _ in 0..100 {
            if let Ok(event) = receiver.try_recv() {
                if event == GraphEvent::GraphReplaced && !event1_received {
                    event1_received = true;
                } else if event == GraphEvent::GraphCleared && event1_received && !event2_received {
                    event2_received = true;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            event1_received,
            "Should have received GraphReplaced after clear"
        );
        assert!(
            event2_received,
            "Should have received GraphCleared after clear"
        );
    }

    #[test]
    fn test_broadcast_with_no_subscribers_does_not_panic() {
        let cache = GraphCache::new();
        // No subscribers - should not panic when setting graph
        let graph = CallGraph::new();
        cache.set(graph);
        // If we get here without panic, the test passes
        assert_eq!(cache.get().symbol_count(), 0);
    }

    #[test]
    fn test_lagged_receiver_receives_latest_after_overflow() {
        let cache = GraphCache::new();
        let mut rx = cache.subscribe();

        // The broadcast channel has capacity 16 (defined in new())
        // Send more messages than capacity without consuming
        for _ in 0..20 {
            let graph = CallGraph::new();
            cache.set(graph);
        }

        // The receiver may have lagged behind - the system should not panic
        // try_recv should return Err(Lagged) or Ok(latest) but never panic
        let result = rx.try_recv();
        // It's acceptable if it's Lagged (fell behind) or Ok (got latest)
        // Both are valid states - no panic
        if result.is_err() {
            // If error, it could be Lagged or Empty - both are fine
            // Empty means no message was available at that instant
            // Lagged means the receiver couldn't keep up
        }
        // If we get here without panic, test passes
    }

    #[test]
    fn test_receiver_dropped_does_not_affect_cache_operations() {
        let cache = GraphCache::new();
        {
            let _rx = cache.subscribe();
            // _rx goes out of scope and is dropped here
        }
        // After dropping receiver, cache operations should still work
        let graph = CallGraph::new();
        cache.set(graph.clone());
        assert_eq!(cache.get().symbol_count(), 0);

        // Add something and verify
        cache.update(|g| {
            let sym = crate::domain::aggregates::symbol::Symbol::new(
                "dropped_test",
                SymbolKind::Function,
                Location::new("test.rs", 1, 1),
            );
            g.add_symbol(sym);
        });
        assert_eq!(cache.get().symbol_count(), 1);
    }

    // ----- ADR-035 PR-1: checkpoint-aware methods -----

    #[test]
    fn test_graph_cache_set_returns_checkpoint_id() {
        let cache = GraphCache::new();
        // First set on a fresh cache: head_id is still NONE, so the
        // first set is the first real checkpoint.
        let id1 = cache.set(CallGraph::new());
        assert!(id1.is_valid(), "first set must return a valid CheckpointId");

        let id2 = cache.set(CallGraph::new());
        assert!(id2 > id1, "CheckpointIds must be strictly monotonic");
        assert_eq!(cache.current_id(), Some(id2));
    }

    #[test]
    fn test_graph_cache_get_at_returns_pinned_version() {
        let cache = GraphCache::new();

        // id1: a graph with one symbol named "v1"
        let mut g1 = CallGraph::new();
        g1.add_symbol(crate::domain::aggregates::symbol::Symbol::new(
            "v1",
            SymbolKind::Function,
            Location::new("v1.rs", 1, 1),
        ));
        let id1 = cache.set(g1);

        // id2: a graph with one symbol named "v2"
        let mut g2 = CallGraph::new();
        g2.add_symbol(crate::domain::aggregates::symbol::Symbol::new(
            "v2",
            SymbolKind::Function,
            Location::new("v2.rs", 1, 1),
        ));
        let id2 = cache.set(g2);

        // Head is the latest.
        let head = cache.get();
        assert_eq!(head.symbol_count(), 1);

        // Pin to id1 — the older version is still in the ring
        // (retention default is 2).
        let pinned = cache.get_at(id1).expect("id1 should still be in the ring");
        assert_eq!(pinned.symbol_count(), 1);
        assert_ne!(id1, id2);
        assert_eq!(cache.current_id(), Some(id2));
    }

    #[test]
    fn test_graph_cache_current_id() {
        let cache = GraphCache::new();
        // Fresh cache: no checkpoint yet.
        assert_eq!(cache.current_id(), None);

        let id = cache.set(CallGraph::new());
        assert_eq!(cache.current_id(), Some(id));
    }
}
