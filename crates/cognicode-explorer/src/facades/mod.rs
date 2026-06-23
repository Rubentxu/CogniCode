//! Service facades — ISP-segregated trait boundaries for the explorer service.
//!
//! Each facade trait groups related capabilities that handlers consume via
//! `Arc<dyn Trait>`. The concrete implementations live in sibling modules.
//!
//! ## Facade overview
//!
//! | Facade | Responsibility |
//! |---|---|
//! | [`WorkspaceService`] | Workspace lifecycle — open, current workspace |
//! | [`SearchService`] | Spotter search and object inspection |
//! | [`ViewService`] | View listing, contextual view, lenses |
//! | [`PersistenceService`] | Exploration paths, artifacts, ViewSpec CRUD |
//! | [`MoldQLService`] | MoldQL query execution |
//!
//! PR 1 wires [`WorkspaceService`] and [`SearchService`].

pub mod graph;
pub mod moldql;
pub mod persistence;
pub mod search;
pub mod view;
pub mod workspace;

use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::domain::traits::GraphQueryPort;

use crate::dto::{
    ContextualGraphResponse, ContextualView, DecisionArtifactSummary, DriftReport, GraphNode,
    ExplorationSession, GenerateArtifactRequest, InspectableObjectSummary, LensDescriptor,
    LensResult, SpotterResult, SpotterSearchResult, SubgraphResponse, ViewDescriptor, ViewSpec,
    WorkspaceSummary,
};
use crate::error::ExplorerResult;
use crate::moldql::MoldQLResult;
use crate::ports::symbol_repository::{ResolvedSymbol, SymbolRepository};

// ============================================================================
// GraphService
// ============================================================================

/// Direction filter for a subgraph traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubgraphDirection {
    Incoming,
    Outgoing,
    Both,
}

/// Graph traversal facade.
///
/// Bundles `symbol_repo` + `graph_query` for symbol resolution and subgraph
/// traversal operations.
#[async_trait]
pub trait GraphService: Send + Sync {
    /// Resolve a symbol by id, returning the resolved identity.
    async fn resolve_symbol(&self, id: &str) -> ExplorerResult<Option<ResolvedSymbol>>;

    /// Return the graph query port if available.
    fn graph_query(&self) -> Option<Arc<dyn GraphQueryPort>>;

    /// Build a BFS subgraph from root_id.
    async fn build_subgraph(
        &self,
        root_id: &str,
        depth: u8,
        direction: SubgraphDirection,
        max_nodes: u32,
    ) -> ExplorerResult<SubgraphResponse>;

    /// Build an architecture view synthesised from `module_list()`.
    /// C3 components are directories; edges reflect parent-child relationships.
    /// `root_path` is the workspace root directory for parsing Cargo.toml and package.json.
    async fn build_architecture(&self, root_path: &str) -> ExplorerResult<SubgraphResponse>;

    /// Compare the inferred architecture against `.cognicode/expected-architecture.yaml`.
    /// Returns a drift report if the file exists; empty report otherwise.
    async fn compare_architecture(&self, root_path: &str) -> ExplorerResult<DriftReport>;
}

// ============================================================================
// WorkspaceService
// ============================================================================

/// Workspace lifecycle operations.
#[async_trait]
pub trait WorkspaceService: Send + Sync {
    /// Open a workspace at the given path, returning its summary.
    async fn open_workspace(
        &self,
        request: crate::dto::OpenWorkspaceRequest,
    ) -> ExplorerResult<WorkspaceSummary>;

    /// Return the current workspace summary (the one bound at startup).
    fn current_workspace(&self) -> ExplorerResult<WorkspaceSummary>;
}

// ============================================================================
// SearchService
// ============================================================================

/// Spotter search and object inspection.
#[async_trait]
pub trait SearchService: Send + Sync {
    /// Search symbols by name with optional kind filter.
    async fn spotter_search(
        &self,
        query: &str,
        kind: Option<&str>,
    ) -> ExplorerResult<Vec<SpotterResult>>;

    /// Search symbols and ViewSpecs, merging results.
    async fn spotter_search_with_viewspecs(
        &self,
        query: &str,
        kind: Option<&str>,
        workspace_id: Option<&str>,
    ) -> ExplorerResult<Vec<SpotterSearchResult>>;

    /// Inspect an object by its MVP id, returning a summary.
    async fn inspect_object(&self, object_id: &str) -> ExplorerResult<InspectableObjectSummary>;
}

// ============================================================================
// ViewService
// ============================================================================

/// Contextual views, lenses, and ViewSpec execution.
#[async_trait]
pub trait ViewService: Send + Sync {
    /// List built-in views available for the given object.
    async fn available_views(
        &self,
        object_id: &str,
    ) -> ExplorerResult<Vec<ViewDescriptor>>;

    /// Build a contextual view for an object.
    async fn contextual_view(
        &self,
        object_id: &str,
        view_id: &str,
    ) -> ExplorerResult<ContextualView>;

    /// Build a contextual graph (focus + parent + children + same-level).
    async fn build_contextual_graph(
        &self,
        focus_id: &str,
        level: &str,
        depth: u8,
        max_nodes: usize,
    ) -> ExplorerResult<ContextualGraphResponse>;

    /// List design lenses available for the given object.
    async fn available_lenses(&self, object_id: &str) -> ExplorerResult<Vec<LensDescriptor>>;

    /// Apply a design lens to an object.
    async fn apply_lens(&self, object_id: &str, lens_id: &str) -> ExplorerResult<LensResult>;

    /// Execute a ViewSpec against an object.
    async fn execute_view_spec(
        &self,
        spec: &ViewSpec,
        object_id: &str,
    ) -> ExplorerResult<ContextualView>;
}

// ============================================================================
// PersistenceService
// ============================================================================

/// Exploration sessions, artifacts, and ViewSpec CRUD (ADR-045 Phase 1).
#[async_trait]
pub trait PersistenceService: Send + Sync {
    /// Save an exploration session (semantic navigation history, ADR-016 Fase 3).
    async fn save_exploration_session(
        &self,
        request: crate::dto::SaveExplorationSessionRequest,
    ) -> ExplorerResult<crate::dto::ExplorationSession>;

    /// Load an exploration session by id.
    async fn load_exploration_session(
        &self,
        session_id: &str,
    ) -> ExplorerResult<Option<crate::dto::ExplorationSession>>;

    /// Generate a decision artifact from a saved exploration session.
    async fn generate_artifact(
        &self,
        exploration_id: &str,
        request: GenerateArtifactRequest,
    ) -> ExplorerResult<DecisionArtifactSummary>;

    /// Persist a ViewSpec.
    async fn save_view_spec(&self, spec: &ViewSpec, workspace_id: &str, owner: &str)
        -> ExplorerResult<()>;

    /// Load a ViewSpec by id.
    async fn load_view_spec(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> ExplorerResult<Option<ViewSpec>>;

    /// List ViewSpecs for a workspace+owner scope.
    async fn list_view_specs(
        &self,
        workspace_id: &str,
        owner: &str,
    ) -> ExplorerResult<Vec<ViewSpec>>;

    /// Delete a ViewSpec. Returns `true` if a row was removed.
    async fn delete_view_spec(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> ExplorerResult<bool>;

    /// List all saved exploration sessions for a workspace, sorted by creation time.
    ///
    /// ## KNOWN-DEBT (ADR-045 Phase 1 — resolved)
    ///
    /// - Debt 1 ✅: Orphaned `GET /api/explorations/:id` route removed.
    /// - Debt 2 ✅: Dual model unified onto `ExplorationSession` (ADR-040 Wave 3 aligned).
    /// - Debt 3 ⚠️: In-memory store lifetime — Postgres persistence deferred to Phase 2.
    async fn list_explorations(
        &self,
        workspace_id: &str,
    ) -> ExplorerResult<Vec<ExplorationSession>>;
}

// ============================================================================
// MoldQLService
// ============================================================================

/// MoldQL query execution.
#[async_trait]
pub trait MoldQLService: Send + Sync {
    /// Execute a MoldQL query against the default target.
    async fn execute_query(&self, query: &str) -> ExplorerResult<MoldQLResult>;

    /// Execute a MoldQL query against a specific compile target.
    async fn execute_query_with_target(
        &self,
        query: &str,
        target: crate::moldql::compile::CompileTarget,
    ) -> ExplorerResult<MoldQLResult>;
}

// ============================================================================
// LensExecutor
// ============================================================================

/// Standalone lens execution for clients that only need lens application.
#[async_trait]
pub trait LensExecutor: Send + Sync {
    /// Apply a design lens to an object, returning the lens result.
    async fn apply_lens(&self, object_id: &str, lens_id: &str) -> ExplorerResult<LensResult>;
}
