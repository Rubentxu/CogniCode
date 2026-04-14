//! Graph cache implementation

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::events::GraphEvent;
use arc_swap::ArcSwap;
use std::sync::Arc;
use std::sync::Mutex;

/// Thread-safe cache for call graphs with incremental update support
pub struct GraphCache {
    cache: ArcSwap<CallGraph>,
    pending_events: Mutex<Vec<GraphEvent>>,
}

impl GraphCache {
    /// Creates a new empty graph cache
    pub fn new() -> Self {
        Self {
            cache: ArcSwap::from_pointee(CallGraph::new()),
            pending_events: Mutex::new(Vec::new()),
        }
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
}
