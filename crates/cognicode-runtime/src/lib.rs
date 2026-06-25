//! CogniCode Runtime — shared bootstrap for API and MCP binaries.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[cfg(feature = "postgres")]
use cognicode_core::infrastructure::{
    graph::graph_cache::GraphCache, persistence::PostgresRepository,
};

pub struct Runtime {
    pub symbol_repo: Arc<dyn cognicode_explorer::ports::SymbolRepository>,
    pub source_reader: Arc<dyn cognicode_explorer::ports::SourceReader>,
    pub graph: Option<Arc<cognicode_core::domain::aggregates::CallGraph>>,
    pub cwd: PathBuf,
    /// GraphCache for serving the in-memory graph (ArcSwap).
    pub graph_cache: Arc<cognicode_core::infrastructure::graph::graph_cache::GraphCache>,
    /// PostgresRepository for the ingest pipeline (PG-connected Mode B only).
    #[cfg(feature = "postgres")]
    pub pg_repo: Option<Arc<cognicode_core::infrastructure::persistence::PostgresRepository>>,
}

impl Runtime {
    pub async fn bootstrap(cwd: PathBuf, postgres_url: Option<String>) -> Result<Self> {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();

        let source_reader: Arc<dyn cognicode_explorer::ports::SourceReader> = Arc::new(
            cognicode_explorer::adapters::FsSourceReader::new(cwd.clone()),
        );

        let graph_cache =
            Arc::new(cognicode_core::infrastructure::graph::graph_cache::GraphCache::new());

        let pg_url = postgres_url.clone();
        let graph: Option<Arc<cognicode_core::domain::aggregates::CallGraph>> = match &pg_url {
            #[cfg(feature = "postgres")]
            Some(url) => {
                let graph =
                    cognicode_explorer::postgres_bridge::open_graph_from_postgres(url).await?;
                graph_cache.set((*graph).clone());
                Some(graph)
            }
            #[cfg(not(feature = "postgres"))]
            Some(_) => unreachable!("postgres feature not enabled"),
            None => None,
        };

        #[cfg(feature = "postgres")]
        let pg_repo: Option<
            Arc<cognicode_core::infrastructure::persistence::PostgresRepository>,
        > = if let Some(ref url) = pg_url {
            let repo = cognicode_core::infrastructure::persistence::PostgresRepository::new(url)
                .await
                .map_err(|e| anyhow::anyhow!("PG connect: {e}"))?;
            Some(Arc::new(repo))
        } else {
            None
        };

        let symbol_repo: Arc<dyn cognicode_explorer::ports::SymbolRepository> =
            if let Some(ref g) = graph {
                Arc::new(cognicode_explorer::adapters::CallGraphRepository::new(
                    g.clone(),
                ))
            } else {
                return Err(anyhow::anyhow!(
                    "cognicode-runtime requires --postgres <URL> or DATABASE_URL to be set. \
                     Set DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode \
                     or pass --postgres <URL>"
                ));
            };

        Ok(Self {
            symbol_repo,
            source_reader,
            graph,
            cwd,
            graph_cache,
            #[cfg(feature = "postgres")]
            pg_repo,
        })
    }

    /// Construct an `ApiState` with all 6 ISP-segregated facade Arcs.
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
        // Populated when open_workspace is called.
        let ws_resolver: Arc<dyn cognicode_core::application::ingest::WorkspaceResolver> =
            Arc::new(cognicode_core::application::ingest::StaticWorkspaceResolver::new());

        // IngestController — only when PG is available.
        #[cfg(feature = "postgres")]
        let ingest = self.pg_repo.as_ref().map(|repo| {
            Arc::new(cognicode_core::application::ingest::IngestController::new(
                repo.clone(),
                self.graph_cache.clone(),
                ws_resolver.clone(),
            ))
        });
        #[cfg(not(feature = "postgres"))]
        let ingest: Option<Arc<cognicode_core::application::ingest::IngestController>> = None;

        // Workspace facade.
        let workspace: Arc<dyn cognicode_explorer::facades::WorkspaceService> = Arc::new(
            cognicode_explorer::facades::workspace::WorkspaceServiceImpl::new(
                self.symbol_repo.clone(),
                self.cwd.clone(),
            ),
        );

        // Search facade.
        let view_registry = Arc::new(cognicode_explorer::registry::ViewRegistry::new(None));
        let view_registry_for_search = view_registry.clone();
        #[cfg(feature = "postgres")]
        let quality = quality_repo_arc(self.pg_repo.as_ref());
        #[cfg(not(feature = "postgres"))]
        let quality = quality_repo_arc();
        let search: Arc<dyn cognicode_explorer::facades::SearchService> =
            Arc::new(cognicode_explorer::facades::search::SearchServiceImpl::new(
                self.symbol_repo.clone(),
                None, // search_repo
                view_registry_for_search,
                None, // view_spec_store
                quality.clone(), // quality_repo — wired from PG (PR #55)
            ));

        // View facade.
        let view_impl: Arc<cognicode_explorer::facades::view::ViewServiceImpl> =
            Arc::new(cognicode_explorer::facades::view::ViewServiceImpl::new(
                self.symbol_repo.clone(),
                self.source_reader.clone(),
                quality.clone(), // quality_repo — wired from PG (PR #55)
                cognicode_explorer::domain::lens::default_registry(),
                graph_query.clone(),
                view_registry.clone(),
            ));
        let view: Arc<dyn cognicode_explorer::facades::ViewService> = view_impl.clone();
        let lens_executor: Arc<dyn cognicode_explorer::facades::LensExecutor> = view_impl;

        // Persistence facade — always takes 2 args; second is Some when postgres feature is enabled.
        let persistence: Arc<dyn cognicode_explorer::facades::PersistenceService> = Arc::new(
            cognicode_explorer::facades::persistence::PersistenceServiceImpl::new(
                None, // view_spec_store
                #[cfg(feature = "postgres")]
                self.pg_repo.clone(), // postgres_repo
            ),
        );
        let moldql: Arc<dyn cognicode_explorer::facades::MoldQLService> =
            Arc::new(cognicode_explorer::facades::moldql::MoldQLServiceImpl::new(
                self.symbol_repo.clone(),
                quality, // quality_repo — wired from PG (PR #55)
                self.source_reader.clone(),
                lens_executor,
                #[cfg(feature = "multimodal")]
                None, // graph_repo
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

        #[cfg(feature = "postgres")]
        if let Some(ingest_controller) = ingest {
            state = state.with_ingest(ingest_controller);
        }

        state
    }

    pub fn into_mcp_handler(self) -> cognicode_explorer::mcp::ExplorerMcpHandler {
        let view_registry = Arc::new(cognicode_explorer::registry::ViewRegistry::new(None));
        let lens_registry = cognicode_explorer::domain::lens::default_registry();

        #[cfg(feature = "postgres")]
        let quality = quality_repo_arc(self.pg_repo.as_ref());
        #[cfg(not(feature = "postgres"))]
        let quality = quality_repo_arc();

        #[cfg(feature = "postgres")]
        let quality_write = quality_write_repo_arc(self.pg_repo.as_ref());
        #[cfg(not(feature = "postgres"))]
        let quality_write = quality_write_repo_arc();

        cognicode_explorer::mcp::ExplorerMcpHandler::with_graph(
            self.symbol_repo,
            self.source_reader,
            view_registry,
            lens_registry,
            self.cwd,
            self.graph,
            quality,
            quality_write,
        )
    }
}

/// Build a `PostgresQualityRepository` from the runtime's PG repo.
///
/// Returns `None` when the `postgres` feature is off or when no PG
/// connection is available — in both cases the MCP tools degrade
/// gracefully via the `quality_unavailable` envelope. The previous
/// 3-place `None` pass-through was the source of the v0.22.0
/// "always quality_unavailable" symptom; this helper centralizes the
/// adapter construction so adding a new consumer is a one-liner.
#[cfg(feature = "postgres")]
fn quality_repo_arc(
    pg_repo: Option<&Arc<cognicode_core::infrastructure::persistence::PostgresRepository>>,
) -> Option<Arc<dyn cognicode_explorer::ports::QualityRepository>> {
    let pg = pg_repo?;
    Some(Arc::new(
        cognicode_explorer::adapters::PostgresQualityRepository::new(pg),
    ))
}

#[cfg(not(feature = "postgres"))]
fn quality_repo_arc() -> Option<Arc<dyn cognicode_explorer::ports::QualityRepository>> {
    None
}

/// Build a `PostgresQualityRepository` wired as a `QualityWritePort`.
///
/// Mirrors `quality_repo_arc` but returns the write port instead of the
/// read port. Both are backed by the same `PostgresQualityRepository`
/// value — the read/write split is purely at the trait level (ISP).
#[cfg(feature = "postgres")]
fn quality_write_repo_arc(
    pg_repo: Option<&Arc<cognicode_core::infrastructure::persistence::PostgresRepository>>,
) -> Option<Arc<dyn cognicode_explorer::ports::QualityWritePort>> {
    let pg = pg_repo?;
    Some(Arc::new(
        cognicode_explorer::adapters::PostgresQualityRepository::new(pg),
    ))
}

#[cfg(not(feature = "postgres"))]
fn quality_write_repo_arc() -> Option<Arc<dyn cognicode_explorer::ports::QualityWritePort>> {
    None
}
