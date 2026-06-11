//! Async-ready canonical Repository port for the cognicode domain.
//!
//! This trait is the structural seam for the PostgreSQL-backed
//! implementation that lands in a follow-up slice. It is intentionally
//! **standalone** — it does NOT inherit from [`GraphStore`] — so the
//! write-path (synchronous save/load of bincode blobs) and the
//! read-path (async, query-shaped) remain independent seams. A future
//! PostgreSQL struct can implement both traits side by side.
//!
//! The trait is `Send + Sync` and uses `#[async_trait]` so it remains
//! dyn-compatible (e.g. `Box<dyn Repository>`) for application code
//! that wants to swap implementations at runtime.

use async_trait::async_trait;
use thiserror::Error;

use crate::domain::aggregates::Symbol;
use crate::domain::value_objects::EdgeMetadata;

/// Error type for [`Repository`] operations.
///
/// Distinct from [`crate::domain::traits::graph_store::StoreError`]
/// (which models the synchronous blob persistence path) so that async
/// query failures can carry query-specific context without polluting
/// the persistence surface.
#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("store error: {0}")]
    Store(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid query: {0}")]
    InvalidQuery(String),

    /// PostgreSQL unique-violation (`SQLSTATE 23505`). Raised by
    /// `save_named_view` when a `(workspace_id, owner, name)` triple
    /// already exists.
    #[error("unique violation: {0}")]
    UniqueViolation(String),
}

/// Async-ready read-side port over the canonical symbol/edge store.
///
/// Implementations are expected to be `Send + Sync`. The trait
/// starts with symbol queries and grows with edge queries (see
/// `explorer-graph-postgres-call-edges`). The seam is additive: every
/// previous method signature stays stable across minor versions.
#[async_trait]
pub trait Repository: Send + Sync {
    /// Look up a symbol by its fully-qualified name (the canonical
    /// `SymbolId` form: `file:name:line`). Returns `None` when no
    /// symbol matches.
    async fn find_symbol_by_qualified_name(
        &self,
        name: &str,
    ) -> Result<Option<Symbol>, RepositoryError>;

    /// Count every indexed symbol. Cheap call — implementations are
    /// expected to delegate to a precomputed count when available
    /// (e.g. PostgreSQL `pg_stat_user_tables` / a materialized view).
    async fn count_symbols(&self) -> Result<usize, RepositoryError>;

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
    ) -> Result<Vec<EdgeMetadata>, RepositoryError>;

    /// Return every call-graph edge whose `callee_id` matches
    /// `callee_id`. Same empty-result contract as
    /// [`Repository::find_edges_by_caller`].
    async fn find_edges_by_callee(
        &self,
        callee_id: &str,
    ) -> Result<Vec<EdgeMetadata>, RepositoryError>;

    /// Count every indexed call-graph edge. Cheap call — delegates
    /// to `SELECT COUNT(*)` on the `call_edges` table.
    async fn count_edges(&self) -> Result<usize, RepositoryError>;
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
    impl Repository for EmptyRepo {
        async fn find_symbol_by_qualified_name(
            &self,
            _name: &str,
        ) -> Result<Option<Symbol>, RepositoryError> {
            Ok(None)
        }

        async fn count_symbols(&self) -> Result<usize, RepositoryError> {
            Ok(0)
        }

        async fn find_edges_by_caller(
            &self,
            _caller_id: &str,
        ) -> Result<Vec<EdgeMetadata>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn find_edges_by_callee(
            &self,
            _callee_id: &str,
        ) -> Result<Vec<EdgeMetadata>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn count_edges(&self) -> Result<usize, RepositoryError> {
            Ok(0)
        }
    }

    /// A second implementation to prove the trait is open to
    /// additional implementors in the same compilation unit.
    struct CountingRepo {
        symbols: usize,
        edges: usize,
    }

    #[async_trait]
    impl Repository for CountingRepo {
        async fn find_symbol_by_qualified_name(
            &self,
            _name: &str,
        ) -> Result<Option<Symbol>, RepositoryError> {
            Ok(None)
        }

        async fn count_symbols(&self) -> Result<usize, RepositoryError> {
            Ok(self.symbols)
        }

        async fn find_edges_by_caller(
            &self,
            _caller_id: &str,
        ) -> Result<Vec<EdgeMetadata>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn find_edges_by_callee(
            &self,
            _callee_id: &str,
        ) -> Result<Vec<EdgeMetadata>, RepositoryError> {
            Ok(Vec::new())
        }

        async fn count_edges(&self) -> Result<usize, RepositoryError> {
            Ok(self.edges)
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
    async fn trait_is_dyn_compatible_and_send_sync() {
        // This test would NOT compile if `Repository` lost its
        // `Send + Sync` bound or its `#[async_trait]` annotation.
        let boxed: Box<dyn Repository> = Box::new(CountingRepo {
            symbols: 7,
            edges: 3,
        });
        let _shared: Arc<dyn Repository> = Arc::new(EmptyRepo);
        assert_eq!(boxed.count_symbols().await.unwrap(), 7);
        assert_eq!(boxed.count_edges().await.unwrap(), 3);
    }

    #[test]
    fn repository_error_display_is_informative() {
        let err = RepositoryError::NotFound("src/missing.rs".to_string());
        let msg = err.to_string();
        assert!(msg.contains("src/missing.rs"), "got: {msg}");
    }
}
