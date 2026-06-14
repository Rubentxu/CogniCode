//! PostgreSQL-backed [`ViewSpecStore`] implementation.
//!
//! Backed by the `view_specs` table (see the migration at
//! `migrations/20260612000001_view_specs.sql`). Each operation is
//! stateless — the `PostgresRepository` owns the connection pool.

#[cfg(feature = "postgres")]
use std::sync::Arc;

#[cfg(feature = "postgres")]
use async_trait::async_trait;

#[cfg(feature = "postgres")]
use cognicode_core::infrastructure::persistence::PostgresRepository;

#[cfg(feature = "postgres")]
use crate::dto::{DataSource, InspectableObjectType, RendererKind, Transform, ViewKind, ViewSpec};
#[cfg(feature = "postgres")]
use crate::registry::{ViewSpecStore, ViewSpecStoreError};

/// A `ViewSpecStore` backed by PostgreSQL.
///
/// Each method maps typed errors onto [`ViewSpecStoreError`] so callers
/// (the explorer service) never deal with `RepositoryError` directly.
#[cfg(feature = "postgres")]
pub struct PostgresViewSpecStore {
    repo: Arc<PostgresRepository>,
}

#[cfg(feature = "postgres")]
impl PostgresViewSpecStore {
    /// Construct a store from an existing `Arc<PostgresRepository>`.
    pub fn new(repo: Arc<PostgresRepository>) -> Self {
        Self { repo }
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl ViewSpecStore for PostgresViewSpecStore {
    async fn save(
        &self,
        spec: &ViewSpec,
        workspace_id: &str,
        owner: &str,
    ) -> Result<(), ViewSpecStoreError> {
        let data_source_json =
            serde_json::to_string(&spec.data_source).map_err(|e| ViewSpecStoreError::Store(e.to_string()))?;
        let transform_json = spec
            .transform
            .as_ref()
            .map(|t| serde_json::to_string(t))
            .transpose()
            .map_err(|e| ViewSpecStoreError::Store(e.to_string()))?;
        let props_json =
            serde_json::to_string(&spec.props).map_err(|e| ViewSpecStoreError::Store(e.to_string()))?;

        self.repo
            .save_view_spec(
                &spec.id,
                workspace_id,
                owner,
                &spec.title,
                &applies_to_to_string(&spec.applies_to),
                &view_kind_to_string(&spec.view_kind),
                &data_source_json,
                transform_json.as_deref(),
                &renderer_kind_to_string(&spec.renderer_kind),
                &props_json,
            )
            .await
            .map_err(|e| match e {
                cognicode_core::domain::traits::RepositoryError::UniqueViolation(msg) => {
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
    ) -> Result<Option<ViewSpec>, ViewSpecStoreError> {
        self.repo
            .load_view_spec(id, workspace_id, owner)
            .await
            .map_err(|e| ViewSpecStoreError::Store(e.to_string()))?
            .map(|row| view_spec_row_to_view_spec(row))
            .transpose()
            .map_err(|e| ViewSpecStoreError::Store(e.to_string()))
    }

    async fn list(
        &self,
        workspace_id: &str,
        owner: &str,
    ) -> Result<Vec<ViewSpec>, ViewSpecStoreError> {
        self.repo
            .list_view_specs(workspace_id, owner)
            .await
            .map_err(|e| ViewSpecStoreError::Store(e.to_string()))
            .map(|rows| {
                rows.into_iter()
                    .filter_map(|row| view_spec_row_to_view_spec(row).ok())
                    .collect()
            })
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
        applies_to: InspectableObjectType,
    ) -> Result<Vec<ViewSpec>, ViewSpecStoreError> {
        let applies_to_str = applies_to_to_string(&applies_to);
        self.repo
            .list_view_specs_for_workspace(workspace_id, &applies_to_str)
            .await
            .map_err(|e| ViewSpecStoreError::Store(e.to_string()))
            .map(|rows| {
                rows.into_iter()
                    .filter_map(|row| view_spec_row_to_view_spec(row).ok())
                    .collect()
            })
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

#[cfg(feature = "postgres")]
fn applies_to_to_string(applies_to: &InspectableObjectType) -> String {
    // Mirror the snake_case serde rename
    match applies_to {
        InspectableObjectType::Workspace => "workspace".to_string(),
        InspectableObjectType::Scope => "scope".to_string(),
        InspectableObjectType::Symbol => "symbol".to_string(),
        InspectableObjectType::File => "file".to_string(),
        InspectableObjectType::Module => "module".to_string(),
        InspectableObjectType::Evidence => "evidence".to_string(),
        InspectableObjectType::DecisionArtifact => "decision_artifact".to_string(),
        InspectableObjectType::QualityIssue => "quality_issue".to_string(),
        InspectableObjectType::Rule => "rule".to_string(),
    }
}

#[cfg(feature = "postgres")]
fn string_to_applies_to(s: &str) -> Option<InspectableObjectType> {
    match s {
        "workspace" => Some(InspectableObjectType::Workspace),
        "scope" => Some(InspectableObjectType::Scope),
        "symbol" => Some(InspectableObjectType::Symbol),
        "file" => Some(InspectableObjectType::File),
        "module" => Some(InspectableObjectType::Module),
        "evidence" => Some(InspectableObjectType::Evidence),
        "decision_artifact" => Some(InspectableObjectType::DecisionArtifact),
        "quality_issue" => Some(InspectableObjectType::QualityIssue),
        "rule" => Some(InspectableObjectType::Rule),
        _ => None,
    }
}

#[cfg(feature = "postgres")]
fn view_kind_to_string(kind: &ViewKind) -> String {
    // Use the serde serialization
    serde_json::to_string(kind)
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_else(|_| "custom".to_string())
}

#[cfg(feature = "postgres")]
fn renderer_kind_to_string(kind: &RendererKind) -> String {
    serde_json::to_string(kind)
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_else(|_| "json".to_string())
}

#[cfg(feature = "postgres")]
fn view_spec_row_to_view_spec(
    row: cognicode_core::infrastructure::persistence::ViewSpecRow,
) -> Result<ViewSpec, String> {
    let applies_to = string_to_applies_to(&row.applies_to)
        .ok_or_else(|| format!("unknown applies_to: {}", row.applies_to))?;
    let view_kind: ViewKind =
        serde_json::from_str(&row.view_kind).map_err(|e| format!("view_kind parse error: {e}"))?;
    let data_source: DataSource =
        serde_json::from_str(&row.data_source).map_err(|e| format!("data_source parse error: {e}"))?;
    let transform: Option<Transform> = row
        .transform
        .as_ref()
        .map(|t| serde_json::from_str(t))
        .transpose()
        .map_err(|e| format!("transform parse error: {e}"))?;
    let renderer_kind: RendererKind =
        serde_json::from_str(&row.renderer_kind).map_err(|e| format!("renderer_kind parse error: {e}"))?;
    let props: serde_json::Value =
        serde_json::from_str(&row.props).map_err(|e| format!("props parse error: {e}"))?;

    Ok(ViewSpec {
        id: row.id,
        title: row.title,
        applies_to,
        view_kind,
        data_source,
        transform,
        renderer_kind,
        props,
        created_at: row.created_at,
        updated_at: row.updated_at,
        owner: row.owner,
    })
}
