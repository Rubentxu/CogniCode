//! `ViewSpecStore` — domain port for the persisted view specifications.
//!
//! Surfaces the `view_specs` table (see migration
//! `migrations/20260612000001_view_specs.sql`) behind a `Send + Sync`
//! trait shape.
//!
//! # Phase 0 origin and DTO boundary
//!
//! This trait was relocated from `cognicode-explorer::registry` per
//! ADR-028 to make the port importable from `cognicode_core` only.
//!
//! The wire-format [`ViewSpecPayload`] struct is the *port-side* DTO
//! — it carries the JSON-serialized representations of the explorer
//! DTOs (`ViewSpec`, `DataSource`, `Transform`, etc.). The explorer
//! has `Into`/`From` impls between `ViewSpec` and `ViewSpecPayload`
//! so the adapter boundary crosses via serde, keeping the explorer DTOs
//! (and their ~1457 lines of variants + methods) in `cognicode-explorer`
//! where they belong.
//!
//! This preserves the original trait semantics (the existing
//! `PostgresViewSpecStore` adapter already serialized the rich fields
//! to JSON before persisting; the wire-format struct is the explicit
//! port type that replaces the implicit JSON round-trip).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Wire-format payload for [`ViewSpecStore`] operations.
///
/// Carries the JSON-serialized representations of the explorer-side
/// DTOs (`ViewSpec`, `DataSource`, `Transform`, `InspectableObjectType`,
/// `RendererKind`, `ViewKind`) plus a small set of stable scalars
/// (`id`, `title`, `owner`, `created_at`, `updated_at`, etc.).
///
/// Adapter implementations are free to interpret these JSON values
/// any way they choose; the intent is to preserve the public surface
/// of the explorer DTOs without coupling the port to those types
/// directly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewSpecPayload {
    pub id: String,
    pub title: String,
    /// The object kind this view applies to (snake_case wire form).
    pub applies_to: String,
    /// The semantic view intent (snake_case wire form of the enum tag).
    pub view_kind: serde_json::Value,
    /// JSON-serialized `DataSource` enum payload.
    pub data_source: serde_json::Value,
    /// Optional JSON-serialized `Transform` enum payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<serde_json::Value>,
    /// The visual renderer selector (e.g. `"graph"`, `"table"`).
    pub renderer_kind: String,
    /// Opaque props object (the authoring tool's own bag).
    pub props: serde_json::Value,
    /// ISO-8601 UTC; server-assigned on insert.
    #[serde(default)]
    pub created_at: String,
    /// ISO-8601 UTC; server-assigned on update.
    #[serde(default)]
    pub updated_at: String,
    /// The owner of this view spec.
    #[serde(default)]
    pub owner: String,
    /// Origin seed object id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_object_id: Option<String>,
    /// Origin seed view id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_view_id: Option<String>,
    /// Optional MoldQL predicate narrowing when this spec applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applies_when: Option<String>,
}

/// Errors returned by [`ViewSpecStore`] operations.
#[derive(Debug, Clone)]
pub enum ViewSpecStoreError {
    /// The operation failed due to a storage error.
    Store(String),
    /// A row with the same `(workspace_id, owner, title)` already exists.
    Conflict(String),
    /// The requested view spec was not found.
    NotFound(String),
}

impl std::fmt::Display for ViewSpecStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(msg) => write!(f, "view_spec store error: {msg}"),
            Self::Conflict(msg) => write!(f, "view_spec conflict: {msg}"),
            Self::NotFound(msg) => write!(f, "view_spec not found: {msg}"),
        }
    }
}

impl std::error::Error for ViewSpecStoreError {}

/// Domain port for persisted view specifications.
///
/// The trait takes [`ViewSpecPayload`] (a JSON-friendly wire-format
/// struct) on the way in and returns it on the way out — this keeps
/// the port decoupled from the explorer's rich DTO types while still
/// preserving type safety inside `cognicode-core`.
#[async_trait]
pub trait ViewSpecStore: Send + Sync + 'static {
    /// Persist a view spec. The `id` is client-provided; the store
    /// must return [`ViewSpecStoreError::Conflict`] when a row with the
    /// same `(workspace_id, owner, title)` already exists (idempotent
    /// save is the caller's responsibility).
    async fn save(
        &self,
        payload: &ViewSpecPayload,
        workspace_id: &str,
        owner: &str,
    ) -> Result<(), ViewSpecStoreError>;

    /// Load a single view spec by id, scoped to `(workspace_id, owner)`.
    /// Returns `Ok(None)` when no matching row exists.
    async fn load(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> Result<Option<ViewSpecPayload>, ViewSpecStoreError>;

    /// List every view spec for `(workspace_id, owner)`, ordered by
    /// `created_at DESC` (newest first). Returns `Ok(vec![])` for an
    /// empty scope — NOT an error.
    async fn list(
        &self,
        workspace_id: &str,
        owner: &str,
    ) -> Result<Vec<ViewSpecPayload>, ViewSpecStoreError>;

    /// Delete a view spec by id, scoped to `(workspace_id, owner)`.
    /// Returns `Ok(true)` if a row was deleted, `Ok(false)` if no
    /// matching row existed.
    async fn delete(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> Result<bool, ViewSpecStoreError>;

    /// List every view spec for `workspace_id` with the given
    /// `applies_to_kind` (the snake_case wire form of the
    /// `InspectableObjectType` enum), across ALL owners.
    /// Returns `Ok(vec![])` for an empty scope — NOT an error.
    async fn list_for_workspace(
        &self,
        workspace_id: &str,
        applies_to_kind: &str,
    ) -> Result<Vec<ViewSpecPayload>, ViewSpecStoreError>;

    /// Update a view spec's provenance fields (seed_object_id, seed_view_id,
    /// applies_when) in-place without touching other columns.
    /// Returns `Ok(true)` if a row was updated, `Ok(false)` if no matching
    /// row existed.
    async fn update(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
        seed_object_id: Option<&str>,
        seed_view_id: Option<&str>,
        applies_when: Option<&str>,
    ) -> Result<bool, ViewSpecStoreError>;
}

// =============================================================================
// PostgresViewSpecStore adapter
// =============================================================================

#[cfg(feature = "postgres")]
mod postgres_adapter {
    use std::sync::Arc;

    use async_trait::async_trait;

    use crate::infrastructure::persistence::PostgresRepository;
    use crate::infrastructure::persistence::postgres_repository::ViewSpecRow;

    use super::{ViewSpecPayload, ViewSpecStore, ViewSpecStoreError};

    /// `PostgresViewSpecStore` — [`ViewSpecStore`] adapter backed by
    /// PostgreSQL via the existing `view_specs` table persistence layer.
    ///
    /// Each operation is stateless — the pool is owned by the parent
    /// `PostgresRepository`.
    pub struct PostgresViewSpecStore {
        repo: Arc<PostgresRepository>,
    }

    impl PostgresViewSpecStore {
        /// Construct from an existing `Arc<PostgresRepository>`.
        pub fn new(repo: Arc<PostgresRepository>) -> Self {
            Self { repo }
        }
    }

    fn row_to_payload(row: ViewSpecRow) -> Result<ViewSpecPayload, ViewSpecStoreError> {
        let data_source: serde_json::Value = serde_json::from_str(&row.data_source)
            .map_err(|e| ViewSpecStoreError::Store(format!("data_source parse error: {e}")))?;
        let transform: Option<serde_json::Value> =
            match row.transform.as_ref() {
                Some(s) => Some(serde_json::from_str(s).map_err(|e| {
                    ViewSpecStoreError::Store(format!("transform parse error: {e}"))
                })?),
                None => None,
            };
        let view_kind: serde_json::Value = serde_json::from_str(&row.view_kind)
            .map_err(|e| ViewSpecStoreError::Store(format!("view_kind parse error: {e}")))?;
        let props: serde_json::Value = serde_json::from_str(&row.props)
            .map_err(|e| ViewSpecStoreError::Store(format!("props parse error: {e}")))?;

        Ok(ViewSpecPayload {
            id: row.id,
            title: row.title,
            applies_to: row.applies_to,
            view_kind,
            data_source,
            transform,
            renderer_kind: row.renderer_kind,
            props,
            created_at: row.created_at,
            updated_at: row.updated_at,
            owner: row.owner,
            seed_object_id: row.seed_object_id,
            seed_view_id: row.seed_view_id,
            applies_when: row.applies_when,
        })
    }

    #[async_trait]
    impl ViewSpecStore for PostgresViewSpecStore {
        async fn save(
            &self,
            payload: &ViewSpecPayload,
            workspace_id: &str,
            owner: &str,
        ) -> Result<(), ViewSpecStoreError> {
            let data_source_json = serde_json::to_string(&payload.data_source)
                .map_err(|e| ViewSpecStoreError::Store(e.to_string()))?;
            let transform_json = payload
                .transform
                .as_ref()
                .map(|t| serde_json::to_string(t))
                .transpose()
                .map_err(|e| ViewSpecStoreError::Store(e.to_string()))?;
            let props_json = serde_json::to_string(&payload.props)
                .map_err(|e| ViewSpecStoreError::Store(e.to_string()))?;
            let view_kind_json = serde_json::to_string(&payload.view_kind)
                .map_err(|e| ViewSpecStoreError::Store(e.to_string()))?;

            self.repo
                .save_view_spec(
                    &payload.id,
                    workspace_id,
                    owner,
                    &payload.title,
                    &payload.applies_to,
                    &view_kind_json,
                    &data_source_json,
                    transform_json.as_deref(),
                    &payload.renderer_kind,
                    &props_json,
                    payload.seed_object_id.as_deref(),
                    payload.seed_view_id.as_deref(),
                    payload.applies_when.as_deref(),
                )
                .await
                .map_err(|e| match e {
                    crate::domain::traits::CallGraphStoreError::UniqueViolation(msg) => {
                        ViewSpecStoreError::Conflict(msg)
                    }
                    other => ViewSpecStoreError::Store(other.to_string()),
                })
        }

        async fn load(
            &self,
            id: &str,
            workspace_id: &str,
            owner: &str,
        ) -> Result<Option<ViewSpecPayload>, ViewSpecStoreError> {
            let opt = self
                .repo
                .load_view_spec(id, workspace_id, owner)
                .await
                .map_err(|e| ViewSpecStoreError::Store(e.to_string()))?;
            match opt {
                None => Ok(None),
                Some(row) => Ok(Some(row_to_payload(row)?)),
            }
        }

        async fn list(
            &self,
            workspace_id: &str,
            owner: &str,
        ) -> Result<Vec<ViewSpecPayload>, ViewSpecStoreError> {
            let rows = self
                .repo
                .list_view_specs(workspace_id, owner)
                .await
                .map_err(|e| ViewSpecStoreError::Store(e.to_string()))?;
            rows.into_iter()
                .map(|r| row_to_payload(r))
                .collect::<Result<Vec<_>, _>>()
        }

        async fn delete(
            &self,
            id: &str,
            workspace_id: &str,
            owner: &str,
        ) -> Result<bool, ViewSpecStoreError> {
            self.repo
                .delete_view_spec(id, workspace_id, owner)
                .await
                .map_err(|e| ViewSpecStoreError::Store(e.to_string()))
        }

        async fn list_for_workspace(
            &self,
            workspace_id: &str,
            applies_to_kind: &str,
        ) -> Result<Vec<ViewSpecPayload>, ViewSpecStoreError> {
            let rows = self
                .repo
                .list_view_specs_for_workspace(workspace_id, applies_to_kind)
                .await
                .map_err(|e| ViewSpecStoreError::Store(e.to_string()))?;
            rows.into_iter()
                .map(|r| row_to_payload(r))
                .collect::<Result<Vec<_>, _>>()
        }

        async fn update(
            &self,
            id: &str,
            workspace_id: &str,
            owner: &str,
            seed_object_id: Option<&str>,
            seed_view_id: Option<&str>,
            applies_when: Option<&str>,
        ) -> Result<bool, ViewSpecStoreError> {
            self.repo
                .update_view_spec(
                    id,
                    workspace_id,
                    owner,
                    seed_object_id,
                    seed_view_id,
                    applies_when,
                )
                .await
                .map_err(|e| ViewSpecStoreError::Store(e.to_string()))
        }
    }
}

#[cfg(feature = "postgres")]
pub use postgres_adapter::PostgresViewSpecStore;
