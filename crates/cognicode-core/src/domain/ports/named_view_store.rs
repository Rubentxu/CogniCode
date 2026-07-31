//! Domain port for the `named_views` table.
//!
//! `named_views` stores saved graph projection tuples (level, lens,
//! focus_node, max_depth) plus user-facing metadata. The four-tuple
//! is the projection that `view_load` re-invokes through the explorer
//! service.
//!
//! This port exists so application code can CRUD named views without
//! depending on `PostgresRepository` concrete types.

use async_trait::async_trait;

/// Domain type returned by [`NamedViewStore::load`] and [`NamedViewStore::list`].
///
/// Mirrors the columns of the `named_views` table but lives in the
/// domain layer. The Postgres adapter performs the row-to-domain
/// translation internally; callers never see the row type.
#[derive(Debug, Clone)]
pub struct NamedView {
    pub id: String,
    pub workspace_id: String,
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub level: String,
    pub lens: String,
    pub focus_node: String,
    pub max_depth: i32,
    pub created_at: String,
}

/// Port for named view persistence.
#[async_trait]
pub trait NamedViewStore: Send + Sync {
    /// Persist a single named view. The `id` is a server-generated
    /// UUID string; `created_at` is filled by the store.
    ///
    /// Returns `Err(NamedViewError::Conflict)` when a row with the same
    /// `(workspace_id, owner, name)` already exists.
    async fn save(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
        name: &str,
        description: Option<&str>,
        level: &str,
        lens: &str,
        focus_node: &str,
        max_depth: i32,
    ) -> Result<(), NamedViewError>;

    /// Look up a single named view by id, scoped to
    /// `(workspace_id, owner)`.
    ///
    /// Returns `Ok(None)` when the id is missing or the scope does
    /// not match.
    async fn load(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> Result<Option<NamedView>, NamedViewError>;

    /// List every named view for `(workspace_id, owner)`, newest-first.
    /// Returns an empty `Vec` for an empty scope (NOT an error).
    async fn list(
        &self,
        workspace_id: &str,
        owner: &str,
    ) -> Result<Vec<NamedView>, NamedViewError>;

    /// Delete a single named view, scoped to `(workspace_id, owner)`.
    ///
    /// Returns `Ok(true)` if a row was removed, `Ok(false)` otherwise.
    async fn delete(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> Result<bool, NamedViewError>;
}

/// Error type for [`NamedViewStore`] operations.
#[derive(Debug, thiserror::Error)]
pub enum NamedViewError {
    #[error("named_view already exists: ({0}, {1}, {2})")]
    Conflict(String, String, String),
    #[error("named_view store error: {0}")]
    Store(String),
}

#[cfg(feature = "postgres")]
mod postgres_adapter {
    use super::{NamedView, NamedViewError, NamedViewStore};
    use crate::domain::traits::repository::CallGraphStoreError;
    use crate::infrastructure::persistence::{NamedViewRow, PostgresRepository};
    use async_trait::async_trait;
    use std::sync::Arc;

    /// Translate the Postgres row type into the domain type. Keeps the
    /// adapter the only place that knows about [`NamedViewRow`].
    impl From<NamedViewRow> for NamedView {
        fn from(r: NamedViewRow) -> Self {
            Self {
                id: r.id,
                workspace_id: r.workspace_id,
                owner: r.owner,
                name: r.name,
                description: r.description,
                level: r.level,
                lens: r.lens,
                focus_node: r.focus_node,
                max_depth: r.max_depth,
                created_at: r.created_at,
            }
        }
    }

    /// Adapter that delegates every [`NamedViewStore`] method to a
    /// [`PostgresRepository`].
    #[cfg(feature = "postgres")]
    pub struct PostgresNamedViewStore {
        repo: Arc<PostgresRepository>,
    }

    #[cfg(feature = "postgres")]
    impl PostgresNamedViewStore {
        pub fn new(repo: Arc<PostgresRepository>) -> Self {
            Self { repo }
        }
    }

    #[cfg(feature = "postgres")]
    #[async_trait]
    impl NamedViewStore for PostgresNamedViewStore {
        async fn save(
            &self,
            id: &str,
            workspace_id: &str,
            owner: &str,
            name: &str,
            description: Option<&str>,
            level: &str,
            lens: &str,
            focus_node: &str,
            max_depth: i32,
        ) -> Result<(), NamedViewError> {
            self.repo
                .save_named_view(
                    id,
                    workspace_id,
                    owner,
                    name,
                    description,
                    level,
                    lens,
                    focus_node,
                    max_depth,
                )
                .await
                .map_err(|e| match e {
                    CallGraphStoreError::UniqueViolation(_) => {
                        NamedViewError::Conflict(
                            workspace_id.to_string(),
                            owner.to_string(),
                            name.to_string(),
                        )
                    }
                    other => NamedViewError::Store(other.to_string()),
                })
        }

        async fn load(
            &self,
            id: &str,
            workspace_id: &str,
            owner: &str,
        ) -> Result<Option<NamedView>, NamedViewError> {
            self.repo
                .load_named_view(id, workspace_id, owner)
                .await
                .map(|opt| opt.map(NamedView::from))
                .map_err(|e| NamedViewError::Store(e.to_string()))
        }

        async fn list(
            &self,
            workspace_id: &str,
            owner: &str,
        ) -> Result<Vec<NamedView>, NamedViewError> {
            self.repo
                .list_named_views(workspace_id, owner)
                .await
                .map(|rows| rows.into_iter().map(NamedView::from).collect())
                .map_err(|e| NamedViewError::Store(e.to_string()))
        }

        async fn delete(
            &self,
            id: &str,
            workspace_id: &str,
            owner: &str,
        ) -> Result<bool, NamedViewError> {
            self.repo
                .delete_named_view(id, workspace_id, owner)
                .await
                .map_err(|e| NamedViewError::Store(e.to_string()))
        }
    }
}

#[cfg(feature = "postgres")]
pub use postgres_adapter::PostgresNamedViewStore;