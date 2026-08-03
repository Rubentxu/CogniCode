//! Investigation facade — ADR-005 Phase INV-1 + ADR-010 E24.1.
//!
//! Provides a typed facade for investigation operations over the HTTP API.

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{ExplorerError, ExplorerResult};

pub use cognicode_core::domain::investigation::Evidence;

// Re-export the domain types from cognicode-core.
pub use cognicode_core::domain::investigation::Artifact;
pub use cognicode_core::domain::investigation::DiagramProvenance;
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

/// Request to add an artifact to an investigation (ADR-005 E21-6 + ADR-010 E24.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AddArtifactRequest {
    /// The kind of artifact (e.g. "mermaid", "svg", "drawio").
    pub kind: String,
    /// Human-readable title for this artifact.
    pub title: String,
    /// The generated content.
    pub content: String,
    /// Optional reference to the object/view that generated this artifact.
    /// Retained for backward compatibility with pre-E24.1 clients.
    pub generated_from: Option<String>,
    /// Structured provenance metadata — ADR-010 R1–R2.
    /// Defaults to None for backward compatibility.
    #[serde(default)]
    pub provenance: Option<DiagramProvenance>,
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
    async fn add_evidence(&self, investigation_id: &str, evidence: Evidence) -> ExplorerResult<()>;

    /// Add a single artifact to an existing investigation (ADR-010 E24.1).
    /// Returns the persisted Artifact (with server-assigned id and stamped provenance.created_at).
    async fn add_artifact(
        &self,
        investigation_id: &str,
        request: AddArtifactRequest,
    ) -> ExplorerResult<Artifact>;
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
            inner: cognicode_core::application::investigation_service::InvestigationService::new(
                store,
            ),
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

    async fn add_evidence(&self, investigation_id: &str, evidence: Evidence) -> ExplorerResult<()> {
        self.inner
            .add_evidence(investigation_id, evidence)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e.to_string())))
    }

    async fn add_artifact(
        &self,
        investigation_id: &str,
        request: AddArtifactRequest,
    ) -> ExplorerResult<Artifact> {
        // Build the Artifact from the request. The service will stamp provenance.created_at.
        let artifact = Artifact {
            id: format!(
                "art_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            kind: request.kind,
            title: request.title,
            content: request.content,
            generated_from: request.generated_from,
            provenance: request.provenance,
        };

        self.inner
            .add_artifact(investigation_id, artifact)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!(e.to_string())))
    }
}

