//! Execution context passed to every tool handler.
//!
//! [`McpContext`] holds the ports that handlers need:
//! - 5 facades — Workspace, Search, View, MoldQL, Persistence
//! - [`CallGraph`] — optional call graph for impact/graph tools
//! - [`SessionRegistry`] — brain-session registry for session tools
//!
//! The context is constructed once at handler creation and shared across
//! all invocations. Handlers that need only a subset of these fields
//! borrow only what they use (ISP compliance).

use std::sync::Arc;

use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::traits::GraphQueryPort;

use crate::facades::{
    GraphService, MoldQLService, PersistenceService, SearchService, ViewService, WorkspaceService,
};
use crate::session::SessionRegistry;

/// Optional Generic Graph Layer port for multimodal queries.
/// Populated when `multimodal` feature is enabled and a
/// `GraphRepository` has been wired in.
#[cfg(feature = "multimodal")]
use crate::ports::GraphRepository;

/// The shared execution context passed to every tool handler.
///
/// All fields are public so handlers can borrow exactly what they need
/// without a getter API (zero-cost abstraction, ISP-compliant).
pub struct McpContext {
    /// Optional in-memory call graph. `None` when the graph has not
    /// been loaded (e.g. no `--postgres` flag on startup). Handlers
    /// that need the graph should return a structured error envelope
    /// rather than panicking.
    pub graph: Option<Arc<CallGraph>>,
    /// Brain-session registry — holds all live sessions and their
    /// per-session state.
    pub session_registry: SessionRegistry,
    /// Optional Generic Graph Layer port for multimodal queries.
    /// `None` on default builds or when no repo was wired at startup.
    #[cfg(feature = "multimodal")]
    pub graph_repo: Option<Arc<dyn GraphRepository>>,
    /// Workspace facade — PR 1 migration target from service.
    pub workspace: Option<Arc<dyn WorkspaceService>>,
    /// Search facade — PR 1 migration target from service.
    pub search: Option<Arc<dyn SearchService>>,
    /// View facade — PR 2 migration target from service.
    pub view: Option<Arc<dyn ViewService>>,
    /// MoldQL facade — PR 2 migration target from service.
    pub moldql: Option<Arc<dyn MoldQLService>>,
    /// Persistence facade — PR 3 migration target from service.
    pub persistence: Option<Arc<dyn PersistenceService>>,
    /// Graph query port for Phase 4 graph traversal queries.
    pub graph_query: Option<Arc<dyn GraphQueryPort>>,
    /// Graph service facade — provides build_architecture and compare_architecture.
    pub graph_service: Option<Arc<dyn GraphService>>,
}

impl McpContext {
    /// Construct a new context from the primary ports.
    pub fn new(graph: Option<Arc<CallGraph>>, session_registry: SessionRegistry) -> Self {
        Self {
            graph,
            session_registry,
            #[cfg(feature = "multimodal")]
            graph_repo: None,
            workspace: None,
            search: None,
            view: None,
            moldql: None,
            persistence: None,
            graph_query: None,
            graph_service: None,
        }
    }

    /// Start a builder chain for McpContext.
    pub fn builder() -> McpContextBuilder {
        McpContextBuilder::new()
    }

    /// Wire a `GraphRepository` into the context (multimodal builds only).
    #[cfg(feature = "multimodal")]
    pub fn with_graph_repo(mut self, repo: Arc<dyn GraphRepository>) -> Self {
        self.graph_repo = Some(repo);
        self
    }

    /// Wire a `WorkspaceService` facade into the context (PR 1).
    pub fn with_workspace(mut self, w: Arc<dyn WorkspaceService>) -> Self {
        self.workspace = Some(w);
        self
    }

    /// Wire a `SearchService` facade into the context (PR 1).
    pub fn with_search(mut self, s: Arc<dyn SearchService>) -> Self {
        self.search = Some(s);
        self
    }

    /// Wire a `ViewService` facade into the context (PR 2).
    pub fn with_view(mut self, v: Arc<dyn ViewService>) -> Self {
        self.view = Some(v);
        self
    }

    /// Wire a `MoldQLService` facade into the context (PR 2).
    pub fn with_moldql(mut self, m: Arc<dyn MoldQLService>) -> Self {
        self.moldql = Some(m);
        self
    }

    /// Wire a `PersistenceService` facade into the context (PR 3).
    pub fn with_persistence(mut self, p: Arc<dyn PersistenceService>) -> Self {
        self.persistence = Some(p);
        self
    }
}

/// Builder for [`McpContext`].
pub struct McpContextBuilder {
    graph: Option<Option<Arc<CallGraph>>>,
    session_registry: Option<SessionRegistry>,
    workspace: Option<Arc<dyn WorkspaceService>>,
    search: Option<Arc<dyn SearchService>>,
    view: Option<Arc<dyn ViewService>>,
    moldql: Option<Arc<dyn MoldQLService>>,
    persistence: Option<Arc<dyn PersistenceService>>,
    graph_query: Option<Arc<dyn GraphQueryPort>>,
    graph_service: Option<Arc<dyn GraphService>>,
    #[cfg(feature = "multimodal")]
    graph_repo: Option<Option<Arc<dyn crate::ports::graph_repository::GraphRepository>>>,
}

impl McpContextBuilder {
    pub fn new() -> Self {
        Self {
            graph: Some(None),
            session_registry: None,
            workspace: None,
            search: None,
            view: None,
            moldql: None,
            persistence: None,
            graph_query: None,
            graph_service: None,
            #[cfg(feature = "multimodal")]
            graph_repo: Some(None),
        }
    }

    /// Set the call graph.
    pub fn with_graph(mut self, graph: Option<Arc<CallGraph>>) -> Self {
        self.graph = Some(graph);
        self
    }

    /// Set the session registry.
    pub fn with_session_registry(mut self, registry: SessionRegistry) -> Self {
        self.session_registry = Some(registry);
        self
    }

    /// Wire a `WorkspaceService` facade (PR 1).
    pub fn with_workspace(mut self, workspace: Arc<dyn WorkspaceService>) -> Self {
        self.workspace = Some(workspace);
        self
    }

    /// Wire a `SearchService` facade (PR 1).
    pub fn with_search(mut self, search: Arc<dyn SearchService>) -> Self {
        self.search = Some(search);
        self
    }

    /// Wire a `ViewService` facade (PR 2).
    pub fn with_view(mut self, view: Arc<dyn ViewService>) -> Self {
        self.view = Some(view);
        self
    }

    /// Wire a `MoldQLService` facade (PR 2).
    pub fn with_moldql(mut self, moldql: Arc<dyn MoldQLService>) -> Self {
        self.moldql = Some(moldql);
        self
    }

    /// Wire a `PersistenceService` facade (PR 3).
    pub fn with_persistence(mut self, persistence: Arc<dyn PersistenceService>) -> Self {
        self.persistence = Some(persistence);
        self
    }

    /// Wire a `GraphQueryPort` into the context (Phase 4).
    pub fn with_graph_query(mut self, graph_query: Arc<dyn GraphQueryPort>) -> Self {
        self.graph_query = Some(graph_query);
        self
    }

    /// Wire a `GraphService` facade into the context.
    pub fn with_graph_service(mut self, graph_service: Arc<dyn GraphService>) -> Self {
        self.graph_service = Some(graph_service);
        self
    }

    /// Wire an optional `GraphQueryPort` into the context (Phase 4).
    /// Passes through `None` when `graph_query` is `None`.
    pub fn with_optional_graph_query(
        mut self,
        graph_query: Option<Arc<dyn GraphQueryPort>>,
    ) -> Self {
        self.graph_query = graph_query;
        self
    }

    /// Wire a `GraphRepository` (multimodal builds only).
    #[cfg(feature = "multimodal")]
    pub fn with_graph_repo(mut self, repo: Option<Arc<dyn GraphRepository>>) -> Self {
        self.graph_repo = Some(repo);
        self
    }

    /// Build the [`McpContext`].
    pub fn build(self) -> McpContext {
        McpContext {
            graph: self.graph.unwrap_or(None),
            session_registry: self
                .session_registry
                .unwrap_or_else(|| crate::session::SessionRegistry::new()),
            #[cfg(feature = "multimodal")]
            graph_repo: self.graph_repo.unwrap_or(None),
            workspace: self.workspace,
            search: self.search,
            view: self.view,
            moldql: self.moldql,
            persistence: self.persistence,
            graph_query: self.graph_query,
            graph_service: self.graph_service,
        }
    }
}

impl Default for McpContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}
