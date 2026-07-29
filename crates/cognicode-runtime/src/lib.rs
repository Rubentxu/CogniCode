//! CogniCode Runtime — shared bootstrap for API and MCP binaries.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

use cognicode_core::infrastructure::graph::graph_cache::GraphCache;
#[cfg(feature = "postgres")]
use cognicode_core::infrastructure::persistence::PostgresRepository;

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
                #[cfg(not(feature = "ownership"))]
                {
                    Arc::new(cognicode_explorer::adapters::CallGraphRepository::new(
                        g.clone(),
                    ))
                }
                #[cfg(feature = "ownership")]
                {
                    Arc::new(
                        cognicode_explorer::adapters::CallGraphRepository::new_with_pg_repo(
                            g.clone(),
                            pg_repo.clone(),
                        ),
                    )
                }
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
        #[cfg(not(feature = "ownership"))]
        let graph_query: Option<Arc<dyn GraphQueryPort>> = self.graph.clone().map(|g| {
            Arc::new(cognicode_explorer::adapters::CallGraphRepository::new(g))
                as Arc<dyn GraphQueryPort>
        });
        #[cfg(feature = "ownership")]
        let graph_query: Option<Arc<dyn GraphQueryPort>> = self.graph.clone().map(|g| {
            Arc::new(
                cognicode_explorer::adapters::CallGraphRepository::new_with_pg_repo(
                    g,
                    self.pg_repo.clone(),
                ),
            ) as Arc<dyn GraphQueryPort>
        });

        // GraphExecutor for Pattern Profile queries. Constructed from pg_repo when available.
        #[cfg(feature = "postgres")]
        let graph_executor: Option<
            Arc<dyn cognicode_core::domain::plan::executor::GraphExecutor>,
        > = self.pg_repo.as_ref().map(|repo| {
            let pool = repo.pool().clone();
            let pg_repo =
                cognicode_core::infrastructure::persistence::PostgresRepository::from_pool(pool);
            Arc::new(
                cognicode_core::infrastructure::persistence::pg_graph_executor::PgGraphExecutor::new(
                    pg_repo,
                ),
            ) as Arc<dyn cognicode_core::domain::plan::executor::GraphExecutor>
        });
        #[cfg(not(feature = "postgres"))]
        let graph_executor: Option<
            Arc<dyn cognicode_core::domain::plan::executor::GraphExecutor>,
        > = None;

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

        // Persistence facade — always takes 2 args; second is Some when postgres feature is enabled.
        let persistence: Arc<dyn cognicode_explorer::facades::PersistenceService> = Arc::new(
            cognicode_explorer::facades::persistence::PersistenceServiceImpl::new(
                None, // view_spec_store
                #[cfg(feature = "postgres")]
                self.pg_repo.clone(), // postgres_repo
            ),
        );

        // Search facade.
        let view_registry = Arc::new(cognicode_explorer::registry::ViewRegistry::new(None));
        let view_registry_for_search = view_registry.clone();
        #[cfg(feature = "postgres")]
        let quality = quality_repo_arc(self.pg_repo.as_ref());
        #[cfg(not(feature = "postgres"))]
        let quality = quality_repo_arc();

        // Investigation facade — wired from postgres when available (ADR-005 INV-1)
        #[cfg(feature = "postgres")]
        let investigation: Option<
            Arc<dyn cognicode_explorer::facades::InvestigationFacade>,
        > = if let Some(ref repo) = self.pg_repo {
            Some(
                cognicode_explorer::facades::investigation::new_investigation_service_from_postgres(
                    repo.pool(),
                ),
            )
        } else {
            None
        };
        #[cfg(not(feature = "postgres"))]
        let investigation: Option<
            Arc<dyn cognicode_explorer::facades::InvestigationFacade>,
        > = None;

        // Graph repository for multimodal search (Doc/Decision/Evidence families).
        // Read path is ungated (default build); write/exTRACTION stays behind multimodal.
        // NOTE: PgGraphRepository impl requires multimodal domain types (NodeKind::Doc,
        // EdgeKind::Justifies/Cites/Resolves/CorroboratedBy). Full read-path wiring with
        // postgres-only is deferred to a follow-up that un-gates those domain types.
        #[cfg(all(feature = "multimodal", feature = "postgres"))]
        let graph_repo: Option<Arc<dyn cognicode_core::domain::ports::GraphRepository>> =
            if let Some(ref pg) = self.pg_repo {
                Some(Arc::new(
                    cognicode_explorer::adapters::PgGraphRepository::new(pg.pool().clone()),
                ))
            } else {
                None
            };
        #[cfg(all(feature = "multimodal", not(feature = "postgres")))]
        let graph_repo: Option<Arc<dyn cognicode_core::domain::ports::GraphRepository>> = Some(Arc::new(
            cognicode_explorer::adapters::InMemoryGraphRepository::new(vec![], vec![]),
        ));
        #[cfg(not(feature = "multimodal"))]
        let graph_repo: Option<Arc<dyn cognicode_core::domain::ports::GraphRepository>> = Some(Arc::new(
            cognicode_explorer::adapters::InMemoryGraphRepository::new(vec![], vec![]),
        ));

        let search: Arc<dyn cognicode_explorer::facades::SearchService> =
            Arc::new(cognicode_explorer::facades::search::SearchServiceImpl::new(
                self.symbol_repo.clone(),
                None, // search_repo
                view_registry_for_search,
                None,                      // view_spec_store
                quality.clone(),           // quality_repo — wired from PG (PR #55)
                Some(persistence.clone()), // persistence — for SavedExploration search
                investigation,             // investigation — wired from PG (e13-wave-1)
                graph_repo.clone(),
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
                Some(persistence.clone()), // persistence — for SavedExploration view resolution
                graph_repo.clone(),
            ));
        let view: Arc<dyn cognicode_explorer::facades::ViewService> = view_impl.clone();
        let lens_executor: Arc<dyn cognicode_explorer::facades::LensExecutor> = view_impl;

        let moldql: Arc<dyn cognicode_explorer::facades::MoldQLService> =
            Arc::new(cognicode_explorer::facades::moldql::MoldQLServiceImpl::new(
                self.symbol_repo.clone(),
                quality, // quality_repo — wired from PG (PR #55)
                self.source_reader.clone(),
                lens_executor,
                #[cfg(feature = "multimodal")]
                None, // graph_repo
                graph_executor,
                Some("default".to_string()), // workspace_id — runtime doesn't track active workspace yet
                Some(1),                   // revision_id — default revision
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

        #[cfg(feature = "multimodal")]
        {
            let snapshot_service =
                Arc::new(cognicode_explorer::domain::snapshot::SnapshotService::new());
            state = state.with_snapshot(snapshot_service);
        }

        // Investigation service — ADR-005 INV-1
        #[cfg(feature = "postgres")]
        if let Some(ref pg_repo) = self.pg_repo {
            use cognicode_explorer::facades::investigation::new_investigation_service_from_postgres;
            let investigation = new_investigation_service_from_postgres(pg_repo.pool());
            state = state.with_investigation(investigation);
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

        #[cfg(feature = "postgres")]
        let edge_emitter = edge_emitter_repo_arc(self.pg_repo.as_ref());
        #[cfg(not(feature = "postgres"))]
        let edge_emitter = None;

        cognicode_explorer::mcp::ExplorerMcpHandler::with_graph(
            self.symbol_repo,
            self.source_reader,
            view_registry,
            lens_registry,
            self.cwd,
            self.graph,
            quality,
            quality_write,
            #[cfg(feature = "multimodal")]
            edge_emitter,
            #[cfg(feature = "ownership")]
            self.pg_repo.clone(),
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

/// Build a `PostgresEdgeEmitter` wired as an `EdgeEmitter` port.
///
/// Returns `None` when the `postgres` feature is off or when no PG
/// connection is available — the `ingest_openapi` and `trace_route`
/// tools degrade gracefully with a `feature_disabled` envelope.
#[cfg(feature = "postgres")]
fn edge_emitter_repo_arc(
    pg_repo: Option<&Arc<cognicode_core::infrastructure::persistence::PostgresRepository>>,
) -> Option<Arc<dyn cognicode_explorer::ports::EdgeEmitter>> {
    let pg = pg_repo?;
    Some(Arc::new(
        cognicode_explorer::adapters::PostgresEdgeEmitter::from_pool(pg.pool().clone()),
    ))
}

#[cfg(not(feature = "postgres"))]
fn edge_emitter_repo_arc(
    _pg_repo: Option<&Arc<cognicode_core::infrastructure::persistence::PostgresRepository>>,
) -> Option<Arc<dyn cognicode_explorer::ports::EdgeEmitter>> {
    None
}
