//! Domain port for the `exploration_sessions` table.
//!
//! `exploration_sessions` stores per-session navigation history
//! (events, panes, navigation mode) plus an optional link to an
//! active investigation (ADR-005 INV-1).
//!
//! This port exists so application code (the explorer persistence
//! facade) can CRUD sessions without depending on `PostgresRepository`
//! concrete types.

use async_trait::async_trait;

/// Port for exploration session persistence.
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Persist a single exploration session. The `id` is client-provided
    /// and `created_at` is filled by the store.
    ///
    /// `events` and `panes` are stored as JSONB.
    async fn save(
        &self,
        id: &str,
        workspace_id: &str,
        events_json: &str,
        navigation_mode: &str,
        panes_json: &str,
        investigation_id: Option<&str>,
    ) -> Result<(), SessionError>;

    /// Load a single session by id, scoped to `workspace_id`.
    ///
    /// Returns `Ok(None)` when the id is missing or the scope does
    /// not match.
    async fn load(&self, id: &str, workspace_id: &str) -> Result<Option<SessionRow>, SessionError>;

    /// List every session for a workspace, ordered by `created_at DESC`.
    /// Returns an empty `Vec` for an empty scope (NOT an error).
    async fn list(&self, workspace_id: &str) -> Result<Vec<SessionRow>, SessionError>;
}

/// Subset of `exploration_sessions` columns surfaced to the port.
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: String,
    pub workspace_id: String,
    pub events: serde_json::Value,
    pub navigation_mode: String,
    pub panes: serde_json::Value,
    pub created_at: String,
    pub investigation_id: Option<String>,
}

/// Error type for [`SessionStore`] operations.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session store error: {0}")]
    Store(String),
}

#[cfg(feature = "postgres")]
mod postgres_adapter {
    use super::{SessionError, SessionRow, SessionStore};
    use crate::infrastructure::persistence::PostgresRepository;
    use async_trait::async_trait;
    use std::sync::Arc;

    /// Adapter that delegates every [`SessionStore`] method to a
    /// [`PostgresRepository`].
    #[cfg(feature = "postgres")]
    pub struct PostgresSessionStore {
        repo: Arc<PostgresRepository>,
    }

    #[cfg(feature = "postgres")]
    impl PostgresSessionStore {
        pub fn new(repo: Arc<PostgresRepository>) -> Self {
            Self { repo }
        }
    }

    #[cfg(feature = "postgres")]
    #[async_trait]
    impl SessionStore for PostgresSessionStore {
        async fn save(
            &self,
            id: &str,
            workspace_id: &str,
            events_json: &str,
            navigation_mode: &str,
            panes_json: &str,
            investigation_id: Option<&str>,
        ) -> Result<(), SessionError> {
            self.repo
                .save_exploration_session(
                    id,
                    workspace_id,
                    events_json,
                    navigation_mode,
                    panes_json,
                    investigation_id,
                )
                .await
                .map_err(|e| SessionError::Store(e.to_string()))
        }

        async fn load(
            &self,
            id: &str,
            workspace_id: &str,
        ) -> Result<Option<SessionRow>, SessionError> {
            self.repo
                .load_exploration_session(id, workspace_id)
                .await
                .map(|opt| {
                    opt.map(|r| SessionRow {
                        id: r.id,
                        workspace_id: r.workspace_id,
                        events: r.events,
                        navigation_mode: r.navigation_mode,
                        panes: r.panes,
                        created_at: r.created_at,
                        investigation_id: r.investigation_id,
                    })
                })
                .map_err(|e| SessionError::Store(e.to_string()))
        }

        async fn list(&self, workspace_id: &str) -> Result<Vec<SessionRow>, SessionError> {
            self.repo
                .list_exploration_sessions(workspace_id)
                .await
                .map(|rows| {
                    rows.into_iter()
                        .map(|r| SessionRow {
                            id: r.id,
                            workspace_id: r.workspace_id,
                            events: r.events,
                            navigation_mode: r.navigation_mode,
                            panes: r.panes,
                            created_at: r.created_at,
                            investigation_id: r.investigation_id,
                        })
                        .collect()
                })
                .map_err(|e| SessionError::Store(e.to_string()))
        }
    }
}

#[cfg(feature = "postgres")]
pub use postgres_adapter::PostgresSessionStore;
