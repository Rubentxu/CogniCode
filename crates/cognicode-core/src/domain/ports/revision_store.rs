//! Domain port for the `graph_revisions` table.
//!
//! `graph_revisions` tracks the workspace-scoped revision timeline.
//! Each workspace has a monotonically-increasing integer revision counter,
//! and exactly one row has `head_of = true` at any time (the "current"
//! revision). Opening a new revision is atomic: demote the old head,
//! insert the new row, and bump the counter.
//!
//! This port exists so `save_call_graph_ws` can open revisions without
//! depending on the `PostgresRepository` concrete type directly.

use async_trait::async_trait;

use crate::domain::value_objects::{RevisionId, WorkspaceId};

/// Port for revision lifecycle management.
#[async_trait]
pub trait RevisionStore: Send + Sync {
    /// Open a new revision for `workspace_id`, atomically demoting the
    /// prior head (if any) and inserting the new row with `head_of = true`.
    ///
    /// Returns the newly-opened [`RevisionId`].
    async fn create_revision(
        &self,
        conn: &mut sqlx::PgConnection,
        ws: &WorkspaceId,
    ) -> Result<RevisionId, RevisionError>;

    /// Promote an existing revision to be the new head of the workspace.
    /// Demotes any prior head atomically within the caller's transaction.
    async fn set_head(
        &self,
        conn: &mut sqlx::PgConnection,
        ws: &WorkspaceId,
        rev: RevisionId,
    ) -> Result<(), RevisionError>;

    /// Return the current head revision id for a workspace, if any.
    async fn head_revision(
        &self,
        conn: &mut sqlx::PgConnection,
        ws: &WorkspaceId,
    ) -> Result<Option<RevisionId>, RevisionError>;
}

/// Error type for [`RevisionStore`] operations.
#[derive(Debug, thiserror::Error)]
pub enum RevisionError {
    #[error("revision store error: {0}")]
    Store(String),

    #[error("revision not found for workspace {0}")]
    NotFound(WorkspaceId),
}

#[cfg(feature = "postgres")]
pub mod postgres_adapter {
    use super::{RevisionError, RevisionId, RevisionStore, WorkspaceId};
    use async_trait::async_trait;

    /// Adapter that delegates every [`RevisionStore`] method to raw SQL
    /// executed on a shared `PgPool`.
    #[cfg(feature = "postgres")]
    pub struct PostgresRevisionStore {
        #[allow(dead_code)]
        pool: sqlx::PgPool,
    }

    #[cfg(feature = "postgres")]
    impl PostgresRevisionStore {
        /// Build the adapter from a shared `PgPool`.
        pub fn new(pool: sqlx::PgPool) -> Self {
            Self { pool }
        }
    }

    #[cfg(feature = "postgres")]
    #[async_trait]
    impl RevisionStore for PostgresRevisionStore {
        async fn create_revision(
            &self,
            conn: &mut sqlx::PgConnection,
            ws: &WorkspaceId,
        ) -> Result<RevisionId, RevisionError> {
            // Demote the existing head (if any) to `head_of = false`.
            sqlx::query(
                "UPDATE graph_revisions \
                 SET head_of = false \
                 WHERE workspace_id = $1 AND head_of = true",
            )
            .bind(ws.as_str())
            .execute(&mut *conn)
            .await
            .map_err(|e| RevisionError::Store(format!("create_revision demote head: {e}")))?;

            // Compute MAX(revision_id) + 1 for this workspace.
            // COALESCE handles the "no rows yet" case → starts at 1.
            let next_rev: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(revision_id), 0) + 1 \
                 FROM graph_revisions \
                 WHERE workspace_id = $1",
            )
            .bind(ws.as_str())
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| {
                RevisionError::Store(format!("create_revision compute next revision: {e}"))
            })?;

            // Insert the new head row.
            sqlx::query(
                "INSERT INTO graph_revisions (workspace_id, revision_id, head_of) \
                 VALUES ($1, $2, true)",
            )
            .bind(ws.as_str())
            .bind(next_rev)
            .execute(&mut *conn)
            .await
            .map_err(|e| RevisionError::Store(format!("create_revision insert revision: {e}")))?;

            Ok(RevisionId(next_rev as u64))
        }

        async fn set_head(
            &self,
            conn: &mut sqlx::PgConnection,
            ws: &WorkspaceId,
            rev: RevisionId,
        ) -> Result<(), RevisionError> {
            // Demote the existing head (if any).
            sqlx::query(
                "UPDATE graph_revisions \
                 SET head_of = false \
                 WHERE workspace_id = $1 AND head_of = true",
            )
            .bind(ws.as_str())
            .execute(&mut *conn)
            .await
            .map_err(|e| RevisionError::Store(format!("set_head demote: {e}")))?;

            // Promote the target revision to head.
            sqlx::query(
                "UPDATE graph_revisions \
                 SET head_of = true \
                 WHERE workspace_id = $1 AND revision_id = $2",
            )
            .bind(ws.as_str())
            .bind(rev.get() as i64)
            .execute(&mut *conn)
            .await
            .map_err(|e| RevisionError::Store(format!("set_head promote: {e}")))?;

            Ok(())
        }

        async fn head_revision(
            &self,
            conn: &mut sqlx::PgConnection,
            ws: &WorkspaceId,
        ) -> Result<Option<RevisionId>, RevisionError> {
            let row: Option<(i64,)> = sqlx::query_as(
                "SELECT revision_id \
                 FROM graph_revisions \
                 WHERE workspace_id = $1 AND head_of = true",
            )
            .bind(ws.as_str())
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| RevisionError::Store(format!("head_revision query: {e}")))?;

            Ok(row.map(|(r,)| RevisionId(r as u64)))
        }
    }
}

#[cfg(feature = "postgres")]
pub use postgres_adapter::PostgresRevisionStore;
