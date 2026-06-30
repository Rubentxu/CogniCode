//! Snapshot view-kind dispatch for rendering canonical Mermaid diagrams.
//!
//! Provides the [`SnapshotViewKind`] enum and [`emit_mermaid_for_snapshot`]
//! function used by both the HTTP API handler (`api.rs`) and the MCP
//! tool handler (`mcp/handler/snapshot.rs`).
//!
//! ## Design note
//!
//! `emit_mermaid_for_snapshot` requires workspace and graph access.  The
//! function is intentionally not shared because `api.rs` works with a concrete
//! `ApiState` while `mcp/handler/snapshot.rs` works with an `McpContext` —
//! these contexts have different service types and error hierarchies.
//! What IS shared is the `SnapshotViewKind` taxonomy and its parsing/dispatch.

use std::sync::Arc;

use crate::domain::c4_mermaid::{c4_to_mermaid, C4Level};
use crate::domain::trace_mermaid::{
    call_graph_to_mermaid, impact_radius_to_mermaid, vertical_slice_to_mermaid, TraceEmitContext,
};
use crate::dto::InspectionTarget;

// ============================================================================
// SnapshotViewKind
// ============================================================================

/// Snapshot-specific view kinds supported for PNG/SVG rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotViewKind {
    C4Context,
    C4Container,
    C4Component,
    CallGraph,
    ImpactRadius,
    VerticalSlice,
}

impl SnapshotViewKind {
    /// Parse a view-kind string into a [`SnapshotViewKind`].
    ///
    /// Returns `Err(String)` with the unrecognized value.
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "c4_context" => Ok(Self::C4Context),
            "c4_container" => Ok(Self::C4Container),
            "c4_component" => Ok(Self::C4Component),
            "call_graph" => Ok(Self::CallGraph),
            "impact_radius" => Ok(Self::ImpactRadius),
            "vertical_slice" => Ok(Self::VerticalSlice),
            other => Err(other.to_string()),
        }
    }

    /// Returns `true` for trace-based view kinds (call graph, impact radius,
    /// vertical slice) that require a target symbol.
    pub fn is_trace_kind(&self) -> bool {
        matches!(
            self,
            Self::CallGraph | Self::ImpactRadius | Self::VerticalSlice
        )
    }

    /// Returns the corresponding [`C4Level`] for C4 view kinds.
    pub fn as_c4_level(self) -> Option<C4Level> {
        match self {
            Self::C4Context => Some(C4Level::Context),
            Self::C4Container => Some(C4Level::Container),
            Self::C4Component => Some(C4Level::Component),
            _ => None,
        }
    }
}

/// Whitelist of view kinds that support snapshot rendering.
pub const SNAPSHOT_VIEW_KINDS: &[&str] = &[
    "c4_context",
    "c4_container",
    "c4_component",
    "call_graph",
    "impact_radius",
    "vertical_slice",
];

// ============================================================================
// Mermaid emission traits
// ============================================================================

/// Port for emitting Mermaid text for a snapshot view kind.
///
/// Abstracts over the API state (`ApiState`) and MCP context (`McpContext`)
/// so the dispatch logic is written once.
pub trait SnapshotMermaidEmitter {
    /// Emit Mermaid text for the given view kind and optional target.
    fn emit_mermaid(
        &self,
        view_kind: SnapshotViewKind,
        target: Option<&str>,
    ) -> impl std::future::Future<Output = Result<String, String>> + Send;
}

// ============================================================================
// Mermaid emission for trace-based view kinds (call graph / impact radius /
// vertical slice).
// ============================================================================

/// Emit Mermaid text for a trace-based snapshot view kind.
pub async fn emit_trace_mermaid(
    graph_service: &dyn crate::facades::GraphService,
    workspace: &dyn crate::facades::WorkspaceService,
    view_kind: SnapshotViewKind,
    target: &str,
) -> Result<String, String> {
    let graph_query = graph_service
        .graph_query()
        .ok_or_else(|| "call graph not loaded".to_string())?;

    let resolved = graph_service
        .resolve_symbol(target)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("target not found: {target}"))?;

    let inspection_target = InspectionTarget::Symbol(resolved);
    let trace_ctx = TraceEmitContext {
        graph_query: graph_query.as_ref(),
        target: &inspection_target,
    };

    let mermaid = match view_kind {
        SnapshotViewKind::CallGraph => call_graph_to_mermaid(&trace_ctx, target),
        SnapshotViewKind::ImpactRadius => impact_radius_to_mermaid(&trace_ctx, target),
        SnapshotViewKind::VerticalSlice => vertical_slice_to_mermaid(&trace_ctx, target),
        _ => unreachable!(),
    };

    Ok(mermaid)
}

/// Emit Mermaid text for a C4-level snapshot view kind.
pub async fn emit_c4_mermaid(
    graph_service: &dyn crate::facades::GraphService,
    workspace: &dyn crate::facades::WorkspaceService,
    view_kind: SnapshotViewKind,
) -> Result<String, String> {
    let level = view_kind
        .as_c4_level()
        .expect("as_c4_level called on non-C4 view kind");

    let workspace_info = workspace
        .current_workspace()
        .map_err(|e| e.to_string())?;

    let architecture = graph_service
        .build_architecture(&workspace_info.root_path)
        .await
        .map_err(|e| e.to_string())?;

    Ok(c4_to_mermaid(
        &architecture.nodes,
        &architecture.edges,
        level,
    ))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_view_kind_from_str_valid() {
        assert_eq!(SnapshotViewKind::from_str("c4_context"), Ok(SnapshotViewKind::C4Context));
        assert_eq!(SnapshotViewKind::from_str("c4_container"), Ok(SnapshotViewKind::C4Container));
        assert_eq!(SnapshotViewKind::from_str("c4_component"), Ok(SnapshotViewKind::C4Component));
        assert_eq!(SnapshotViewKind::from_str("call_graph"), Ok(SnapshotViewKind::CallGraph));
        assert_eq!(SnapshotViewKind::from_str("impact_radius"), Ok(SnapshotViewKind::ImpactRadius));
        assert_eq!(SnapshotViewKind::from_str("vertical_slice"), Ok(SnapshotViewKind::VerticalSlice));
    }

    #[test]
    fn snapshot_view_kind_from_str_invalid() {
        assert_eq!(SnapshotViewKind::from_str("invalid"), Err("invalid".to_string()));
        assert_eq!(SnapshotViewKind::from_str("decision_trace"), Err("decision_trace".to_string()));
    }

    #[test]
    fn snapshot_view_kind_is_trace_kind() {
        assert!(!SnapshotViewKind::C4Context.is_trace_kind());
        assert!(!SnapshotViewKind::C4Container.is_trace_kind());
        assert!(!SnapshotViewKind::C4Component.is_trace_kind());
        assert!(SnapshotViewKind::CallGraph.is_trace_kind());
        assert!(SnapshotViewKind::ImpactRadius.is_trace_kind());
        assert!(SnapshotViewKind::VerticalSlice.is_trace_kind());
    }

    #[test]
    fn snapshot_view_kind_as_c4_level() {
        assert_eq!(SnapshotViewKind::C4Context.as_c4_level(), Some(C4Level::Context));
        assert_eq!(SnapshotViewKind::C4Container.as_c4_level(), Some(C4Level::Container));
        assert_eq!(SnapshotViewKind::C4Component.as_c4_level(), Some(C4Level::Component));
        assert_eq!(SnapshotViewKind::CallGraph.as_c4_level(), None);
        assert_eq!(SnapshotViewKind::ImpactRadius.as_c4_level(), None);
        assert_eq!(SnapshotViewKind::VerticalSlice.as_c4_level(), None);
    }
}
