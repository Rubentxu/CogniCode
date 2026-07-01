//! Investigation facade — ADR-005 Phase INV-1.
//!
//! Provides a typed facade for investigation operations over the HTTP API.

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{ExplorerError, ExplorerResult};

pub use cognicode_core::domain::investigation::Evidence;

// Re-export the domain types from cognicode-core.
pub use cognicode_core::domain::investigation::Investigation;
pub use cognicode_core::domain::investigation::Status as InvestigationStatus;

/// Request to create a new investigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateInvestigationRequest {
    pub workspace_id: String,
    pub title: String,
    pub goal: String,
}

/// Request to update an investigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateInvestigationRequest {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub goal: String,
    pub status: InvestigationStatus,
    pub entry_point: Option<String>,
    pub panes: Vec<cognicode_core::domain::investigation::PaneSnapshot>,
    pub evidence: Vec<cognicode_core::domain::investigation::Evidence>,
    pub artifacts: Vec<cognicode_core::domain::investigation::Artifact>,
    pub narrative: String,
    pub related_adrs: Vec<String>,
}

/// Request to pin an evidence item to an investigation (ADR-005 E21-2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PinEvidenceRequest {
    /// The object id this evidence references (e.g. `symbol:path:name:line`).
    pub object_id: String,
    /// Optional view id when the evidence was captured from a specific view.
    pub view_id: Option<String>,
    /// User-authored note explaining why this evidence is relevant.
    pub note: String,
}

/// Investigation facade trait — ISP-segregated port for investigation operations.
#[async_trait]
pub trait InvestigationFacade: Send + Sync {
    /// Create a new investigation.
    async fn create_investigation(
        &self,
        workspace_id: &str,
        title: &str,
        goal: &str,
    ) -> ExplorerResult<Investigation>;

    /// Get an investigation by ID.
    async fn get_investigation(&self, id: &str) -> ExplorerResult<Option<Investigation>>;

    /// List all investigations for a workspace.
    async fn list_investigations(&self, workspace_id: &str) -> ExplorerResult<Vec<Investigation>>;

    /// Update an existing investigation.
    async fn update_investigation(&self, investigation: Investigation) -> ExplorerResult<()>;

    /// Delete an investigation by ID.
    async fn delete_investigation(&self, id: &str) -> ExplorerResult<()>;

    /// Add a single evidence item to an existing investigation.
    async fn add_evidence(
        &self,
        investigation_id: &str,
        evidence: Evidence,
    ) -> ExplorerResult<()>;
}

/// Wrapper that adapts `InvestigationService<S>` from core to the
/// `InvestigationFacade` trait.
pub struct InvestigationServiceImpl<
    S: cognicode_core::domain::investigation_store::InvestigationStore,
> {
    inner: cognicode_core::application::investigation_service::InvestigationService<S>,
}

impl<S: cognicode_core::domain::investigation_store::InvestigationStore + 'static>
    InvestigationServiceImpl<S>
{
    pub fn new(store: S) -> Self {
        Self {
            inner: cognicode_core::application::investigation_service::InvestigationService::new(store),
        }
    }
}

#[async_trait]
impl<S: cognicode_core::domain::investigation_store::InvestigationStore + 'static>
    InvestigationFacade for InvestigationServiceImpl<S>
{
    async fn create_investigation(
        &self,
        workspace_id: &str,
        title: &str,
        goal: &str,
    ) -> ExplorerResult<Investigation> {
        self.inner
            .create_investigation(workspace_id, title, goal)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e.to_string())))
    }

    async fn get_investigation(&self, id: &str) -> ExplorerResult<Option<Investigation>> {
        self.inner
            .get_investigation(id)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e.to_string())))
    }

    async fn list_investigations(&self, workspace_id: &str) -> ExplorerResult<Vec<Investigation>> {
        self.inner
            .list_investigations(workspace_id)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e.to_string())))
    }

    async fn update_investigation(&self, investigation: Investigation) -> ExplorerResult<()> {
        self.inner
            .update_investigation(investigation)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e.to_string())))
    }

    async fn delete_investigation(&self, id: &str) -> ExplorerResult<()> {
        self.inner
            .delete_investigation(id)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e.to_string())))
    }

    async fn add_evidence(
        &self,
        investigation_id: &str,
        evidence: Evidence,
    ) -> ExplorerResult<()> {
        self.inner
            .add_evidence(investigation_id, evidence)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e.to_string())))
    }
}

/// Create an investigation service from a postgres store.
#[cfg(feature = "postgres")]
pub fn new_investigation_service_from_postgres(
    pool: &sqlx::PgPool,
) -> Arc<dyn InvestigationFacade> {
    let store = cognicode_core::infrastructure::persistence::PostgresInvestigationStore::new(pool.clone());
    Arc::new(InvestigationServiceImpl::new(store))
}
