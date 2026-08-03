//! CogniCode Runtime — shared bootstrap for API and MCP binaries.
//!
//! LadybugDB is the sole storage backend (e29-7 full postgres removal).
//! The runtime carries an optional `backend: Option<Arc<dyn PgBackend>>`
//! (ladybug path) that exposes the relocated domain ports
//! (`quality_store` / `view_spec_store` / `call_graph_store`).
//! `bootstrap_with_backend` is the canonical entry point.

use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

pub struct Runtime {
    pub symbol_repo: Arc<dyn cognicode_explorer::ports::SymbolRepository>,
    pub source_reader: Arc<dyn cognicode_explorer::ports::SourceReader>,
    pub graph: Option<Arc<cognicode_core::domain::aggregates::CallGraph>>,
    pub cwd: PathBuf,
    /// GraphCache for serving the in-memory graph (ArcSwap).
    pub graph_cache: Arc<cognicode_core::infrastructure::graph::graph_cache::GraphCache>,
    /// `PgBackend` trait object — the storage backend contract
    /// (`LadybugPgBackend` implements it). The runtime uses this for
    /// port construction on the ladybug path.
    pub backend: Option<Arc<dyn PgBackend>>,
    /// Shared revision tracker — bumped by `index_workspace` after each successful ingest.
    pub revision_tracker: Arc<AtomicU64>,
    /// Optional `QualityStore` port (PR2 relocation: from
    /// `cognicode_explorer::ports::quality_repository::QualityRepository`
    /// to the unified `cognicode_core::domain::ports::QualityStore`).
    pub quality_store: Option<Arc<dyn cognicode_explorer::ports::QualityStore>>,
    /// Optional `ViewSpecStore` port (PR2 relocation from
    /// `cognicode_explorer::registry::ViewSpecStore` to
    /// `cognicode_core::domain::ports::ViewSpecStore`).
    pub view_spec_store: Option<Arc<dyn cognicode_core::domain::ports::ViewSpecStore>>,
    /// Optional `CallGraphStore` port (e29-0-refactor-call-sites).
    /// Surfaces the save/load call-graph aggregate behind the domain
    /// port so consumers depend on `Arc<dyn CallGraphStore>` instead of
    /// a concrete backend.
    pub call_graph_store: Option<Arc<dyn cognicode_core::domain::ports::CallGraphStore>>,
}

/// `PgBackend` trait — abstracts the subset of storage operations the
/// runtime needs. Implemented by `LadybugPgBackend`.
pub trait PgBackend: Send + Sync {
    fn quality_store(&self) -> Option<Arc<dyn cognicode_core::domain::ports::QualityStore>>;
    fn view_spec_store(&self) -> Option<Arc<dyn cognicode_core::domain::ports::ViewSpecStore>>;
    fn call_graph_store(&self) -> Option<Arc<dyn cognicode_core::domain::ports::CallGraphStore>>;
}

/// `LadybugPgBackend` — implements `PgBackend` on top of the
/// `cognicode-ladybug` crate. Used when the runtime is built with
/// `--features ladybug` (the default).
pub struct LadybugPgBackend {
    quality_store: Option<Arc<dyn cognicode_core::domain::ports::QualityStore>>,
    view_spec_store: Option<Arc<dyn cognicode_core::domain::ports::ViewSpecStore>>,
    call_graph_store: Option<Arc<dyn cognicode_core::domain::ports::CallGraphStore>>,
}

impl LadybugPgBackend {
    pub fn new(
        quality_store: Option<Arc<dyn cognicode_core::domain::ports::QualityStore>>,
        view_spec_store: Option<Arc<dyn cognicode_core::domain::ports::ViewSpecStore>>,
        call_graph_store: Option<Arc<dyn cognicode_core::domain::ports::CallGraphStore>>,
    ) -> Self {
        Self {
            quality_store,
            view_spec_store,
            call_graph_store,
        }
    }
}

impl PgBackend for LadybugPgBackend {
    fn quality_store(&self) -> Option<Arc<dyn cognicode_core::domain::ports::QualityStore>> {
        self.quality_store.clone()
    }
    fn view_spec_store(&self) -> Option<Arc<dyn cognicode_core::domain::ports::ViewSpecStore>> {
        self.view_spec_store.clone()
    }
    fn call_graph_store(&self) -> Option<Arc<dyn cognicode_core::domain::ports::CallGraphStore>> {
        self.call_graph_store.clone()
    }
}

/// Build a Runtime with an explicit `&dyn PgBackend`. The canonical
/// entry point (e29-2-final-cutover / e29-7 full postgres removal):
/// the runtime no longer requires a live PG; the backend exposes the
/// relocated domain ports.
///
/// `graph` stays `None` (ladybug limitation — the symbol repository is
/// backed by an empty `CallGraph` so facade wiring stays functional).
pub async fn bootstrap_with_backend(
    cwd: std::path::PathBuf,
    backend: std::sync::Arc<dyn PgBackend>,
) -> Result<Runtime, anyhow::Error> {
    // Best-effort tracing init. `try_init` (not `init`) so repeated
    // calls from tests and multiple entry points don't panic on
    // double-init.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    let source_reader: Arc<dyn cognicode_explorer::ports::SourceReader> = Arc::new(
        cognicode_explorer::adapters::FsSourceReader::new(cwd.clone()),
    );

    let graph_cache =
        Arc::new(cognicode_core::infrastructure::graph::graph_cache::GraphCache::new());

    // The 3 relocated ports are built from the backend's port
    // accessors.
    let quality_store = backend.quality_store();
    let view_spec_store = backend.view_spec_store();
    let call_graph_store = backend.call_graph_store();

    // graph=None degraded: ladybug path uses no graph. The symbol
    // repository is backed by an empty CallGraph so the facade wiring
    // stays functional (searches return empty results).
    let graph: Option<Arc<cognicode_core::domain::aggregates::CallGraph>> = None;
    let empty_graph = Arc::new(cognicode_core::domain::aggregates::CallGraph::new());
    let symbol_repo: Arc<dyn cognicode_explorer::ports::SymbolRepository> =
        Arc::new(cognicode_explorer::adapters::CallGraphRepository::new(empty_graph));

    Ok(Runtime {
        symbol_repo,
        source_reader,
        graph,
        cwd,
        graph_cache,
        backend: Some(backend),
        revision_tracker: Arc::new(AtomicU64::new(1)),
        quality_store,
        view_spec_store,
        call_graph_store,
    })
}

/// Build a Runtime without a backend (no ports wired). Retained as a
/// minimal entry point for binaries that only need the in-memory
/// surface; `bootstrap_with_backend` is the canonical path.
pub async fn bootstrap(cwd: std::path::PathBuf) -> Result<Runtime, anyhow::Error> {
    // Best-effort tracing init.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    let source_reader: Arc<dyn cognicode_explorer::ports::SourceReader> = Arc::new(
        cognicode_explorer::adapters::FsSourceReader::new(cwd.clone()),
    );

    let graph_cache =
        Arc::new(cognicode_core::infrastructure::graph::graph_cache::GraphCache::new());

    let empty_graph = Arc::new(cognicode_core::domain::aggregates::CallGraph::new());
    let symbol_repo: Arc<dyn cognicode_explorer::ports::SymbolRepository> =
        Arc::new(cognicode_explorer::adapters::CallGraphRepository::new(empty_graph));

    Ok(Runtime {
        symbol_repo,
        source_reader,
        graph: None,
        cwd,
        graph_cache,
        backend: None,
        revision_tracker: Arc::new(AtomicU64::new(1)),
        quality_store: None,
        view_spec_store: None,
        call_graph_store: None,
    })
}

impl Runtime {
    /// Construct an `ApiState` with all ISP-segregated facade Arcs.
    ///
    /// This is the preferred constructor for the HTTP API binary.
    /// The `graph_query` port is created from `self.graph` on demand.
    pub fn into_api_state(self) -> cognicode_explorer::api::ApiState {
        use cognicode_core::domain::traits::GraphQueryPort;

        // Create the GraphQueryPort from the CallGraph (may be None).
        let graph_query: Option<Arc<dyn GraphQueryPort>> = self.graph.clone().map(|g| {
            Arc::new(cognicode_explorer::adapters::CallGraphRepository::new(g))
                as Arc<dyn GraphQueryPort>
        });

        // Workspace resolver — maps workspace_id → root_path.
        let ws_resolver =
            Arc::new(cognicode_core::application::ingest::StaticWorkspaceResolver::new());

        // Workspace facade.
        let workspace: Arc<dyn cognicode_explorer::facades::WorkspaceService> = Arc::new(
            cognicode_explorer::facades::workspace::WorkspaceServiceImpl::new(
                self.symbol_repo.clone(),
                self.cwd.clone(),
                Some(ws_resolver.clone()),
            ),
        );

        // Persistence facade — in-memory only (postgres removed).
        let persistence: Arc<dyn cognicode_explorer::facades::PersistenceService> = Arc::new(
            cognicode_explorer::facades::persistence::PersistenceServiceImpl::new(None),
        );

        // Quality facade — wired from the backend port when available.
        let quality: Option<Arc<dyn cognicode_explorer::ports::QualityStore>> =
            self.quality_store.clone();

        // Investigation facade — no postgres-backed store.
        let investigation: Option<Arc<dyn cognicode_explorer::facades::InvestigationFacade>> = None;

        // Graph repository for multimodal search. Read path is
        // ungated (default build); the in-memory repo is used.
        #[cfg(feature = "multimodal")]
        let graph_repo: Option<Arc<dyn cognicode_core::domain::ports::GraphRepository>> =
            Some(Arc::new(
                cognicode_explorer::adapters::InMemoryGraphRepository::new(vec![], vec![]),
            ));
        #[cfg(not(feature = "multimodal"))]
        let graph_repo: Option<Arc<dyn cognicode_core::domain::ports::GraphRepository>> =
            Some(Arc::new(
                cognicode_explorer::adapters::InMemoryGraphRepository::new(vec![], vec![]),
            ));

        let search: Arc<dyn cognicode_explorer::facades::SearchService> =
            Arc::new(cognicode_explorer::facades::search::SearchServiceImpl::new(
                self.symbol_repo.clone(),
                None, // search_repo
                Arc::new(cognicode_explorer::registry::ViewRegistry::new(None)),
                None, // view_spec_store
                quality.clone(),
                Some(persistence.clone()),
                investigation.clone(),
                graph_repo.clone(),
            ));

        // View facade.
        let view_impl: Arc<cognicode_explorer::facades::view::ViewServiceImpl> =
            Arc::new(cognicode_explorer::facades::view::ViewServiceImpl::new(
                self.symbol_repo.clone(),
                self.source_reader.clone(),
                quality.clone(),
                cognicode_explorer::domain::lens::default_registry(),
                graph_query.clone(),
                Arc::new(cognicode_explorer::registry::ViewRegistry::new(None)),
                Some(persistence.clone()),
                graph_repo.clone(),
            ));
        let view: Arc<dyn cognicode_explorer::facades::ViewService> = view_impl.clone();
        let lens_executor: Arc<dyn cognicode_explorer::facades::LensService> = view_impl;

        let moldql: Arc<dyn cognicode_explorer::facades::MoldQLService> =
            Arc::new(cognicode_explorer::facades::moldql::MoldQLServiceImpl::new(
                self.symbol_repo.clone(),
                quality,
                self.source_reader.clone(),
                lens_executor,
                #[cfg(feature = "multimodal")]
                None, // graph_repo
                None, // graph_executor
                Some("default".to_string()),
                Some(1),
            ));

        // Graph facade.
        let graph: Arc<dyn cognicode_explorer::facades::GraphService> =
            Arc::new(cognicode_explorer::facades::graph::GraphServiceImpl::new(
                self.symbol_repo.clone(),
                graph_query,
            ));

        let mut state = cognicode_explorer::api::ApiState::new(
            workspace,
            search,
            view,
            persistence,
            moldql,
            graph,
        );

        #[cfg(feature = "multimodal")]
        {
            let snapshot_service =
                Arc::new(cognicode_explorer::domain::snapshot::SnapshotService::new());
            state = state.with_snapshot(snapshot_service);
        }

        // Wire the shared revision tracker so MoldQL REST endpoints can pin queries.
        state = state.with_revision_tracker(self.revision_tracker.clone());

        state
    }

    pub fn into_mcp_handler(self) -> cognicode_explorer::mcp::ExplorerMcpHandler {
        let view_registry = Arc::new(cognicode_explorer::registry::ViewRegistry::new(None));
        let lens_registry = cognicode_explorer::domain::lens::default_registry();

        let quality: Option<Arc<dyn cognicode_explorer::ports::QualityStore>> =
            self.quality_store.clone();
        let quality_write: Option<Arc<dyn cognicode_explorer::ports::QualityStore>> =
            self.quality_store.clone();

        cognicode_explorer::mcp::ExplorerMcpHandler::with_graph(
            self.symbol_repo,
            self.source_reader,
            view_registry,
            lens_registry,
            self.cwd,
            self.graph,
            quality,
            quality_write,
            self.revision_tracker,
            None, // route_store — postgres-backed adapter removed
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{bootstrap_with_backend, LadybugPgBackend};
    use cognicode_core::domain::aggregates::CallGraph;
    use cognicode_core::domain::ports::{CallGraphStore, QualityStore, ViewSpecStore};
    use cognicode_core::domain::value_objects::{RevisionId, WorkspaceId};

    /// Identity stub for [`QualityStore`] — never called in this test,
    /// only held and compared by Arc identity.
    struct TestQualityStore;
    impl QualityStore for TestQualityStore {
        fn issues_for_file(
            &self,
            _file: &str,
        ) -> Result<
            Vec<cognicode_core::domain::ports::QualityIssue>,
            cognicode_core::domain::ports::QualityError,
        > {
            unimplemented!()
        }
        fn issues_for_scope(
            &self,
            _scope_prefix: &str,
        ) -> Result<
            Vec<cognicode_core::domain::ports::QualityIssue>,
            cognicode_core::domain::ports::QualityError,
        > {
            unimplemented!()
        }
        fn issues_at_line(
            &self,
            _file: &str,
            _line: u32,
        ) -> Result<
            Vec<cognicode_core::domain::ports::QualityIssue>,
            cognicode_core::domain::ports::QualityError,
        > {
            unimplemented!()
        }
        fn issue_by_id(
            &self,
            _id: i64,
        ) -> Result<
            Option<cognicode_core::domain::ports::QualityIssue>,
            cognicode_core::domain::ports::QualityError,
        > {
            unimplemented!()
        }
        fn rule_summary(
            &self,
            _rule_id: &str,
        ) -> Result<
            cognicode_core::domain::ports::RuleSummary,
            cognicode_core::domain::ports::QualityError,
        > {
            unimplemented!()
        }
        fn quality_gate(
            &self,
            _workspace_id: Option<&str>,
        ) -> Result<
            cognicode_core::domain::ports::QualityGateSummary,
            cognicode_core::domain::ports::QualityError,
        > {
            unimplemented!()
        }
        fn open_issues_count(
            &self,
            _workspace_id: Option<&str>,
        ) -> Result<usize, cognicode_core::domain::ports::QualityError> {
            unimplemented!()
        }
        fn issues_for_workspace(
            &self,
            _workspace_id: Option<&str>,
            _filter: &cognicode_core::domain::ports::IssueFilter,
        ) -> Result<
            Vec<cognicode_core::domain::ports::QualityIssue>,
            cognicode_core::domain::ports::QualityError,
        > {
            unimplemented!()
        }
        fn insert_issues(
            &self,
            _issues: &[cognicode_core::domain::ports::NewIssue],
        ) -> Result<
            cognicode_core::domain::ports::UpsertSummary,
            cognicode_core::domain::ports::QualityError,
        > {
            unimplemented!()
        }
        fn delete_issue(
            &self,
            _workspace_id: &str,
            _rule_id: &str,
            _file_path: &str,
            _line: u32,
        ) -> Result<bool, cognicode_core::domain::ports::QualityError> {
            unimplemented!()
        }
    }

    /// Identity stub for [`ViewSpecStore`] — never called in this test.
    struct TestViewSpecStore;
    #[async_trait::async_trait]
    impl ViewSpecStore for TestViewSpecStore {
        async fn save(
            &self,
            _payload: &cognicode_core::domain::ports::ViewSpecPayload,
            _workspace_id: &str,
            _owner: &str,
        ) -> Result<(), cognicode_core::domain::ports::ViewSpecStoreError> {
            unimplemented!()
        }
        async fn load(
            &self,
            _id: &str,
            _workspace_id: &str,
            _owner: &str,
        ) -> Result<
            Option<cognicode_core::domain::ports::ViewSpecPayload>,
            cognicode_core::domain::ports::ViewSpecStoreError,
        > {
            unimplemented!()
        }
        async fn list(
            &self,
            _workspace_id: &str,
            _owner: &str,
        ) -> Result<
            Vec<cognicode_core::domain::ports::ViewSpecPayload>,
            cognicode_core::domain::ports::ViewSpecStoreError,
        > {
            unimplemented!()
        }
        async fn delete(
            &self,
            _id: &str,
            _workspace_id: &str,
            _owner: &str,
        ) -> Result<bool, cognicode_core::domain::ports::ViewSpecStoreError> {
            unimplemented!()
        }
        async fn list_for_workspace(
            &self,
            _workspace_id: &str,
            _applies_to_kind: &str,
        ) -> Result<
            Vec<cognicode_core::domain::ports::ViewSpecPayload>,
            cognicode_core::domain::ports::ViewSpecStoreError,
        > {
            unimplemented!()
        }
        async fn update(
            &self,
            _id: &str,
            _workspace_id: &str,
            _owner: &str,
            _seed_object_id: Option<&str>,
            _seed_view_id: Option<&str>,
            _applies_when: Option<&str>,
        ) -> Result<bool, cognicode_core::domain::ports::ViewSpecStoreError> {
            unimplemented!()
        }
    }

    /// Identity stub for [`CallGraphStore`] — never called in this test.
    struct TestCallGraphStore;
    #[async_trait::async_trait]
    impl CallGraphStore for TestCallGraphStore {
        async fn save_call_graph_ws(
            &self,
            _graph: &CallGraph,
            _ws: &WorkspaceId,
        ) -> Result<RevisionId, cognicode_core::domain::ports::CallGraphError> {
            unimplemented!()
        }
        async fn load_call_graph_ws(
            &self,
            _ws: &WorkspaceId,
            _revision: RevisionId,
        ) -> Result<Option<CallGraph>, cognicode_core::domain::ports::CallGraphError> {
            unimplemented!()
        }
        async fn load_call_graph_current(
            &self,
            _ws: &WorkspaceId,
        ) -> Result<Option<CallGraph>, cognicode_core::domain::ports::CallGraphError> {
            unimplemented!()
        }
    }

    /// The 3 relocated ports are populated FROM the backend's port
    /// accessors — identity preserved (same Arc).
    #[tokio::test]
    async fn bootstrap_with_backend_populates_ports_from_backend() {
        let quality: Arc<dyn QualityStore> = Arc::new(TestQualityStore);
        let view_spec: Arc<dyn ViewSpecStore> = Arc::new(TestViewSpecStore);
        let cg_store: Arc<dyn CallGraphStore> = Arc::new(TestCallGraphStore);
        let backend = Arc::new(LadybugPgBackend::new(
            Some(quality.clone()),
            Some(view_spec.clone()),
            Some(cg_store.clone()),
        ));

        let runtime = bootstrap_with_backend(std::env::temp_dir(), backend)
            .await
            .expect("bootstrap_with_backend succeeds with a provided backend");

        assert!(
            Arc::ptr_eq(runtime.quality_store.as_ref().unwrap(), &quality),
            "quality_store must be the SAME Arc the backend returned"
        );
        assert!(
            Arc::ptr_eq(runtime.view_spec_store.as_ref().unwrap(), &view_spec),
            "view_spec_store must be the SAME Arc the backend returned"
        );
        assert!(
            Arc::ptr_eq(runtime.call_graph_store.as_ref().unwrap(), &cg_store),
            "call_graph_store must be the SAME Arc the backend returned"
        );
    }
}
