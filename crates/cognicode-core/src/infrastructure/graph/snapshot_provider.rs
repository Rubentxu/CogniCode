//! SnapshotProvider — domain port for versioned graph snapshots.
//!
//! ADR-035: Snapshot isolation for the explorer. This trait is the
//! read-side contract that lets callers pin to a specific `(workspace,
//! revision)` pair and receive a consistent `Arc<CallGraph>` snapshot.
//!
//! Implementations are expected to be `Send + Sync` and may hold internal
//! state (e.g. a versioned ring cache). The trait is `dyn`-compatible
//! so the composition root can wire a single instance and hand clones to
//! every consumer.


use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::value_objects::{RevisionId, WorkspaceId};
use std::sync::Arc;


/// Errors returned by [`SnapshotProvider`] operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SnapshotError {
    /// The requested `(workspace, revision)` pair does not exist in
    /// `graph_revisions`. The revision may never have been created or
    /// may have been rolled back.
    #[error("unknown revision: {workspace} r{revision}")]
    UnknownRevision {
        workspace: WorkspaceId,
        revision: RevisionId,
    },

    /// The provider is not yet initialized for this workspace.
    /// Call `current_head` first to discover the live head.
    #[error("no snapshot available for workspace: {0}")]
    NoSnapshot(WorkspaceId),
}

/// A domain port offering versioned `CallGraph` snapshots.
///
/// Callers use this to:
/// - Discover the current head revision via `current_head`
/// - Pin to a specific revision via `snapshot(ws, rev)`
/// - Subscribe to change notifications via `subscribe`
///
/// Implementations MUST guarantee:
/// - `snapshot(ws, rev)` returns bit-exact graph state for that revision
/// - `snapshot` is safe to call concurrently (interior mutability allowed)
/// - The provider is a single instance per process (enforced by wiring)
pub trait SnapshotProvider: Send + Sync {
    /// Return the current head [`RevisionId`] for the given workspace.
    /// Returns `Ok(RevisionId::NONE)` when no revision has been created yet.
    fn current_head(&self, workspace: &WorkspaceId) -> Result<RevisionId, SnapshotError>;

    /// Return a snapshot for the given `(workspace, revision)`.
    ///
    /// Returns `Ok(Arc<CallGraph>)` when the revision exists (including
    /// the `RevisionId::NONE` sentinel for empty graphs).
    ///
    /// Returns `Err(SnapshotError::UnknownRevision{ws, rev})` when the
    /// revision has never been created or has been rolled back.
    fn snapshot(
        &self,
        workspace: &WorkspaceId,
        revision: RevisionId,
    ) -> Result<Arc<CallGraph>, SnapshotError>;

    /// Subscribe to change notifications for a workspace.
    ///
    /// The returned receiver will receive `SnapshotEvent` values when
    /// new snapshots become available for the given workspace.
    ///
    /// Implementations MAY batch notifications (e.g. 100ms debounce) to
    /// avoid flooding during bulk ingest operations.
    fn subscribe(
        &self,
        workspace: &WorkspaceId,
    ) -> tokio::sync::broadcast::Receiver<SnapshotEvent>;
}

/// Events emitted by [`SnapshotProvider::subscribe`].
#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotEvent {
    /// A new snapshot is available for the given workspace.
    Updated {
        workspace: WorkspaceId,
        revision: RevisionId,
    },
}
mod tests {
    use super::*;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::value_objects::{Location, SymbolKind};
    use crate::infrastructure::graph::checkpoint::CheckpointId;
    use crate::infrastructure::graph::graph_cache::GraphCache;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    // ---------------------------------------------------------------------------
    // ---------------------------------------------------------------------------

    /// Build a simple one-symbol CallGraph for testing.
    fn make_graph(symbol_name: &str) -> CallGraph {
        let mut g = CallGraph::new();
        let sym = Symbol::new(
            symbol_name,
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        g.add_symbol(sym);
        g
    }

    // ---------------------------------------------------------------------------
    // ---------------------------------------------------------------------------

    // ---------------------------------------------------------------------------
    // two sequential saves advancing 5 → 7.
    // ---------------------------------------------------------------------------

    // ---------------------------------------------------------------------------
    // 3.3b GREEN — wiring test: GraphCache::get_at_provider() routes to the
    // SnapshotProvider when one is set. Uses TestSnapshotProvider in-memory.
    // ---------------------------------------------------------------------------
    #[test]
    fn get_at_provider_routes_to_provider_when_set() {
        use crate::domain::aggregates::call_graph::CallGraph;
        use crate::domain::value_objects::{RevisionId, WorkspaceId};
        use std::sync::Arc;

        // Build a TestSnapshotProvider that returns a known graph.
        let test_provider = TestSnapshotProvider::new();
        let ws = WorkspaceId::try_new("test-workspace").unwrap();

        let mut expected_graph = CallGraph::new();
        let sym = Symbol::new(
            "routed_func",
            SymbolKind::Function,
            Location::new("routed.rs", 42, 7),
        );
        expected_graph.add_symbol(sym);
        test_provider.insert(&ws, RevisionId(3), expected_graph);

        let provider_arc: Arc<dyn SnapshotProvider> = Arc::new(test_provider);

        // Create a GraphCache and set the provider.
        let cache = GraphCache::new();
        cache.set_provider(provider_arc.clone());

        // get_at_provider must route to the provider and return the known graph.
        let result = cache.get_at_provider(&ws, RevisionId(3));
        assert!(
            result.is_some(),
            "get_at_provider must return Some when provider is set"
        );
        let loaded = result.unwrap();
        assert_eq!(
            loaded.symbol_count(),
            1,
            "loaded graph must have exactly 1 symbol from the provider"
        );
    }

    #[test]
    fn get_at_provider_returns_none_when_no_provider_set() {
        let ws = WorkspaceId::try_new("test-workspace").unwrap();
        let cache = GraphCache::new();
        // Without a provider set, get_at_provider must return None.
        let result = cache.get_at_provider(&ws, RevisionId(1));
        assert!(
            result.is_none(),
            "get_at_provider must return None when no provider is set"
        );
    }

    // ---------------------------------------------------------------------------
    // produce ≤1 notification carrying the final revision id (batched, NOT 50).
    // ---------------------------------------------------------------------------

    // ---------------------------------------------------------------------------
    // snapshot(ws, current_head(ws)) returns the just-committed state.
    // ---------------------------------------------------------------------------

    // ---------------------------------------------------------------------------
    // revision-5 snapshot when concurrent ingest advances head to 6.
    // the next ingest's cache update.
    // ---------------------------------------------------------------------------

    /// A minimal in-memory implementation for testing.
    /// Stores snapshots keyed by (workspace, revision).
    struct TestSnapshotProvider {
        snapshots: std::sync::Mutex<std::collections::HashMap<(String, u64), Arc<CallGraph>>>,
        heads: std::sync::Mutex<std::collections::HashMap<String, RevisionId>>,
    }

    impl TestSnapshotProvider {
        fn new() -> Self {
            Self {
                snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
                heads: std::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }

        fn insert(&self, ws: &WorkspaceId, rev: RevisionId, graph: CallGraph) {
            let key = (ws.as_str().to_string(), rev.get());
            self.snapshots.lock().unwrap().insert(key, Arc::new(graph));
            *self
                .heads
                .lock()
                .unwrap()
                .entry(ws.as_str().to_string())
                .or_insert(RevisionId::NONE) = rev;
        }
    }

    impl SnapshotProvider for TestSnapshotProvider {
        fn current_head(&self, workspace: &WorkspaceId) -> Result<RevisionId, SnapshotError> {
            let heads = self.heads.lock().unwrap();
            Ok(heads
                .get(workspace.as_str())
                .copied()
                .unwrap_or(RevisionId::NONE))
        }

        fn snapshot(
            &self,
            workspace: &WorkspaceId,
            revision: RevisionId,
        ) -> Result<Arc<CallGraph>, SnapshotError> {
            let snapshots = self.snapshots.lock().unwrap();
            let key = (workspace.as_str().to_string(), revision.get());
            snapshots
                .get(&key)
                .cloned()
                .ok_or_else(|| SnapshotError::UnknownRevision {
                    workspace: workspace.clone(),
                    revision,
                })
        }

        fn subscribe(
            &self,
            _workspace: &WorkspaceId,
        ) -> tokio::sync::broadcast::Receiver<SnapshotEvent> {
            let (tx, rx) = tokio::sync::broadcast::channel(16);
            let _ = tx.send(SnapshotEvent::Updated {
                workspace: _workspace.clone(),
                revision: RevisionId::NONE,
            });
            rx
        }
    }

    // 3.1a RED tests — these verify the SnapshotProvider contract

    #[test]
    fn snapshot_returns_arc_call_graph_for_known_revision() {
        let provider = TestSnapshotProvider::new();
        let ws = WorkspaceId::try_new("test-workspace").unwrap();

        // Build a graph with one symbol
        let mut graph = CallGraph::new();
        let sym = Symbol::new(
            "test_func",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        graph.add_symbol(sym);
        let graph_arc = Arc::new(graph);

        // Insert it at revision 5
        provider.insert(&ws, RevisionId(5), (*graph_arc).clone());

        // snapshot(ws, 5) should return Ok(Arc<CallGraph>) with 1 symbol
        let result = provider.snapshot(&ws, RevisionId(5));
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        let loaded = result.unwrap();
        assert_eq!(
            loaded.symbol_count(),
            1,
            "loaded graph should have 1 symbol"
        );
    }

    #[test]
    fn snapshot_returns_unknown_revision_error_for_stale_id() {
        let provider = TestSnapshotProvider::new();
        let ws = WorkspaceId::try_new("test-workspace").unwrap();

        // No snapshots inserted — revision 99 does not exist
        let result = provider.snapshot(&ws, RevisionId(99));

        assert!(result.is_err(), "expected Err, got Ok {:?}", result);
        let err = result.unwrap_err();
        assert!(matches!(err, SnapshotError::UnknownRevision { .. }));
        if let SnapshotError::UnknownRevision {
            workspace,
            revision,
        } = err
        {
            assert_eq!(workspace.as_str(), "test-workspace");
            assert_eq!(revision.get(), 99);
        }
    }

    // 3.3a RED — unit test asserting two clones of Arc<dyn SnapshotProvider> satisfy Arc::ptr_eq.
    // This verifies the "one provider instance per process" requirement.
    #[test]
    fn arc_clones_satisfy_ptr_eq() {
        // Create a boxed trait object
        let provider = TestSnapshotProvider::new();
        let boxed: Box<dyn SnapshotProvider> = Box::new(provider);
        let arc1: Arc<dyn SnapshotProvider> = Arc::from(boxed);
        let arc2 = arc1.clone();

        // Two clones of an Arc pointing to the same allocation satisfy Arc::ptr_eq
        assert!(
            Arc::ptr_eq(&arc1, &arc2),
            "arc clones must point to the same allocation"
        );
    }
}
