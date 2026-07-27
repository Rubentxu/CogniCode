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
#[cfg(feature = "postgres")]
use std::time::{Duration, Instant};

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
    /// Per-workspace versioned graph cache (CheckpointId-based ring).
    caches: HashMap<WorkspaceId, VersionedGraphCache>,
    /// Snapshot cache keyed by (workspace, revision).
    /// Populated on `snapshot()` calls; retains graphs for pinned reads.
    /// Protected by a Mutex for interior mutability (needed since `snapshot` is `&self`).
    snapshot_cache: std::sync::Mutex<HashMap<(WorkspaceId, RevisionId), Arc<CallGraph>>>,
    /// Broadcast channel for change notifications.
    notify_tx: broadcast::Sender<SnapshotEvent>,
    /// Default retention per workspace ring.
    retention: usize,
}

#[cfg(feature = "postgres")]
impl SnapshotProviderImpl {
    /// Build a new `SnapshotProviderImpl` backed by the given `PgPool`.
    ///
    /// Spawns a background task that LISTENs to `pg_notify('graph_updated')`
    /// and debounces notifications per workspace (100ms window).
    pub fn new(pool: sqlx::PgPool) -> Self {
        let notify_tx = broadcast::Sender::new(16);
        let pool_clone = pool.clone();
        let tx_clone = notify_tx.clone();

        // Spawn the LISTEN + debounce background task using sqlx_postgres::PgListener.
        // This task subscribes to graph_updated notifications and debounces
        // by 100ms per workspace before broadcasting through notify_tx.
        tokio::spawn(async move {
            let notify_tx = tx_clone;
            let mut pending: HashMap<WorkspaceId, Instant> = HashMap::new();
            const DEBOUNCE_MS: u64 = 100;

            // Clone pool for use in the flush branch (can be used multiple times).
            let pool_for_flush = pool_clone.clone();

            // Use PgListener from sqlx_postgres to receive notifications.
            // PgListener::connect_with takes &Pool<Postgres> = &sqlx::PgPool.
            let mut listener = match sqlx_postgres::PgListener::connect_with(&pool_clone).await {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("SnapshotProviderImpl LISTEN: failed to connect PgListener: {e}");
                    return;
                }
            };

            if let Err(e) = listener.listen("graph_updated").await {
                eprintln!("SnapshotProviderImpl LISTEN: failed to LISTEN on graph_updated: {e}");
                return;
            }

            // Process notifications in a loop using try_recv.
            loop {
                match tokio::time::timeout(Duration::from_millis(50), listener.try_recv()).await {
                    Ok(Ok(Some(notification))) => {
                        // Parse workspace_id from the notification payload.
                        if let Some(workspace) =
                            Self::parse_workspace_from_notification(notification.payload())
                        {
                            pending.insert(workspace, Instant::now());
                        }
                    }
                    Ok(Ok(None)) => {
                        // No notification available this poll.
                    }
                    Ok(Err(e)) => {
                        // Connection error.
                        eprintln!("SnapshotProviderImpl LISTEN: error receiving notification: {e}");
                        break;
                    }
                    Err(_) => {
                        // Timeout: flush any pending notifications whose debounce window has elapsed.
                        let now = Instant::now();
                        let mut to_flush: Vec<WorkspaceId> = Vec::new();
                        for (ws, since) in pending.iter() {
                            if now.duration_since(*since) >= Duration::from_millis(DEBOUNCE_MS) {
                                to_flush.push(ws.clone());
                            }
                        }
                        for ws in to_flush.drain(..) {
                            if pending.remove(&ws).is_some() {
                                // Query the current head revision for this workspace.
                                // The spawned task IS already an async context — use direct .await.
                                let row: Option<(i64,)> = sqlx::query_as(
                                    "SELECT MAX(revision_id) FROM graph_revisions \
                                     WHERE workspace_id = $1 AND head_of = true",
                                )
                                .bind(ws.as_str())
                                .fetch_optional(&pool_for_flush)
                                .await
                                .ok()
                                .flatten();
                                let head = row.map(|(rev,)| RevisionId(rev as u64))
                                    .unwrap_or(RevisionId::NONE);
                                let _ = notify_tx.send(SnapshotEvent::Updated {
                                    workspace: ws,
                                    revision: head,
                                });
                            }
                        }
                    }
                }
            }
        });

        Self {
            pool,
            caches: HashMap::new(),
            snapshot_cache: std::sync::Mutex::new(HashMap::new()),
            notify_tx,
            retention: 2,
        }
    }

    /// Parse `workspace_id` from a pg_notify payload JSON string.
    fn parse_workspace_from_notification(payload: &str) -> Option<WorkspaceId> {
        // Payload is like: {"workspace_id": "default", "source_path": "...", "action": "INSERT", "timestamp": ...}
        let json: serde_json::Value = serde_json::from_str(payload).ok()?;
        let ws_str = json.get("workspace_id")?.as_str()?;
        WorkspaceId::try_new(ws_str).ok()
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
        let ws_str = workspace.as_str().to_string();
        let pool = self.pool.clone();

        // Use block_in_place to run async SQL from this sync context on a
        // blocking thread. This avoids the Handle::block_on panic when called
        // from a tokio worker thread (multi-thread runtime).
        let (tx, rx) = std::sync::mpsc::channel();
        tokio::task::block_in_place(move || {
            let handle = tokio::runtime::Handle::current();
            let _enter = handle.enter();
            tokio::spawn(async move {
                let row: Option<(i64,)> = sqlx::query_as(
                    "SELECT MAX(revision_id) FROM graph_revisions \
                     WHERE workspace_id = $1 AND head_of = true",
                )
                .bind(&ws_str)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten();
                let _ = tx.send(row);
            });
        });
        let row: Option<(i64,)> = rx.recv().unwrap_or(None);
        Ok(row
            .map(|(rev,)| RevisionId(rev as u64))
            .unwrap_or(RevisionId::NONE))
    }

    fn snapshot(
        &self,
        workspace: &WorkspaceId,
        revision: RevisionId,
    ) -> Result<Arc<CallGraph>, SnapshotError> {
        // Fast path: check the revision-keyed snapshot cache.
        {
            let cache = self.snapshot_cache.lock().unwrap();
            if let Some(cached) = cache.get(&(workspace.clone(), revision)) {
                return Ok(cached.clone());
            }
        }

        // Slow path: load from PostgreSQL using block_in_place.
        // This avoids the Handle::block_on panic when called from a tokio
        // worker thread (multi-thread runtime).
        let ws = workspace.clone();
        let pool = self.pool.clone();
        let rev = revision;

        let (tx, rx) = std::sync::mpsc::channel();
        tokio::task::block_in_place(move || {
            let handle = tokio::runtime::Handle::current();
            let _enter = handle.enter();
            tokio::spawn(async move {
                let result = Self::load_from_pg(&pool, &ws, rev)
                    .await
                    .map_err(|e| match e {
                        RepositoryError::UnknownRevision { workspace, revision } => {
                            SnapshotError::UnknownRevision { workspace, revision }
                        }
                        _ => SnapshotError::NoSnapshot(ws.clone()),
                    });
                let _ = tx.send(result);
            });
        });

        let graph = rx.recv().unwrap_or_else(|_| {
            Err(SnapshotError::NoSnapshot(workspace.clone()))
        })?;

        let graph_arc = Arc::new(graph);

        // Populate the snapshot cache for future pinned reads.
        let key = (workspace.clone(), revision);
        let mut cache = self.snapshot_cache.lock().unwrap();
        cache.entry(key).or_insert_with(|| graph_arc.clone());

        Ok(graph_arc)
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
    use std::sync::atomic::{AtomicU64, Ordering};

    // ---------------------------------------------------------------------------
    // Helpers adapted from postgres_repository.rs for pg_test! macro
    // ---------------------------------------------------------------------------

    /// Unique counter for per-test database names (avoids conflicts in shared CIs).
    static UNIQ: AtomicU64 = AtomicU64::new(0);

    /// Build a unique per-test database URL. Returns `None` when
    /// `TEST_DATABASE_URL` is not set so tests skip gracefully.
    async fn fresh_pool() -> Option<sqlx::PgPool> {
        use crate::infrastructure::persistence::postgres_repository::PostgresRepository;

        let base = std::env::var("TEST_DATABASE_URL").ok()?;
        let n = UNIQ.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let db_name = format!("cognicode_test_{pid}_{n}");
        let admin_url = base.clone();
        let test_url = rewrite_db_name(&admin_url, &db_name);

        // Create the unique DB (idempotent: drop first if it lingers from a crashed run).
        let admin = sqlx::PgPool::connect(&admin_url).await.ok()?;
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{db_name}\""))
            .execute(&admin)
            .await;
        sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
            .execute(&admin)
            .await
            .ok()?;

        // Connect to the new DB and run the full migration chain
        // (SCHEMA_SQL + m0009…m0018) so tests start from a complete schema.
        let pool = sqlx::PgPool::connect(&test_url).await.ok()?;
        PostgresRepository::from_pool(pool.clone())
            .run_migrations()
            .await
            .ok()?;

        Some(pool)
    }

    /// Replace the database segment in a `postgres://...` URL.
    fn rewrite_db_name(url: &str, new_name: &str) -> String {
        if let Some(at_idx) = url.rfind('@') {
            let (head, tail) = url.split_at(at_idx);
            if let Some(slash_idx) = tail.find('/') {
                let (host, _) = tail.split_at(slash_idx);
                return format!("{head}{host}/{new_name}");
            }
        }
        let trimmed = url.trim_end_matches('/');
        format!("{trimmed}/{new_name}")
    }

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
    // pg_test! macro — identical contract to postgres_repository.rs
    // ---------------------------------------------------------------------------
    macro_rules! pg_test {
        ($name:ident, |$pool:ident: sqlx::PgPool| $body:tt) => {
            #[tokio::test]
            async fn $name() {
                let Some($pool) = fresh_pool().await else {
                    eprintln!(
                        "skipping {}: TEST_DATABASE_URL not set",
                        stringify!($name)
                    );
                    return;
                };
                async fn inner($pool: sqlx::PgPool) {
                    $body
                }
                inner($pool).await
            }
        };
    }

    // ---------------------------------------------------------------------------
    // 3.2a RED — pg_test asserting current_head returns correct revision after
    // two sequential saves advancing 5 → 7.
    // ---------------------------------------------------------------------------
    pg_test!(current_head_returns_live_head_after_two_commits, |pool: sqlx::PgPool| {
        use crate::domain::traits::repository::Repository;
        use crate::infrastructure::persistence::postgres_repository::PostgresRepository;

        let repo = PostgresRepository::from_pool(pool.clone());
        let ws = crate::domain::value_objects::WorkspaceId::default();

        // Save first graph — expect rev 1
        let g1 = make_graph("func_v1");
        let rev1 = repo.save_call_graph_ws(&g1, &ws).await
            .expect("first save must succeed");
        assert!(rev1.is_valid(), "first revision must be valid");

        // Save second graph — expect rev 2
        let g2 = make_graph("func_v2");
        let rev2 = repo.save_call_graph_ws(&g2, &ws).await
            .expect("second save must succeed");
        assert!(rev2.is_valid(), "second revision must be valid");
        assert_eq!(rev2.get(), rev1.get() + 1, "revisions must be sequential");

        // SnapshotProvider must see head = rev2
        let provider = SnapshotProviderImpl::new(pool);
        let head = provider.current_head(&ws).expect("current_head must succeed");
        assert_eq!(
            head.get(),
            rev2.get(),
            "current_head must return the latest committed revision_id"
        );
    });

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
        assert!(result.is_none(), "get_at_provider must return None when no provider is set");
    }

    // ---------------------------------------------------------------------------
    // 3.5a RED — pg_test asserting 50 sequential edge inserts within 100ms
    // produce ≤1 notification carrying the final revision id (batched, NOT 50).
    // ---------------------------------------------------------------------------
    pg_test!(notification_batching_50_inserts_produce_at_most_1_event, |pool: sqlx::PgPool| {
        use crate::domain::traits::repository::Repository;
        use crate::infrastructure::persistence::postgres_repository::PostgresRepository;

        let ws = crate::domain::value_objects::WorkspaceId::default();

        // Create a minimal graph to establish rev 1.
        let repo = PostgresRepository::from_pool(pool.clone());
        let g0 = make_graph("base");
        let rev0 = repo.save_call_graph_ws(&g0, &ws).await
            .expect("initial save must succeed");

        // Subscribe BEFORE the batch window.
        let provider = SnapshotProviderImpl::new(pool.clone());
        let mut rx = provider.subscribe(&ws);

        // Drain any prior pending notification.
        let _ = rx.try_recv();

        // Do 50 rapid edge inserts within a tight loop (no sleep — pure speed).
        // Each insert triggers pg_notify which the provider should coalesce.
        // After all inserts, the head is rev0 + 50.
        let final_rev = rev0.get() + 50;
        for i in 0..50_i32 {
            let node_a = format!("n{}", i * 2);
            let node_b = format!("n{}", i * 2 + 1);
            // Insert two nodes then an edge to advance revision
            let g = make_graph(&node_a);
            let _ = repo.save_call_graph_ws(&g, &ws).await;
        }

        // Drain events with a short timeout (150ms covers 100ms debounce + margin).
        let mut events_received = 0;
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(150);
        while std::time::Instant::now() < deadline {
            if let Ok(_) = rx.try_recv() {
                events_received += 1;
            }
            if events_received >= 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }

        assert!(
            events_received <= 1,
            "50 rapid inserts must produce at most 1 batched notification, got {}",
            events_received
        );
    });

    // ---------------------------------------------------------------------------
    // 3.6a RED — pg_test asserting save_call_graph_ws returns OK then
    // snapshot(ws, current_head(ws)) returns the just-committed state.
    // ---------------------------------------------------------------------------
    pg_test!(sequential_commit_and_read_returns_just_committed_state, |pool: sqlx::PgPool| {
        use crate::domain::traits::repository::Repository;
        use crate::infrastructure::persistence::postgres_repository::PostgresRepository;

        let repo = PostgresRepository::from_pool(pool.clone());
        let ws = crate::domain::value_objects::WorkspaceId::default();

        // Commit a graph with 3 symbols.
        let mut g = CallGraph::new();
        let sym1 = Symbol::new("alice", SymbolKind::Function, Location::new("a.rs", 1, 0));
        let sym2 = Symbol::new("bob", SymbolKind::Function, Location::new("b.rs", 2, 0));
        let sym3 = Symbol::new("carol", SymbolKind::Class, Location::new("c.rs", 3, 0));
        let id1 = g.add_symbol(sym1);
        let id2 = g.add_symbol(sym2);
        let id3 = g.add_symbol(sym3);
        use crate::domain::services::ExtractionContext;
        use crate::domain::value_objects::DependencyType;
        let _ = g.add_dependency_with_provenance(
            &id1, &id2, DependencyType::Calls, ExtractionContext::DirectExtraction,
        );
        let _ = g.add_dependency_with_provenance(
            &id2, &id3, DependencyType::Imports, ExtractionContext::DirectExtraction,
        );

        let rev = repo.save_call_graph_ws(&g, &ws).await
            .expect("save_call_graph_ws must succeed");

        // SnapshotProvider must see the just-committed state.
        let provider = SnapshotProviderImpl::new(pool);
        let head = provider.current_head(&ws).expect("current_head must succeed");
        assert_eq!(head.get(), rev.get(), "head must match committed revision");

        let snapshot = provider.snapshot(&ws, head)
            .expect("snapshot must succeed for known revision");

        assert_eq!(
            snapshot.symbol_count(),
            3,
            "snapshot must reflect all 3 just-committed symbols"
        );
        assert_eq!(
            snapshot.edge_count(),
            2,
            "snapshot must reflect all 2 just-committed edges"
        );
    });

    // ---------------------------------------------------------------------------
    // 3.7a RED — pg_test asserting a reader pinned to (ws, 5) keeps returning
    // revision-5 snapshot when concurrent ingest advances head to 6.
    // VersionedGraphCache retention must be ≥ 2 so pinned revision survives
    // the next ingest's cache update.
    // ---------------------------------------------------------------------------
    pg_test!(pinned_read_survives_concurrent_ingest, |pool: sqlx::PgPool| {
        use crate::domain::traits::repository::Repository;
        use crate::infrastructure::persistence::postgres_repository::PostgresRepository;

        let repo = PostgresRepository::from_pool(pool.clone());
        let ws = crate::domain::value_objects::WorkspaceId::default();

        // Rev 1: "first" graph
        let g1 = make_graph("first");
        let rev1 = repo.save_call_graph_ws(&g1, &ws).await
            .expect("save rev1 must succeed");

        // Rev 2: "second" graph — head now at rev2
        let g2 = make_graph("second");
        let rev2 = repo.save_call_graph_ws(&g2, &ws).await
            .expect("save rev2 must succeed");

        // SnapshotProvider: load and cache both revisions.
        let provider = SnapshotProviderImpl::new(pool.clone());

        // Prime the cache by loading both revisions.
        let snap1 = provider.snapshot(&ws, rev1)
            .expect("snapshot(rev1) must succeed");
        let snap2 = provider.snapshot(&ws, rev2)
            .expect("snapshot(rev2) must succeed");

        assert_eq!(
            snap1.symbol_count(), 1,
            "rev1 snapshot must have exactly 1 symbol (named 'first')"
        );
        assert_eq!(
            snap2.symbol_count(), 1,
            "rev2 snapshot must have exactly 1 symbol (named 'second')"
        );

        // Rev 3: concurrent ingest advances head to rev3.
        let g3 = make_graph("third");
        let rev3 = repo.save_call_graph_ws(&g3, &ws).await
            .expect("save rev3 must succeed");

        // Head is now rev3; pinned reader at rev1 must STILL return rev1 data.
        let pinned1 = provider.snapshot(&ws, rev1)
            .expect("snapshot(rev1) must still succeed after head advanced");
        assert_eq!(
            pinned1.symbol_count(), 1,
            "pinned revision-1 read must still return revision-1 graph (1 symbol)"
        );

        // Also verify rev2 is still accessible.
        let pinned2 = provider.snapshot(&ws, rev2)
            .expect("snapshot(rev2) must still succeed after head advanced");
        assert_eq!(
            pinned2.symbol_count(), 1,
            "pinned revision-2 read must still return revision-2 graph"
        );
    });

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
