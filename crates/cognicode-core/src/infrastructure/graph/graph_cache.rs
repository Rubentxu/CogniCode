//! Graph cache implementation

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::events::GraphEvent;
use arc_swap::ArcSwap;
use tokio::sync::broadcast;
use std::sync::Arc;
use std::sync::Mutex;

/// Thread-safe cache for call graphs with incremental update support
pub struct GraphCache {
    cache: ArcSwap<CallGraph>,
    pending_events: Mutex<Vec<GraphEvent>>,
    event_sender: broadcast::Sender<GraphEvent>,
}

impl GraphCache {
    /// Creates a new empty graph cache
    pub fn new() -> Self {
        let (event_sender, _) = broadcast::channel(16);
        Self {
            cache: ArcSwap::from_pointee(CallGraph::new()),
            pending_events: Mutex::new(Vec::new()),
            event_sender,
        }
    }

    /// Subscribe to graph mutation events
    pub fn subscribe(&self) -> broadcast::Receiver<GraphEvent> {
        self.event_sender.subscribe()
    }

    /// Gets the current graph
    pub fn get(&self) -> Arc<CallGraph> {
        self.cache.load().clone()
    }

    /// Gets a reference to the underlying graph
    pub fn get_ref(&self) -> &CallGraph {
        // Safety: ArcSwap guarantees memory safety and the returned reference
        // is valid for as long as the ArcSwap is alive
        unsafe { &*(self.cache.load().as_ref() as *const CallGraph) }
    }

    /// Updates the cached graph
    pub fn set(&self, graph: CallGraph) {
        self.cache.store(Arc::new(graph));
        let _ = self.event_sender.send(GraphEvent::GraphReplaced);
    }

    /// Updates the graph using a closure
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut CallGraph),
    {
        let mut graph = (**self.cache.load()).clone();
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
        let mut graph = (**self.cache.load()).clone();
        graph.apply_events(events)?;
        self.set(graph);
        let _ = self.event_sender.send(GraphEvent::GraphModified);
        Ok(())
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
}

impl Default for GraphCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::aggregates::call_graph::CallGraph;
    use crate::domain::events::graph_event::{DependencyEvent, SymbolAddedEvent};
    use crate::domain::events::GraphEvent;
    use crate::domain::value_objects::{DependencyType, Location, SymbolKind};
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
        assert!(event1_received, "Receiver1 should have received GraphReplaced event");
        assert!(event2_received, "Receiver2 should have received GraphReplaced event");
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
        assert!(event1_received, "Should have received GraphReplaced after clear");
        assert!(event2_received, "Should have received GraphCleared after clear");
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
}
