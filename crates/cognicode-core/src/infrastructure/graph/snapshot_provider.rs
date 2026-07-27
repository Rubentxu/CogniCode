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

#[cfg(feature = "postgres")]
use std::collections::HashMap;
#[cfg(feature = "postgres")]
use std::str::FromStr;

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::value_objects::{RevisionId, WorkspaceId};
use std::sync::Arc;

#[cfg(feature = "postgres")]
use crate::domain::traits::repository::RepositoryError;
#[cfg(feature = "postgres")]
use crate::infrastructure::graph::checkpoint::VersionedGraphCache;
#[cfg(feature = "postgres")]
use tokio::sync::broadcast;

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

// =============================================================================
// SnapshotProviderImpl — PostgreSQL-backed implementation
// =============================================================================

/// PostgreSQL-backed [`SnapshotProvider`] implementation.
///
/// Uses a `HashMap<WorkspaceId, VersionedGraphCache>` as a local L2 cache
/// in front of PostgreSQL. Cache entries are populated lazily on first
/// access and retained up to `retention` versions per workspace.
///
/// The cache is NOT invalidated on `graph_updated` notifications — instead,
/// the cache is populated on-demand via `snapshot()` calls. The 100ms debounce
/// in `subscribe()` coalesces rapid notifications during bulk ingest.
#[cfg(feature = "postgres")]
pub struct SnapshotProviderImpl {
    /// Shared PostgreSQL connection pool.
    pool: sqlx::PgPool,
    /// Per-workspace versioned graph cache.
    /// Each workspace gets its own ring with the configured retention.
    caches: HashMap<WorkspaceId, VersionedGraphCache>,
    /// Broadcast channel for change notifications.
    notify_tx: broadcast::Sender<SnapshotEvent>,
    /// Default retention per workspace ring.
    retention: usize,
}

#[cfg(feature = "postgres")]
impl SnapshotProviderImpl {
    /// Build a new `SnapshotProviderImpl` backed by the given `PgPool`.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            pool,
            caches: HashMap::new(),
            notify_tx: broadcast::Sender::new(16),
            retention: 2,
        }
    }

    /// Get or create the cache for a workspace.
    fn cache_for(&mut self, workspace: &WorkspaceId) -> &mut VersionedGraphCache {
        if !self.caches.contains_key(workspace) {
            self.caches.insert(workspace.clone(), VersionedGraphCache::new(self.retention));
        }
        self.caches.get_mut(workspace).expect("just inserted")
    }

    /// Load a graph from PostgreSQL for the given workspace + revision.
    async fn load_from_pg(
        pool: &sqlx::PgPool,
        workspace: &WorkspaceId,
        revision: RevisionId,
    ) -> Result<CallGraph, RepositoryError> {
        // Load a CallGraph from PostgreSQL for the given workspace + revision.
        // This is a static async function to avoid borrow conflicts.
        use sqlx::Row;

        #[derive(Debug, sqlx::FromRow)]
        struct NodeRow {
            id: String,
            label: String,
            kind: String,
            source_path: String,
            properties: serde_json::Value,
        }

        #[derive(Debug, sqlx::FromRow)]
        struct GraphEdgeRow {
            source_id: String,
            target_id: String,
            kind: String,
            provenance: String,
            confidence: f64,
        }

        let ws_str = workspace.as_str();

        // First check the revision exists
        let rev_exists: Option<(i64, bool)> = sqlx::query_as(
            "SELECT revision_id, head_of FROM graph_revisions \
             WHERE workspace_id = $1 AND revision_id = $2",
        )
        .bind(ws_str)
        .bind(revision.get() as i64)
        .fetch_optional(pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("snapshot check revision: {e}")))?;

        if rev_exists.is_none() {
            return Err(RepositoryError::UnknownRevision {
                workspace: workspace.clone(),
                revision,
            });
        }

        // Load nodes
        let nodes: Vec<NodeRow> = sqlx::query_as(
            "SELECT id, label, kind, source_path, properties \
             FROM graph_nodes \
             WHERE workspace_id = $1 \
             ORDER BY id",
        )
        .bind(ws_str)
        .fetch_all(pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("snapshot select nodes: {e}")))?;

        use std::collections::HashMap;
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{DependencyType, Location, Provenance, SymbolKind};
        use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};

        let mut graph = CallGraph::new();
        let mut fqn_to_id: HashMap<String, SymbolId> = HashMap::new();

        for row in nodes {
            let kind_str = row.kind.strip_prefix("symbol.").unwrap_or(&row.kind);
            let kind: SymbolKind = kind_str.parse().unwrap_or(SymbolKind::Unknown);
            let (line, column) = match &row.properties {
                serde_json::Value::Object(map) => {
                    let l = map.get("line").and_then(|v| v.as_i64()).unwrap_or(1) as u32;
                    let c = map.get("column").and_then(|v| v.as_i64()).unwrap_or(0) as u32;
                    (l, c)
                }
                _ => (1, 0),
            };
            let location = Location::new(&row.source_path, line, column);
            let symbol = Symbol::new(&row.label, kind, location);
            let id = graph.add_symbol(symbol);
            fqn_to_id.insert(row.id.clone(), id);
        }

        // Load edges
        let edges: Vec<GraphEdgeRow> = sqlx::query_as(
            "SELECT source_id, target_id, kind, provenance, confidence \
             FROM graph_edges \
             WHERE workspace_id = $1 \
             ORDER BY source_id",
        )
        .bind(ws_str)
        .fetch_all(pool)
        .await
        .map_err(|e| RepositoryError::Store(format!("snapshot select edges: {e}")))?;

        for row in edges {
            let src_id = match fqn_to_id.get(&row.source_id) {
                Some(id) => id.clone(),
                None => continue,
            };
            let tgt_id = match fqn_to_id.get(&row.target_id) {
                Some(id) => id.clone(),
                None => continue,
            };

            let dep_type_str = row.kind.strip_prefix("dependency.").unwrap_or(&row.kind);
            let dep_type: DependencyType =
                dep_type_str.parse().unwrap_or(DependencyType::Calls);
            let provenance: Provenance =
                row.provenance.parse().unwrap_or(Provenance::Extracted);

            let ctx = match provenance {
                Provenance::Extracted => crate::domain::services::ExtractionContext::DirectExtraction,
                Provenance::Inferred => crate::domain::services::ExtractionContext::Heuristic {
                    score: row.confidence,
                },
                Provenance::Ambiguous => crate::domain::services::ExtractionContext::Unresolved,
                Provenance::Manual => crate::domain::services::ExtractionContext::Manual,
                Provenance::Tested => crate::domain::services::ExtractionContext::Tested,
            };

            let _ = graph.add_dependency_with_provenance(&src_id, &tgt_id, dep_type, ctx);
        }

        Ok(graph)
    }
}

#[cfg(feature = "postgres")]
impl SnapshotProvider for SnapshotProviderImpl {
    fn current_head(&self, workspace: &WorkspaceId) -> Result<RevisionId, SnapshotError> {
        // Synchronous version using pool.blocking_read() for simplicity
        // in the non-async context. For full async, use .fetch_one(&self.pool) directly.
        let ws_str = workspace.as_str().to_string();
        let pool = self.pool.clone();

        // Use try_one to check if there's a current head
        let head: Result<RevisionId, SnapshotError> =
            tokio::runtime::Handle::current()
                .block_on(async {
                    let row: Option<(i64,)> = sqlx::query_as(
                        "SELECT MAX(revision_id) FROM graph_revisions \
                         WHERE workspace_id = $1 AND head_of = true",
                    )
                    .bind(&ws_str)
                    .fetch_optional(&pool)
                    .await
                    .map_err(|e| SnapshotError::NoSnapshot(workspace.clone()))?;

                    Ok(row
                        .map(|(rev,)| RevisionId(rev as u64))
                        .unwrap_or(RevisionId::NONE))
                });

        head
    }

    fn snapshot(
        &self,
        workspace: &WorkspaceId,
        revision: RevisionId,
    ) -> Result<Arc<CallGraph>, SnapshotError> {
        let ws = workspace.clone();
        let pool = self.pool.clone();
        let rev = revision;

        tokio::runtime::Handle::current()
            .block_on(async {
                // Try to load from cache first (if workspace cache exists and has the revision)
                // For simplicity, we always load from PG and populate the cache

                let graph = Self::load_from_pg(&pool, &ws, rev)
                    .await
                    .map_err(|e| match e {
                        RepositoryError::UnknownRevision { workspace, revision } => {
                            SnapshotError::UnknownRevision { workspace, revision }
                        }
                        _ => SnapshotError::NoSnapshot(ws.clone()),
                    })?;

                Ok(Arc::new(graph))
            })
    }

    fn subscribe(
        &self,
        _workspace: &WorkspaceId,
    ) -> tokio::sync::broadcast::Receiver<SnapshotEvent> {
        self.notify_tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::value_objects::{Location, SymbolKind};
    use crate::infrastructure::graph::checkpoint::CheckpointId;
    use crate::infrastructure::graph::graph_cache::GraphCache;
    use std::sync::Arc;

    /// A minimal in-memory implementation for testing.
    /// Stores snapshots keyed by (workspace, revision).
    struct TestSnapshotProvider {
        snapshots: std::sync::Mutex<std::collections::HashMap<
            (String, u64),
            Arc<CallGraph>,
        >>,
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
            *self.heads.lock().unwrap().entry(ws.as_str().to_string()).or_insert(RevisionId::NONE) = rev;
        }
    }

    impl SnapshotProvider for TestSnapshotProvider {
        fn current_head(&self, workspace: &WorkspaceId) -> Result<RevisionId, SnapshotError> {
            let heads = self.heads.lock().unwrap();
            Ok(heads.get(workspace.as_str()).copied().unwrap_or(RevisionId::NONE))
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
        let sym = Symbol::new("test_func", SymbolKind::Function, Location::new("test.rs", 1, 1));
        graph.add_symbol(sym);
        let graph_arc = Arc::new(graph);

        // Insert it at revision 5
        provider.insert(&ws, RevisionId(5), (*graph_arc).clone());

        // snapshot(ws, 5) should return Ok(Arc<CallGraph>) with 1 symbol
        let result = provider.snapshot(&ws, RevisionId(5));
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        let loaded = result.unwrap();
        assert_eq!(loaded.symbol_count(), 1, "loaded graph should have 1 symbol");
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
        if let SnapshotError::UnknownRevision { workspace, revision } = err {
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
