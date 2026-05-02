//! Contract tests for GraphStore implementations
//!
//! This module defines a test suite that verifies any GraphStore implementation
//! satisfies the contract expected by the system. These tests are instantiated
//! for InMemoryGraphStore.

#[cfg(test)]
mod tests {
    use crate::domain::aggregates::call_graph::CallGraph;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::traits::graph_store::GraphStore;
    use crate::domain::value_objects::file_manifest::FileManifest;
    use crate::domain::value_objects::{Location, SymbolKind};
    use std::path::PathBuf;

    /// Helper to create a test graph with some symbols
    fn create_test_graph() -> CallGraph {
        let mut graph = CallGraph::new();
        let symbol1 = Symbol::new(
            "test_function",
            SymbolKind::Function,
            Location::new("test_file.rs", 0, 0),
        );
        let symbol2 = Symbol::new(
            "TestStruct",
            SymbolKind::Struct,
            Location::new("test_file.rs", 10, 0),
        );
        graph.add_symbol(symbol1);
        graph.add_symbol(symbol2);
        graph
    }

    /// Helper to create a test manifest
    fn create_test_manifest() -> FileManifest {
        let mut manifest = FileManifest::new(PathBuf::from("/project"));
        manifest.update_entries(&[
            (PathBuf::from("src/main.rs"), "hash123".to_string(), 1000, 5),
            (PathBuf::from("src/lib.rs"), "hash456".to_string(), 2000, 3),
        ]);
        manifest
    }

    fn run_load_from_empty_returns_none<S: GraphStore>(store: &S) {
        let loaded = store.load_graph().unwrap();
        assert!(loaded.is_none(), "Loading from empty store should return None");
    }

    fn run_save_and_load_roundtrip<S: GraphStore>(store: &S) {
        let graph = create_test_graph();
        store.save_graph(&graph).unwrap();

        let loaded = store.load_graph().unwrap().unwrap();
        assert_eq!(
            loaded.symbol_count(),
            graph.symbol_count(),
            "Loaded graph should have same symbol count"
        );
    }

    fn run_corrupted_or_cleared_returns_none<S: GraphStore>(store: &S) {
        // Save valid data
        let graph = create_test_graph();
        store.save_graph(&graph).unwrap();
        assert!(store.load_graph().unwrap().is_some());

        // Clear the store
        store.clear().unwrap();

        // After clear, loading should return None
        let loaded = store.load_graph().unwrap();
        assert!(loaded.is_none(), "Loading after clear should return None");
    }

    fn run_manifest_roundtrip<S: GraphStore>(store: &S) {
        let manifest = create_test_manifest();
        store.save_manifest(&manifest).unwrap();

        let loaded = store.load_manifest().unwrap().unwrap();
        assert_eq!(
            loaded.entries.len(),
            manifest.entries.len(),
            "Loaded manifest should have same number of entries"
        );
        assert_eq!(
            loaded.get(&PathBuf::from("src/main.rs")).unwrap().content_hash,
            "hash123"
        );
    }

    fn run_clear_removes_graph_and_manifest<S: GraphStore>(store: &S) {
        // Save both graph and manifest
        let graph = create_test_graph();
        store.save_graph(&graph).unwrap();
        let manifest = create_test_manifest();
        store.save_manifest(&manifest).unwrap();

        // Verify both exist
        assert!(store.load_graph().unwrap().is_some());
        assert!(store.load_manifest().unwrap().is_some());

        // Clear
        store.clear().unwrap();

        // Both should be gone
        assert!(store.load_graph().unwrap().is_none());
        assert!(store.load_manifest().unwrap().is_none());
        assert!(!store.exists().unwrap());
    }

    fn run_exists_returns_false_when_empty<S: GraphStore>(store: &S) {
        assert!(!store.exists().unwrap(), "exists() should be false when store is empty");
    }

    fn run_exists_returns_true_when_has_data<S: GraphStore>(store: &S) {
        let graph = create_test_graph();
        store.save_graph(&graph).unwrap();
        assert!(store.exists().unwrap(), "exists() should be true after saving graph");
    }

    // ========================================================================
    // Test instantiation for InMemoryGraphStore
    // ========================================================================

    mod inmemory_tests {
        use super::*;
        use crate::infrastructure::persistence::InMemoryGraphStore;

        #[test]
        fn inmemory_load_from_empty_returns_none() {
            run_load_from_empty_returns_none(&InMemoryGraphStore::new());
        }

        #[test]
        fn inmemory_save_and_load_roundtrip() {
            run_save_and_load_roundtrip(&InMemoryGraphStore::new());
        }

        #[test]
        fn inmemory_corrupted_or_cleared_returns_none() {
            run_corrupted_or_cleared_returns_none(&InMemoryGraphStore::new());
        }

        #[test]
        fn inmemory_manifest_roundtrip() {
            run_manifest_roundtrip(&InMemoryGraphStore::new());
        }

        #[test]
        fn inmemory_clear_removes_graph_and_manifest() {
            run_clear_removes_graph_and_manifest(&InMemoryGraphStore::new());
        }

        #[test]
        fn inmemory_exists_returns_false_when_empty() {
            run_exists_returns_false_when_empty(&InMemoryGraphStore::new());
        }

        #[test]
        fn inmemory_exists_returns_true_when_has_data() {
            run_exists_returns_true_when_has_data(&InMemoryGraphStore::new());
        }
    }
}
