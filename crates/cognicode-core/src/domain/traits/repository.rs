//! Async-ready canonical [`CallGraphStore`] port for the cognicode domain.
//!
//! This trait is the structural seam for the
//! canonical implementation that lands in a follow-up slice. It is intentionally
//! **standalone** — it does NOT inherit from [`GraphStore`] — so the
//! write-path (synchronous save/load of bincode blobs) and the
//! read-path (async, query-shaped) remain independent seams. A future
//! struct can implement both traits side by side.
//!
//! The trait is `Send + Sync` and uses `#[async_trait]` so it remains
//! dyn-compatible (e.g. `Box<dyn CallGraphStore>`) for application code
//! that wants to swap implementations at runtime.
//!
//! **Naming**: this trait was previously called `Repository`. It was
//! renamed to `CallGraphStore` (2026-07-30) to disambiguate from
//! the many other `*Repository` ports in the workspace (e.g.
//! `SymbolRepository`, `GraphRepository`, `AdrRepository`,
//! `DocRepository`, `EvidenceRepository`). The original name was
//! too generic to convey what it operates on. The `Store` suffix
//! follows the workspace convention for ports (see also
//! `InvestigationStore`, `ViewSpecStore`, `RunLineageStore`).

use async_trait::async_trait;
use thiserror::Error;

use crate::domain::aggregates::{CallGraph, Symbol};
use crate::domain::value_objects::{EdgeMetadata, RevisionId, WorkspaceId};

/// Error type for [`CallGraphStore`] operations.
///
/// Distinct from [`crate::domain::traits::graph_store::StoreError`]
/// (which models the synchronous blob persistence path) so that async
/// query failures can carry query-specific context without polluting
/// the persistence surface.
#[derive(Debug, Error)]
pub enum CallGraphStoreError {
    #[error("store error: {0}")]
    Store(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid query: {0}")]
    InvalidQuery(String),

    /// A unique-constraint violation. Raised by
    /// `save_named_view` when a `(workspace_id, owner, name)` triple
    /// already exists.
    #[error("unique violation: {0}")]
    UniqueViolation(String),

    /// Requested revision does not exist in `graph_revisions` for the
    /// given workspace. The revision may have been never created or
    /// rolled back.
    #[error("unknown revision: {workspace} r revision {revision}")]
    UnknownRevision {
        workspace: crate::domain::value_objects::WorkspaceId,
        revision: crate::domain::value_objects::RevisionId,
    },
}

/// Async-ready read-side port over the canonical symbol/edge store.
///
/// Implementations are expected to be `Send + Sync`. The trait
/// starts with symbol queries and grows with edge queries (see
/// `explorer-graph-postgres-call-edges`). The seam is additive: every
/// previous method signature stays stable across minor versions.
///
/// Renamed from `Repository` (2026-07-30) — see the module doc for
/// the rationale.
#[async_trait]
pub trait CallGraphStore: Send + Sync {
    /// Look up a symbol by its fully-qualified name (the canonical
    /// `SymbolId` form: `file:name:line`). Returns `None` when no
    /// symbol matches.
    async fn find_symbol_by_qualified_name(
        &self,
        name: &str,
    ) -> Result<Option<Symbol>, CallGraphStoreError>;

    /// Count every indexed symbol. Cheap call — implementations are
    /// expected to delegate to a precomputed count when available
    /// (e.g. a precomputed count table or materialized view).
    async fn count_symbols(&self) -> Result<usize, CallGraphStoreError>;

    /// Return every call-graph edge whose `caller_id` matches
    /// `caller_id`. The empty case MUST be `Ok(Vec::new())` — never
    /// an error — so callers can branch on the result length without
    /// distinguishing "no rows" from "query failed".
    ///
    /// Implementations order results by the internal surrogate key
    /// (`id`) for stable test assertions; no ordering contract is
    /// guaranteed beyond that.
    async fn find_edges_by_caller(
        &self,
        caller_id: &str,
    ) -> Result<Vec<EdgeMetadata>, CallGraphStoreError>;

    /// Return every call-graph edge whose `callee_id` matches
    /// `callee_id`. Same empty-result contract as
    /// [`Repository::find_edges_by_caller`].
    async fn find_edges_by_callee(
        &self,
        callee_id: &str,
    ) -> Result<Vec<EdgeMetadata>, CallGraphStoreError>;

    /// Count every indexed call-graph edge. Cheap call — delegates
    /// to `SELECT COUNT(*)` on the `call_edges` table.
    async fn count_edges(&self) -> Result<usize, CallGraphStoreError>;

    /// Load the call graph snapshot for a specific revision.
    ///
    /// Returns `Ok(Some(CallGraph))` when the revision exists,
    /// `Ok(None)` when the workspace has no graph yet (empty graph),
    /// and `Err(UnknownRevision{ws, rev})` when the revision does not
    /// exist in `graph_revisions` for this workspace.
    async fn load_call_graph_pinned(
        &self,
        workspace: &WorkspaceId,
        revision: RevisionId,
    ) -> Result<Option<CallGraph>, CallGraphStoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Minimal in-test implementation that exercises the
    /// `dyn Repository` path. Stores an empty symbol/edge set; every
    /// method succeeds but returns `None` / `0` / `vec![]`.
    struct EmptyRepo;

    #[async_trait]
    impl CallGraphStore for EmptyRepo {
        async fn find_symbol_by_qualified_name(
            &self,
            _name: &str,
        ) -> Result<Option<Symbol>, CallGraphStoreError> {
            Ok(None)
        }

        async fn count_symbols(&self) -> Result<usize, CallGraphStoreError> {
            Ok(0)
        }

        async fn find_edges_by_caller(
            &self,
            _caller_id: &str,
        ) -> Result<Vec<EdgeMetadata>, CallGraphStoreError> {
            Ok(Vec::new())
        }

        async fn find_edges_by_callee(
            &self,
            _callee_id: &str,
        ) -> Result<Vec<EdgeMetadata>, CallGraphStoreError> {
            Ok(Vec::new())
        }

        async fn count_edges(&self) -> Result<usize, CallGraphStoreError> {
            Ok(0)
        }

        async fn load_call_graph_pinned(
            &self,
            _workspace: &WorkspaceId,
            _revision: RevisionId,
        ) -> Result<Option<CallGraph>, CallGraphStoreError> {
            Ok(None)
        }
    }

    /// A second implementation to prove the trait is open to
    /// additional implementors in the same compilation unit.
    struct CountingRepo {
        symbols: usize,
        edges: usize,
    }

    #[async_trait]
    impl CallGraphStore for CountingRepo {
        async fn find_symbol_by_qualified_name(
            &self,
            _name: &str,
        ) -> Result<Option<Symbol>, CallGraphStoreError> {
            Ok(None)
        }

        async fn count_symbols(&self) -> Result<usize, CallGraphStoreError> {
            Ok(self.symbols)
        }

        async fn find_edges_by_caller(
            &self,
            _caller_id: &str,
        ) -> Result<Vec<EdgeMetadata>, CallGraphStoreError> {
            Ok(Vec::new())
        }

        async fn find_edges_by_callee(
            &self,
            _callee_id: &str,
        ) -> Result<Vec<EdgeMetadata>, CallGraphStoreError> {
            Ok(Vec::new())
        }

        async fn count_edges(&self) -> Result<usize, CallGraphStoreError> {
            Ok(self.edges)
        }

        async fn load_call_graph_pinned(
            &self,
            _workspace: &WorkspaceId,
            _revision: RevisionId,
        ) -> Result<Option<CallGraph>, CallGraphStoreError> {
            Ok(None)
        }
    }

    #[tokio::test]
    async fn empty_repo_returns_none_and_zero() {
        let repo = EmptyRepo;
        assert!(
            repo.find_symbol_by_qualified_name("anything")
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(repo.count_symbols().await.unwrap(), 0);
        assert!(repo.find_edges_by_caller("x").await.unwrap().is_empty());
        assert!(repo.find_edges_by_callee("x").await.unwrap().is_empty());
        assert_eq!(repo.count_edges().await.unwrap(), 0);
        assert!(
            repo.load_call_graph_pinned(&WorkspaceId::default(), RevisionId::NONE)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn counting_repo_reports_seeded_counts() {
        let repo = CountingRepo {
            symbols: 42,
            edges: 9,
        };
        assert_eq!(repo.count_symbols().await.unwrap(), 42);
        assert_eq!(repo.count_edges().await.unwrap(), 9);
    }

    #[tokio::test]
    async fn load_call_graph_pinned_returns_none_for_empty_repo() {
        let repo = EmptyRepo;
        let result = repo
            .load_call_graph_pinned(&WorkspaceId::default(), RevisionId::NONE)
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn trait_is_dyn_compatible_and_send_sync() {
        // This test would NOT compile if `Repository` lost its
        // `Send + Sync` bound or its `#[async_trait]` annotation.
        let boxed: Box<dyn CallGraphStore> = Box::new(CountingRepo {
            symbols: 7,
            edges: 3,
        });
        let _shared: Arc<dyn CallGraphStore> = Arc::new(EmptyRepo);
        assert_eq!(boxed.count_symbols().await.unwrap(), 7);
        assert_eq!(boxed.count_edges().await.unwrap(), 3);
        // load_call_graph_pinned on dyn box
        assert!(
            boxed
                .load_call_graph_pinned(&WorkspaceId::default(), RevisionId::NONE)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn repository_error_display_is_informative() {
        let err = CallGraphStoreError::NotFound("src/missing.rs".to_string());
        let msg = err.to_string();
        assert!(msg.contains("src/missing.rs"), "got: {msg}");
    }
}
