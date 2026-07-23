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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ExplorerError;
    use crate::facades::SubgraphDirection;
    use crate::ports::symbol_repository::ResolvedSymbol;
    use async_trait::async_trait;
    use std::sync::Arc;
    use time::OffsetDateTime;

    /// Mock GraphService for unit testing.
    struct MockGraphService {
        resolve_symbol_result: Option<ResolvedSymbol>,
        graph_query_result: Option<Arc<dyn crate::ports::GraphQueryPort>>,
    }

    #[async_trait]
    impl GraphService for MockGraphService {
        async fn resolve_symbol(&self, _id: &str) -> Result<Option<ResolvedSymbol>, ExplorerError> {
            Ok(self.resolve_symbol_result.clone())
        }

        fn graph_query(&self) -> Option<Arc<dyn crate::ports::GraphQueryPort>> {
            self.graph_query_result.clone()
        }

        async fn build_subgraph(
            &self,
            _root_id: &str,
            _depth: u8,
            _direction: SubgraphDirection,
            _max_nodes: u32,
        ) -> Result<crate::dto::SubgraphResponse, ExplorerError> {
            unimplemented!()
        }

        async fn build_architecture(
            &self,
            _root_path: &str,
        ) -> Result<crate::dto::SubgraphResponse, ExplorerError> {
            unimplemented!()
        }

        async fn compare_architecture(
            &self,
            _root_path: &str,
        ) -> Result<crate::dto::DriftReport, ExplorerError> {
            unimplemented!()
        }

        async fn landing_entry_points(
            &self,
            _limit: usize,
        ) -> Result<(Vec<ResolvedSymbol>, usize), ExplorerError> {
            unimplemented!()
        }

        async fn landing_hot_paths(
            &self,
            _limit: usize,
            _min_fan_in: usize,
        ) -> Result<Vec<ResolvedSymbol>, ExplorerError> {
            unimplemented!()
        }

        async fn landing_god_nodes(
            &self,
            _limit: usize,
        ) -> Result<Vec<crate::dto::GodNodeEntry>, ExplorerError> {
            unimplemented!()
        }
    }

    /// Mock WorkspaceService for unit testing.
    struct MockWorkspaceService;

    #[async_trait]
    impl WorkspaceService for MockWorkspaceService {
        async fn open_workspace(
            &self,
            _request: crate::dto::OpenWorkspaceRequest,
        ) -> Result<crate::dto::WorkspaceSummary, ExplorerError> {
            unimplemented!()
        }

        fn current_workspace(&self) -> Result<crate::dto::WorkspaceSummary, ExplorerError> {
            Ok(crate::dto::WorkspaceSummary {
                id: "test-workspace".to_string(),
                root_path: "/tmp".to_string(),
                graph_status: crate::dto::GraphStatus::Ready,
                indexed_at: None,
                symbol_count: 0,
                relation_count: 0,
            })
        }
    }

    fn make_provenance(object_id: &str, view_kind: &str) -> cognicode_core::domain::investigation::DiagramProvenance {
        use cognicode_core::domain::investigation::ExportFormat;
        cognicode_core::domain::investigation::DiagramProvenance {
            object_id: object_id.to_string(),
            view_kind: view_kind.to_string(),
            spec_id: None,
            query_id: None,
            export_format: ExportFormat::Mermaid,
            created_at: OffsetDateTime::now_utc(),
        }
    }

    /// Helper to create a minimal ResolvedSymbol for tests.
    fn make_resolved_symbol(name: &str, file: &str, line: u32) -> ResolvedSymbol {
        ResolvedSymbol {
            id: cognicode_core::domain::aggregates::SymbolId::new(&format!("symbol:{}:{}:{}", file, name, line)),
            name: name.to_string(),
            kind: cognicode_core::domain::SymbolKind::Function,
            file: file.to_string(),
            line,
            signature: None,
        }
    }

    #[tokio::test]
    async fn regenerate_unsupported_format_svg() {
        use cognicode_core::domain::investigation::ExportFormat;
        let provenance = cognicode_core::domain::investigation::DiagramProvenance {
            object_id: "symbol:test.rs:main:1".to_string(),
            view_kind: "call_graph".to_string(),
            spec_id: None,
            query_id: None,
            export_format: ExportFormat::Svg,
            created_at: OffsetDateTime::now_utc(),
        };

        let graph = MockGraphService {
            resolve_symbol_result: Some(make_resolved_symbol("test", "test.rs", 1)),
            graph_query_result: None,
        };
        let workspace = MockWorkspaceService;

        let result = DiagramRegenerator::regenerate(&provenance, &graph, &workspace).await;
        assert!(matches!(result, Err(RegenerateError::UnsupportedFormat(_))));
    }

    #[tokio::test]
    async fn regenerate_unsupported_format_png() {
        use cognicode_core::domain::investigation::ExportFormat;
        let provenance = cognicode_core::domain::investigation::DiagramProvenance {
            object_id: "symbol:test.rs:main:1".to_string(),
            view_kind: "call_graph".to_string(),
            spec_id: None,
            query_id: None,
            export_format: ExportFormat::Png,
            created_at: OffsetDateTime::now_utc(),
        };

        let graph = MockGraphService {
            resolve_symbol_result: Some(make_resolved_symbol("test", "test.rs", 1)),
            graph_query_result: None,
        };
        let workspace = MockWorkspaceService;

        let result = DiagramRegenerator::regenerate(&provenance, &graph, &workspace).await;
        assert!(matches!(result, Err(RegenerateError::UnsupportedFormat(_))));
    }

    #[tokio::test]
    async fn regenerate_unsupported_view_kind() {
        let provenance = make_provenance("symbol:test.rs:main:1", "invalid_view_kind");

        let graph = MockGraphService {
            resolve_symbol_result: Some(make_resolved_symbol("test", "test.rs", 1)),
            graph_query_result: None,
        };
        let workspace = MockWorkspaceService;

        let result = DiagramRegenerator::regenerate(&provenance, &graph, &workspace).await;
        assert!(matches!(result, Err(RegenerateError::UnsupportedViewKind(_))));
    }
}
