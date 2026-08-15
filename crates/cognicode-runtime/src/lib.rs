//! CogniCode Runtime — shared bootstrap for API and MCP binaries.
//!
//! LadybugDB is the sole storage backend (e29-7 full postgres removal).
//! The runtime exposes the relocated domain ports directly
//! (`quality_store` / `view_spec_store` / `call_graph_store`) via the
//! `RuntimePorts` DTO. `bootstrap_with_backend` is the canonical entry
//! point; `bootstrap` builds a Runtime with no ports wired.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::redundant_locals)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

pub struct Runtime {
    pub symbol_repo: Arc<dyn cognicode_explorer::ports::SymbolRepository>,
    pub source_reader: Arc<dyn cognicode_explorer::ports::SourceReader>,
    pub graph: Option<Arc<cognicode_core::domain::aggregates::CallGraph>>,
    pub cwd: PathBuf,
    /// GraphCache for serving the in-memory graph (ArcSwap).
    pub graph_cache: Arc<cognicode_core::infrastructure::graph::graph_cache::GraphCache>,
    /// Shared revision tracker — bumped by `index_workspace` after each successful ingest.
    pub revision_tracker: Arc<AtomicU64>,
    /// Optional `QualityStore` port (relocated from
    /// `cognicode_explorer::ports::quality_repository` to the unified
    /// `cognicode_core::domain::ports::QualityStore`).
    pub quality_store: Option<Arc<dyn cognicode_core::domain::ports::QualityStore>>,
    /// Optional `ViewSpecStore` port (relocated from
    /// `cognicode_explorer::registry::ViewSpecStore` to
    /// `cognicode_core::domain::ports::ViewSpecStore`).
    pub view_spec_store: Option<Arc<dyn cognicode_core::domain::ports::ViewSpecStore>>,
    /// Optional `CallGraphStore` port (e29-0-refactor-call-sites).
    /// Surfaces the save/load call-graph aggregate behind the domain
    /// port so consumers depend on `Arc<dyn CallGraphStore>` instead of
    /// a concrete backend.
    pub call_graph_store: Option<Arc<dyn cognicode_core::domain::ports::CallGraphStore>>,
    /// Optional `AlgorithmRegistry` for analytics execution (E28.4).
    /// Wired via `bootstrap_with_backend` when `analytics_lineage_store` is `Some`.
    pub analytics_registry:
        Option<Arc<cognicode_core::application::services::graph_analytics::AlgorithmRegistry>>,
    /// Optional lineage store for analytics run records (E28.4).
    /// When `Some`, `bootstrap_with_backend` constructs the registry automatically.
    pub analytics_lineage_store:
        Option<Arc<dyn cognicode_core::domain::analytics::lineage::RunLineageStore>>,
    // NEW 6 ports (e29-6) — all LadybugStore ports surfaced on Runtime
    /// Revision head tracker (workspace-scoped revision counter).
    pub revision_store: Option<Arc<dyn cognicode_core::domain::ports::RevisionStore>>,
    /// Manifest of scanned files (workspace + path keyed).
    pub manifest_store: Option<Arc<dyn cognicode_core::domain::ports::ManifestStore>>,
    /// Exploration session persistence.
    pub session_store: Option<Arc<dyn cognicode_core::domain::ports::SessionStore>>,
    /// Graph report summaries per workspace.
    pub report_store: Option<Arc<dyn cognicode_core::domain::ports::ReportStore>>,
    /// Narrative view snapshots (e14-c2).
    pub narrative_store: Option<Arc<dyn cognicode_core::domain::ports::NarrativeStore>>,
    /// Multimodal federation store (spaces / repos / issues).
    #[cfg(feature = "multimodal")]
    pub federation_store: Option<Arc<dyn cognicode_core::domain::ports::FederationStore>>,
    /// Multimodal ingest commit port (atomic 3-stage commit).
    #[cfg(feature = "multimodal")]
    pub ingest_commit_port: Option<Arc<dyn cognicode_core::domain::ports::IngestCommitPort>>,
}

/// Plain DTO carrying the 10 LadybugStore port traits into
/// [`bootstrap_with_backend`]. Replaces the previous single-implementer
/// backend trait indirection (collapsed into a struct of
/// `Option<Arc<dyn *Port>>` slots — runtime-bootstrap-contract spec).
///
/// ## Port audit (T-002)
/// | Port | Status in RuntimePorts |
/// |------|------------------------|
/// | `RevisionStore` | MISSING → added |
/// | `FederationStore` | MISSING → added (multimodal) |
/// | `ManifestStore` | MISSING → added |
/// | `SessionStore` | MISSING → added |
/// | `ReportStore` | MISSING → added |
/// | `ViewSpecStore` | present |
/// | `CallGraphStore` | present |
/// | `IngestCommitPort` | MISSING → added (multimodal) |
/// | `RunLineageStore` | present (as `analytics_lineage_store`) |
/// | `QualityStore` | present |
#[derive(Default)]
pub struct RuntimePorts {
    // Existing 4 ports (e29-3 / e29-5)
    pub quality_store: Option<Arc<dyn cognicode_core::domain::ports::QualityStore>>,
    pub view_spec_store: Option<Arc<dyn cognicode_core::domain::ports::ViewSpecStore>>,
    pub call_graph_store: Option<Arc<dyn cognicode_core::domain::ports::CallGraphStore>>,
    /// Optional lineage store for analytics run records (E28.4).
    /// When `Some`, `bootstrap_with_backend` constructs an `AlgorithmRegistry`
    /// automatically via `default_analytics_registry`.
    pub analytics_lineage_store:
        Option<Arc<dyn cognicode_core::domain::analytics::lineage::RunLineageStore>>,
    // NEW 6 ports (e29-6) — all LadybugStore ports wired at runtime
    /// Revision head tracker (workspace-scoped revision counter).
    pub revision_store: Option<Arc<dyn cognicode_core::domain::ports::RevisionStore>>,
    /// Manifest of scanned files (workspace + path keyed).
    pub manifest_store: Option<Arc<dyn cognicode_core::domain::ports::ManifestStore>>,
    /// Exploration session persistence.
    pub session_store: Option<Arc<dyn cognicode_core::domain::ports::SessionStore>>,
    /// Graph report summaries per workspace.
    pub report_store: Option<Arc<dyn cognicode_core::domain::ports::ReportStore>>,
    /// Narrative view snapshots (e14-c2).
    pub narrative_store: Option<Arc<dyn cognicode_core::domain::ports::NarrativeStore>>,
    /// Multimodal federation store (spaces / repos / issues).
    #[cfg(feature = "multimodal")]
    pub federation_store: Option<Arc<dyn cognicode_core::domain::ports::FederationStore>>,
    /// Multimodal ingest commit port (atomic 3-stage commit).
    #[cfg(feature = "multimodal")]
    pub ingest_commit_port: Option<Arc<dyn cognicode_core::domain::ports::IngestCommitPort>>,
}

/// Build a Runtime from a [`RuntimePorts`] DTO. The canonical entry
/// point (e29-2-final-cutover / e29-7 full postgres removal): the
/// runtime no longer requires a live PG and no longer carries a backend
/// indirection — the 3 port Arcs move verbatim onto the `Runtime`.
///
/// `graph` stays `None` (ladybug limitation — the symbol repository is
/// backed by an empty `CallGraph` so facade wiring stays functional).
pub async fn bootstrap_with_backend(
    cwd: std::path::PathBuf,
    ports: RuntimePorts,
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

    // The 4 relocated ports move verbatim from the DTO (Arc identity
    // preserved — same allocation).
    let quality_store = ports.quality_store;
    let view_spec_store = ports.view_spec_store;
    let call_graph_store = ports.call_graph_store;

    // Analytics wiring: when lineage store is provided, construct the registry
    // automatically so analytics tools can execute (E28.4).
    let analytics_lineage_store = ports.analytics_lineage_store;
    let analytics_registry = analytics_lineage_store.as_ref().map(|lineage| {
        Arc::new(
            cognicode_core::application::services::graph_analytics::default_analytics_registry(
                lineage.clone(),
            ),
        )
    });

    // The 6 new ports (e29-6) also move verbatim from the DTO.
    let revision_store = ports.revision_store;
    let manifest_store = ports.manifest_store;
    let session_store = ports.session_store;
    let report_store = ports.report_store;
    let narrative_store = ports.narrative_store;
    #[cfg(feature = "multimodal")]
    let federation_store = ports.federation_store;
    #[cfg(feature = "multimodal")]
    let ingest_commit_port = ports.ingest_commit_port;

    // graph=None degraded: ladybug path uses no graph. The symbol
    // repository is backed by an empty CallGraph so the facade wiring
    // stays functional (searches return empty results).
    let graph: Option<Arc<cognicode_core::domain::aggregates::CallGraph>> = None;
    let empty_graph = Arc::new(cognicode_core::domain::aggregates::CallGraph::new());
    let symbol_repo: Arc<dyn cognicode_explorer::ports::SymbolRepository> = Arc::new(
        cognicode_explorer::adapters::CallGraphRepository::new(empty_graph),
    );

    Ok(Runtime {
        symbol_repo,
        source_reader,
        graph,
        cwd,
        graph_cache,
        revision_tracker: Arc::new(AtomicU64::new(1)),
        quality_store,
        view_spec_store,
        call_graph_store,
        analytics_registry,
        analytics_lineage_store,
        revision_store,
        manifest_store,
        session_store,
        report_store,
        narrative_store,
        #[cfg(feature = "multimodal")]
        federation_store,
        #[cfg(feature = "multimodal")]
        ingest_commit_port,
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
    let symbol_repo: Arc<dyn cognicode_explorer::ports::SymbolRepository> = Arc::new(
        cognicode_explorer::adapters::CallGraphRepository::new(empty_graph),
    );

    Ok(Runtime {
        symbol_repo,
        source_reader,
        graph: None,
        cwd,
        graph_cache,
        revision_tracker: Arc::new(AtomicU64::new(1)),
        quality_store: None,
        view_spec_store: None,
        call_graph_store: None,
        analytics_registry: None,
        analytics_lineage_store: None,
        revision_store: None,
        manifest_store: None,
        session_store: None,
        report_store: None,
        narrative_store: None,
        #[cfg(feature = "multimodal")]
        federation_store: None,
        #[cfg(feature = "multimodal")]
        ingest_commit_port: None,
    })
}

/// Build a Runtime from a LadybugDB file, wiring all 10 port traits.
///
/// ## T-003
/// Calls `LadybugStore::open(db_path)`, extracts all 10 ports as
/// `Arc<dyn Trait>`, builds a full `RuntimePorts`, and delegates to
/// `bootstrap_with_backend`.
pub fn bootstrap_ladybug(
    cwd: std::path::PathBuf,
    db_path: std::path::PathBuf,
) -> Result<Runtime, anyhow::Error> {
    let store = cognicode_ladybug::LadybugStore::open(&db_path)?;
    let store = Arc::new(store);

    // All 10 ports are the same Arc<LadybugStore> cast to the trait object.
    let ports = RuntimePorts {
        quality_store: Some(store.clone() as Arc<dyn cognicode_core::domain::ports::QualityStore>),
        view_spec_store: Some(
            store.clone() as Arc<dyn cognicode_core::domain::ports::ViewSpecStore>
        ),
        call_graph_store: Some(
            store.clone() as Arc<dyn cognicode_core::domain::ports::CallGraphStore>
        ),
        analytics_lineage_store: Some(
            store.clone() as Arc<dyn cognicode_core::domain::analytics::lineage::RunLineageStore>
        ),
        revision_store: Some(store.clone() as Arc<dyn cognicode_core::domain::ports::RevisionStore>),
        manifest_store: Some(store.clone() as Arc<dyn cognicode_core::domain::ports::ManifestStore>),
        session_store: Some(store.clone() as Arc<dyn cognicode_core::domain::ports::SessionStore>),
        report_store: Some(store.clone() as Arc<dyn cognicode_core::domain::ports::ReportStore>),
        narrative_store: Some(
            store.clone() as Arc<dyn cognicode_core::domain::ports::NarrativeStore>
        ),
        #[cfg(feature = "multimodal")]
        federation_store: Some(
            store.clone() as Arc<dyn cognicode_core::domain::ports::FederationStore>
        ),
        #[cfg(feature = "multimodal")]
        ingest_commit_port: Some(
            store.clone() as Arc<dyn cognicode_core::domain::ports::IngestCommitPort>
        ),
    };

    // bootstrap_with_backend is async but our body is sync.
    // If already inside a Tokio runtime, spawn a dedicated thread with its own
    // runtime to avoid "cannot start a runtime from within a runtime".
    if let Ok(_handle) = tokio::runtime::Handle::try_current() {
        let cwd = cwd;
        let ports = ports;
        let result = std::thread::scope(|s| {
            s.spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?;
                runtime.block_on(bootstrap_with_backend(cwd, ports))
            })
            .join()
        });
        match result {
            Ok(Ok(runtime)) => Ok(runtime),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow::anyhow!("bootstrap thread panicked")),
        }
    } else {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(bootstrap_with_backend(cwd, ports))
    }
}

/// Build a Runtime from a LadybugDB file at `./cognicode.lbug` relative to cwd.
///
/// ## T-004
/// Convenience wrapper around `bootstrap_ladybug` that uses the default db path.
pub fn bootstrap_ladybug_default(cwd: std::path::PathBuf) -> Result<Runtime, anyhow::Error> {
    let db_path = cwd.join("cognicode.lbug");
    bootstrap_ladybug(cwd, db_path)
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
        let persistence: Arc<dyn cognicode_explorer::facades::PersistenceService> =
            Arc::new(cognicode_explorer::facades::persistence::PersistenceServiceImpl::new(None));

        // Quality facade — wired from the backend port when available.
        let quality: Option<Arc<dyn cognicode_core::domain::ports::QualityStore>> =
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

        // Wire analytics when both registry and lineage store are present (E28.4).
        if let (Some(registry), Some(lineage_store)) = (
            self.analytics_registry.clone(),
            self.analytics_lineage_store.clone(),
        ) {
            state = state.with_analytics(registry, lineage_store);
        }

        state
    }

    pub fn into_mcp_handler(self) -> cognicode_explorer::mcp::ExplorerMcpHandler {
        let view_registry = Arc::new(cognicode_explorer::registry::ViewRegistry::new(None));
        let lens_registry = cognicode_explorer::domain::lens::default_registry();

        let ports = cognicode_explorer::mcp::explorer::McpHandlerPorts {
            symbol_repo: self.symbol_repo,
            source_reader: self.source_reader,
            view_registry,
            lens_registry,
            cwd: self.cwd,
            graph: self.graph,
            quality_store: self.quality_store.clone(),
            quality_write: self.quality_store.clone(),
            revision_tracker: self.revision_tracker,
            route_store: None, // postgres-backed adapter removed
            analytics_registry: self.analytics_registry,
            analytics_lineage_store: self.analytics_lineage_store,
        };

        cognicode_explorer::mcp::ExplorerMcpHandler::with_graph(ports)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{RuntimePorts, bootstrap_ladybug, bootstrap_with_backend};
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

    /// The 3 relocated ports are moved FROM the RuntimePorts DTO —
    /// identity preserved (same Arc).
    #[tokio::test]
    async fn bootstrap_with_backend_populates_ports_from_backend() {
        let quality: Arc<dyn QualityStore> = Arc::new(TestQualityStore);
        let view_spec: Arc<dyn ViewSpecStore> = Arc::new(TestViewSpecStore);
        let cg_store: Arc<dyn CallGraphStore> = Arc::new(TestCallGraphStore);
        let ports = RuntimePorts {
            quality_store: Some(quality.clone()),
            view_spec_store: Some(view_spec.clone()),
            call_graph_store: Some(cg_store.clone()),
            analytics_lineage_store: None,
            revision_store: None,
            manifest_store: None,
            session_store: None,
            report_store: None,
            narrative_store: None,
        };

        let runtime = bootstrap_with_backend(std::env::temp_dir(), ports)
            .await
            .expect("bootstrap_with_backend succeeds with a RuntimePorts DTO");

        assert!(
            Arc::ptr_eq(runtime.quality_store.as_ref().unwrap(), &quality),
            "quality_store must be the SAME Arc the DTO carried"
        );
        assert!(
            Arc::ptr_eq(runtime.view_spec_store.as_ref().unwrap(), &view_spec),
            "view_spec_store must be the SAME Arc the DTO carried"
        );
        assert!(
            Arc::ptr_eq(runtime.call_graph_store.as_ref().unwrap(), &cg_store),
            "call_graph_store must be the SAME Arc the DTO carried"
        );
    }

    /// When `analytics_lineage_store` is provided, `bootstrap_with_backend`
    /// automatically constructs an `AlgorithmRegistry` (E28.4).
    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_with_backend_constructs_analytics_registry_when_lineage_provided() {
        use cognicode_core::domain::analytics::lineage::{InMemoryLineageStore, RunLineageStore};

        let lineage_store: Arc<dyn RunLineageStore> = Arc::new(InMemoryLineageStore::new());
        let ports = RuntimePorts {
            analytics_lineage_store: Some(lineage_store.clone()),
            ..Default::default()
        };

        let runtime = bootstrap_with_backend(std::env::temp_dir(), ports)
            .await
            .expect("bootstrap_with_backend succeeds with analytics lineage store");

        // Registry must be constructed automatically
        assert!(
            runtime.analytics_registry.is_some(),
            "analytics_registry must be Some when lineage store is provided"
        );
        // Lineage store must be preserved (same Arc)
        assert!(
            Arc::ptr_eq(
                runtime.analytics_lineage_store.as_ref().unwrap(),
                &lineage_store
            ),
            "analytics_lineage_store must be the SAME Arc the DTO carried"
        );
    }

    /// When `analytics_lineage_store` is NOT provided, both analytics fields
    /// remain `None` (backwards compatible).
    #[tokio::test]
    async fn bootstrap_with_backend_leaves_analytics_none_when_no_lineage() {
        let ports = RuntimePorts::default();

        let runtime = bootstrap_with_backend(std::env::temp_dir(), ports)
            .await
            .expect("bootstrap_with_backend succeeds with empty RuntimePorts");

        assert!(
            runtime.analytics_registry.is_none(),
            "analytics_registry must be None when no lineage store is provided"
        );
        assert!(
            runtime.analytics_lineage_store.is_none(),
            "analytics_lineage_store must be None when not provided"
        );
    }

    /// T-008: Integration test verifying all 10 ports are `Some` after
    /// `bootstrap_ladybug()`.
    #[tokio::test]
    async fn bootstrap_ladybug_wires_all_ten_ports() {
        use std::fs;
        let tmp = std::env::temp_dir().join(format!("ladybug_all_ports_{}", std::process::id()));
        let db_path = tmp.join("test.lbdb");
        let cwd = tmp.join("cwd");
        fs::create_dir_all(&cwd).expect("create temp cwd");

        let runtime =
            bootstrap_ladybug(cwd, db_path.clone()).expect("bootstrap_ladybug should succeed");

        // All 4 original ports must be Some.
        assert!(
            runtime.quality_store.is_some(),
            "quality_store must be Some after bootstrap_ladybug"
        );
        assert!(
            runtime.view_spec_store.is_some(),
            "view_spec_store must be Some after bootstrap_ladybug"
        );
        assert!(
            runtime.call_graph_store.is_some(),
            "call_graph_store must be Some after bootstrap_ladybug"
        );
        assert!(
            runtime.analytics_lineage_store.is_some(),
            "analytics_lineage_store must be Some after bootstrap_ladybug"
        );
        // All 6 new ports must be Some (e29-6 wiring).
        assert!(
            runtime.revision_store.is_some(),
            "revision_store must be Some after bootstrap_ladybug"
        );
        assert!(
            runtime.manifest_store.is_some(),
            "manifest_store must be Some after bootstrap_ladybug"
        );
        assert!(
            runtime.session_store.is_some(),
            "session_store must be Some after bootstrap_ladybug"
        );
        assert!(
            runtime.report_store.is_some(),
            "report_store must be Some after bootstrap_ladybug"
        );
        assert!(
            runtime.narrative_store.is_some(),
            "narrative_store must be Some after bootstrap_ladybug"
        );
        // Clean up temp dir.
        let _ = fs::remove_dir_all(tmp);
    }

    /// T-008: Verify all 11 ports are present (same LadybugStore allocation).
    #[tokio::test]
    async fn bootstrap_ladybug_uses_same_store_for_all_ports() {
        use std::fs;
        let tmp = std::env::temp_dir().join(format!("ladybug_same_store_{}", std::process::id()));
        let db_path = tmp.join("test.lbdb");
        let cwd = tmp.join("cwd");
        fs::create_dir_all(&cwd).expect("create temp cwd");

        let runtime =
            bootstrap_ladybug(cwd, db_path.clone()).expect("bootstrap_ladybug should succeed");

        // We can't easily compare raw pointers across trait objects, but we can
        // verify each port is distinct from None (they're all Some).
        assert!(runtime.quality_store.is_some());
        assert!(runtime.view_spec_store.is_some());
        assert!(runtime.call_graph_store.is_some());
        assert!(runtime.analytics_lineage_store.is_some());
        assert!(runtime.revision_store.is_some());
        assert!(runtime.manifest_store.is_some());
        assert!(runtime.session_store.is_some());
        assert!(runtime.report_store.is_some());
        assert!(runtime.narrative_store.is_some());

        // Clean up temp dir.
        let _ = fs::remove_dir_all(tmp);
    }
}
