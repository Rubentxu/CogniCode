//! Diagram artifact regenerator — ADR-010 E24.1.
//!
//! Regenerates diagram content from structured provenance metadata.
//! Reuses the existing `emit_mermaid_for_snapshot` dispatch table.

use crate::domain::snapshot_dispatch::emit_mermaid_for_snapshot;
use crate::domain::snapshot_dispatch::SnapshotViewKind;
use crate::facades::{GraphService, WorkspaceService};

/// Errors that can occur during diagram regeneration.
#[derive(Debug, thiserror::Error)]
pub enum RegenerateError {
    #[error("source object not found in graph (artifact may be stale)")]
    SourceNotFound,

    #[error("view_kind not supported for regeneration: {0}")]
    UnsupportedViewKind(String),

    #[error("export_format not supported for regeneration: {0}")]
    UnsupportedFormat(String),

    #[error("regeneration failed: {0}")]
    EmissionFailed(String),
}

/// Regenerates diagram artifacts from their provenance metadata.
///
/// Given an artifact with `DiagramProvenance`, resolves the source object and
/// re-emits the Mermaid content using the existing dispatch table.
pub struct DiagramRegenerator;

impl DiagramRegenerator {
    /// Regenerate the Mermaid content for an artifact with provenance.
    ///
    /// Returns the new Mermaid text content on success.
    /// Returns `Err(RegenerateError::SourceNotFound)` when the source object
    /// is no longer in the graph.
    pub async fn regenerate(
        provenance: &cognicode_core::domain::investigation::DiagramProvenance,
        graph_service: &dyn GraphService,
        workspace: &dyn WorkspaceService,
    ) -> Result<String, RegenerateError> {
        use cognicode_core::domain::investigation::ExportFormat;

        // Only Mermaid format is supported for regeneration.
        match provenance.export_format {
            ExportFormat::Mermaid => {}
            ExportFormat::Svg
            | ExportFormat::Png
            | ExportFormat::Drawio => {
                return Err(RegenerateError::UnsupportedFormat(
                    provenance.export_format.to_string(),
                ));
            }
        }

        // Parse the view_kind string into SnapshotViewKind.
        let view_kind = SnapshotViewKind::from_str(&provenance.view_kind)
            .map_err(|_| RegenerateError::UnsupportedViewKind(provenance.view_kind.clone()))?;

        // Re-emit using the existing dispatch table.
        emit_mermaid_for_snapshot(graph_service, workspace, view_kind, Some(&provenance.object_id))
            .await
            .map_err(|e| RegenerateError::EmissionFailed(e.to_string()))
    }
}
