//! Domain port for the `spaces` table.
//!
//! `spaces` is the federation unit: one row per registered space
//! (Repo / Docs / Issues). The table is seeded with a "default" space
//! on first migration so every pre-federation node has a valid `space_id`
//! to back-reference.
//!
//! This port exists so the federation layer can manage spaces without
//! depending on the `PostgresRepository` concrete type.

use async_trait::async_trait;

use crate::domain::value_objects::{Space, SpaceId};

/// Port for federation space persistence.
#[async_trait]
pub trait FederationStore: Send + Sync {
    /// Register a new space, returning its assigned [`SpaceId`].
    async fn register_space(&self, space: &Space) -> Result<SpaceId, FederationError>;

    /// List every registered space, ordered by `created_at DESC`.
    async fn list_spaces(&self) -> Result<Vec<Space>, FederationError>;

    /// Load a single space by [`SpaceId`], if it exists.
    async fn get_space(&self, id: &SpaceId) -> Result<Option<Space>, FederationError>;
}

/// Error type for [`FederationStore`] operations.
#[derive(Debug, thiserror::Error)]
pub enum FederationError {
    #[error("federation store error: {0}")]
    Store(String),

    #[error("space already exists: {0}")]
    Conflict(SpaceId),
}

#[cfg(feature = "postgres")]
pub mod postgres_adapter {
    use super::{FederationError, FederationStore, Space, SpaceId};
    use async_trait::async_trait;
    use sqlx::PgPool;

    /// Row type mirroring the `spaces` table columns surfaced by this port.
    #[derive(Debug, Clone, sqlx::FromRow)]
    pub struct SpaceRow {
        pub id: String,
        pub name: String,
        pub kind: String,
        pub source_path: Option<String>,
        pub config: serde_json::Value,
        pub created_at: chrono::DateTime<chrono::Utc>,
    }

    impl From<SpaceRow> for Space {
        fn from(r: SpaceRow) -> Self {
            use crate::domain::value_objects::SpaceKind;
            let kind = match r.kind.as_str() {
                "Repo" => SpaceKind::Repo,
                "Docs" => SpaceKind::Docs,
                "Issues" => SpaceKind::Issues,
                other => SpaceKind::Repo, // fallback for unknown; DB CHECK constrains to these 3
            };
            Space {
                id: SpaceId(r.id),
                name: r.name,
                kind,
                source_path: r.source_path,
                config: r.config,
            }
        }
    }

    /// Adapter that delegates every [`FederationStore`] method to raw SQL
    /// executed against a shared `PgPool`.
    #[cfg(feature = "postgres")]
    pub struct PostgresFederationStore {
        pool: PgPool,
    }

    #[cfg(feature = "postgres")]
    impl PostgresFederationStore {
        /// Build the adapter from a shared `PgPool`.
        pub fn new(pool: PgPool) -> Self {
            Self { pool }
        }
    }

    #[cfg(feature = "postgres")]
    #[async_trait]
    impl FederationStore for PostgresFederationStore {
        async fn register_space(&self, space: &Space) -> Result<SpaceId, FederationError> {
            let kind_str = space.kind.as_str();
            let source_path = space.source_path.as_ref().map(|p| p.to_string_lossy().to_string());

            sqlx::query(
                "INSERT INTO spaces (id, name, kind, source_path, config) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name",
            )
            .bind(space.id.as_str())
            .bind(&space.name)
            .bind(kind_str)
            .bind(&source_path)
            .bind(&space.config)
            .execute(&*self.pool)
            .await
            .map_err(|e| FederationError::Store(format!("register_space: {e}")))?;

            Ok(space.id.clone())
        }

        async fn list_spaces(&self) -> Result<Vec<Space>, FederationError> {
            let rows: Vec<SpaceRow> = sqlx::query_as(
                "SELECT id, name, kind, source_path, config, created_at \
                 FROM spaces \
                 ORDER BY created_at DESC",
            )
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| FederationError::Store(format!("list_spaces: {e}")))?;

            Ok(rows.into_iter().map(Space::from).collect())
        }

        async fn get_space(&self, id: &SpaceId) -> Result<Option<Space>, FederationError> {
            let row: Option<SpaceRow> = sqlx::query_as(
                "SELECT id, name, kind, source_path, config, created_at \
                 FROM spaces \
                 WHERE id = $1",
            )
            .bind(id.as_str())
            .fetch_optional(&*self.pool)
            .await
            .map_err(|e| FederationError::Store(format!("get_space: {e}")))?;

            Ok(row.map(Space::from))
        }
    }
}

#[cfg(feature = "postgres")]
pub use postgres_adapter::PostgresFederationStore;
