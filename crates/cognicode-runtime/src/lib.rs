//! CogniCode Runtime — shared bootstrap for API and MCP binaries.
//!
//! v1 of `e29-2-remove-pg` migration is in progress. The runtime
//! still uses `PostgresRepository` directly; a follow-up PR will
//! migrate the call sites to use the `PgBackend` trait (below) +
//! `LadybugPgBackend` adapter (e29-2-switch-default, PR #204).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
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
    /// `PgBackend` trait object — abstracts the storage backend (PG
    /// live or lbug 0.19). The runtime uses this for port
    /// construction. `None` when no backend was provided (legacy
    /// bootstrap path).
    pub backend: Option<Arc<dyn PgBackend>>,
    /// Shared `PostgresRepository` Arc (e29-7 task-2). This is the
    /// single canonical source for the relocated port adapters
    /// (`quality_store` / `view_spec_store` / `call_graph_store`) and
    /// the legacy PG call sites that still need the concrete repo
    /// type. `Some` on the postgres bootstrap path, `None` on ladybug.
    #[cfg(feature = "postgres")]
    pub pg_repo: Option<Arc<cognicode_core::infrastructure::persistence::PostgresRepository>>,
    /// Shared revision tracker — bumped by `index_workspace` after each successful ingest.
    pub revision_tracker: Arc<AtomicU64>,
    /// Optional `QualityStore` port (PR2 relocation: from
    /// `cognicode_explorer::ports::quality_repository::QualityRepository`
    /// to the unified `cognicode_core::domain::ports::QualityStore`).
    /// `None` when the PG adapter couldn't be constructed.
    pub quality_store: Option<Arc<dyn cognicode_explorer::ports::QualityStore>>,
    /// Optional `ViewSpecStore` port (PR2 relocation from
    /// `cognicode_explorer::registry::ViewSpecStore` to
    /// `cognicode_core::domain::ports::ViewSpecStore`).
    pub view_spec_store: Option<Arc<dyn cognicode_core::domain::ports::ViewSpecStore>>,
    /// Optional `CallGraphStore` port (e29-0-refactor-call-sites).
    /// Mirrors the `PostgresRepository::save_call_graph_ws` /
    /// `load_call_graph_ws` / `load_call_graph_current_ws` SQL
    /// surfaces behind the new domain port so consumers (`PgGraphExecutor`,
    /// `postgres_bridge`) depend on `Arc<dyn CallGraphStore>` instead
    /// of the concrete `PostgresRepository` for this aggregate.
    pub call_graph_store: Option<Arc<dyn cognicode_core::domain::ports::CallGraphStore>>,
}

/// `PgBackend` trait — abstracts the subset of `PostgresRepository`
/// operations the runtime needs. Implemented by both the live
/// `PostgresBackend` (PR follow-up) and `LadybugPgBackend` (this PR).
///
/// v1 of the migration adds the trait + the lbug-based adapter. The
/// runtime's `bootstrap` function still uses `PostgresRepository`
/// directly; a follow-up PR will switch it to use `&dyn PgBackend`.
pub trait PgBackend: Send + Sync {
    fn quality_store(&self) -> Option<Arc<dyn cognicode_core::domain::ports::QualityStore>>;
    fn view_spec_store(&self) -> Option<Arc<dyn cognicode_core::domain::ports::ViewSpecStore>>;
    fn call_graph_store(&self) -> Option<Arc<dyn cognicode_core::domain::ports::CallGraphStore>>;
    /// Returns the concrete `PostgresRepository` if this backend is
    /// the PG-backed one. Used by legacy call sites that need the
    /// concrete repo type (e.g. `new_with_pg_repo` in
    /// CallGraphRepository). Returns `None` for non-PG backends.
    fn as_postgres_repo(
        &self,
    ) -> Option<Arc<cognicode_core::infrastructure::persistence::PostgresRepository>>;
}

/// `LadybugPgBackend` — implements `PgBackend` on top of the
/// `cognicode-ladybug` crate. Used when the runtime is built with
/// `--features ladybug` (e29-2-switch-default, PR #204).
///
/// v1: the 3 ports are stored as `Option<Arc<dyn ...>>` and returned
/// via the trait methods. The full integration (constructing the
/// actual port impls from a `LadybugStore`) is a follow-up PR.
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
    fn as_postgres_repo(
        &self,
    ) -> Option<Arc<cognicode_core::infrastructure::persistence::PostgresRepository>> {
        None
    }
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

/// `PostgresBackend` — implements `PgBackend` on top of the
/// existing `PostgresRepository`. Used when the runtime is built with
/// `--features postgres` (the default until v0.79).
///
/// v1: wraps the existing `PostgresRepository` so the runtime's
/// bootstrap path can stay unchanged (it still uses PG-derived
/// types for the port construction). v1 returns `None` from the
/// 3 port accessors — the ports are populated by the bootstrap
/// function from the existing `PostgresRepository`, not from the
/// `PgBackend` trait. The full migration is a follow-up PR.
#[cfg(feature = "postgres")]
pub struct PostgresBackend {
    repo: Arc<cognicode_core::infrastructure::persistence::PostgresRepository>,
}

#[cfg(feature = "postgres")]
impl PostgresBackend {
    /// Build a new `PostgresBackend` wrapping the given PG repo.
    pub fn new(repo: Arc<cognicode_core::infrastructure::persistence::PostgresRepository>) -> Self {
        Self { repo }
    }
}

#[cfg(feature = "postgres")]
impl PgBackend for PostgresBackend {
    fn as_postgres_repo(
        &self,
    ) -> Option<Arc<cognicode_core::infrastructure::persistence::PostgresRepository>> {
        Some(self.repo.clone())
    }
    fn quality_store(&self) -> Option<Arc<dyn cognicode_core::domain::ports::QualityStore>> {
        // v1: the runtime's bootstrap constructs the QualityStore
        // from self.repo directly (the pre-cutover path). This
        // trait method returns None for now — a follow-up PR will
        // populate it from self.repo.
        None
    }
    fn view_spec_store(&self) -> Option<Arc<dyn cognicode_core::domain::ports::ViewSpecStore>> {
        // Same v1 limitation as quality_store.
        None
    }
    fn call_graph_store(&self) -> Option<Arc<dyn cognicode_core::domain::ports::CallGraphStore>> {
        // Same v1 limitation as quality_store.
        None
    }
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
                    cognicode_explorer::postgres_bridge::open_graph_from_postgres(url, &cwd)
                        .await?;
                graph_cache.set((*graph).clone());
                Some(graph)
            }
            #[cfg(not(feature = "postgres"))]
            Some(_) => unreachable!("postgres feature not enabled"),
            None => None,
        };

        // Build the shared `pg_repo` Arc (e29-7 task-2). This is the
        // single canonical source for the relocated port adapters and
        // for the legacy PG call sites that still need the concrete
        // `PostgresRepository` type.
        #[cfg(feature = "postgres")]
        let pg_repo: Option<
            Arc<cognicode_core::infrastructure::persistence::PostgresRepository>,
        > = if let Some(ref url) = pg_url {
            let repo = Arc::new(
                cognicode_core::infrastructure::persistence::PostgresRepository::new(url)
                    .await
                    .map_err(|e| anyhow::anyhow!("PG connect: {e}"))?,
            );
            Some(repo)
        } else {
            None
        };

        // The `PgBackend` trait object is kept for the postgres path
        // (task-2 → task-6 removes it entirely, setting `backend: None`).
        #[cfg(feature = "postgres")]
        let backend: Option<Arc<dyn PgBackend>> = pg_repo
            .as_ref()
            .map(|repo| Arc::new(PostgresBackend::new(Arc::clone(repo))) as Arc<dyn PgBackend>);
        #[cfg(not(feature = "postgres"))]
        let backend: Option<Arc<dyn PgBackend>> = None;

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
                            #[cfg(feature = "postgres")]
                            pg_repo.clone(),
                            #[cfg(not(feature = "postgres"))]
                            None,
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

        // Construct the relocated port adapters BEFORE we move
        // pg_repo into the struct (PR2 WU2.9 — wiring happens up front).
        // e29-7 task-2: built directly from the shared `pg_repo` Arc
        // (the dead `PgBackend` port accessors are no longer used).
        #[cfg(feature = "postgres")]
        let quality_store: Option<Arc<dyn cognicode_explorer::ports::QualityStore>> =
            pg_repo.as_ref().map(|repo| {
                Arc::new(cognicode_core::domain::ports::PostgresQualityStore::new(repo))
                    as Arc<dyn cognicode_explorer::ports::QualityStore>
            });
        #[cfg(not(feature = "postgres"))]
        let quality_store: Option<Arc<dyn cognicode_explorer::ports::QualityStore>> = None;
        #[cfg(feature = "postgres")]
        let view_spec_store: Option<Arc<dyn cognicode_core::domain::ports::ViewSpecStore>> =
            pg_repo.as_ref().map(|repo| {
                Arc::new(cognicode_core::domain::ports::PostgresViewSpecStore::new(
                    Arc::clone(repo),
                )) as Arc<dyn cognicode_core::domain::ports::ViewSpecStore>
            });
        #[cfg(not(feature = "postgres"))]
        let view_spec_store: Option<Arc<dyn cognicode_core::domain::ports::ViewSpecStore>> = None;

        // `CallGraphStore` port wiring (e29-0-refactor-call-sites).
        // Same shape as the existing `quality_store` / `view_spec_store`
        // wiring: built from the shared `pg_repo` Arc, so Phase 1
        // `LadybugStore` can drop in here as a single-line replacement.
        #[cfg(feature = "postgres")]
        let call_graph_store: Option<
            Arc<dyn cognicode_core::domain::ports::CallGraphStore>,
        > = pg_repo.as_ref().map(|repo| {
            Arc::new(cognicode_core::domain::ports::PostgresCallGraphStore::new(
                Arc::clone(repo),
            )) as Arc<dyn cognicode_core::domain::ports::CallGraphStore>
        });
        #[cfg(not(feature = "postgres"))]
        let call_graph_store: Option<
            Arc<dyn cognicode_core::domain::ports::CallGraphStore>,
        > = None;

        Ok(Self {
            symbol_repo,
            source_reader,
            graph,
            cwd,
            graph_cache,
            backend,
            #[cfg(feature = "postgres")]
            pg_repo,
            revision_tracker: Arc::new(AtomicU64::new(1)),
            quality_store,
            view_spec_store,
            call_graph_store,
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
                    #[cfg(feature = "postgres")]
                    self.pg_repo.clone(),
                    #[cfg(not(feature = "postgres"))]
                    None,
                ),
            ) as Arc<dyn GraphQueryPort>
        });

        // GraphExecutor for Pattern Profile queries. Constructed from pg_repo when available.
        #[cfg(feature = "postgres")]
        let graph_executor: Option<
            Arc<dyn cognicode_core::domain::plan::executor::GraphExecutor>,
        > = self.pg_repo.as_ref().map(|repo| {
            let pool = repo.with_pool(|p| p.clone());
            // `PgGraphExecutor::new` takes an OWNED `PostgresRepository`
            // and `PostgresRepository` is NOT `Clone`, so the executor
            // and the `CallGraphStore` each wrap a `from_pool` view of
            // the same PgPool. (Previously `Arc::try_unwrap` panicked
            // whenever the cg_store held a second Arc clone — e29-7
            // task-3 removes that runtime panic.)
            let cg_store: Arc<dyn cognicode_core::domain::ports::CallGraphStore> = Arc::new(
                cognicode_core::domain::ports::PostgresCallGraphStore::new(Arc::new(
                    cognicode_core::infrastructure::persistence::PostgresRepository::from_pool(
                        pool.clone(),
                    ),
                )),
            );
            Arc::new(
                cognicode_core::infrastructure::persistence::pg_graph_executor::PgGraphExecutor::new(
                    cognicode_core::infrastructure::persistence::PostgresRepository::from_pool(
                        pool,
                    ),
                    cg_store,
                ),
            ) as Arc<dyn cognicode_core::domain::plan::executor::GraphExecutor>
        });
        #[cfg(not(feature = "postgres"))]
        let graph_executor: Option<
            Arc<dyn cognicode_core::domain::plan::executor::GraphExecutor>,
        > = None;

        // Workspace resolver — maps workspace_id → root_path.
        // Populated when open_workspace is called.
        let ws_resolver =
            Arc::new(cognicode_core::application::ingest::StaticWorkspaceResolver::new());
        let ws_resolver_dyn: Arc<dyn cognicode_core::application::ingest::WorkspaceResolver> =
            ws_resolver.clone();

        // IngestController — only when PG is available.
        #[cfg(feature = "postgres")]
        let ingest = self.pg_repo.as_ref().map(|repo| {
            Arc::new(cognicode_core::application::ingest::IngestController::new(
                repo.clone(),
                self.graph_cache.clone(),
                ws_resolver_dyn.clone(),
            ))
        });
        #[cfg(not(feature = "postgres"))]
        let ingest: Option<Arc<cognicode_core::application::ingest::IngestController>> = None;

        // Workspace facade.
        let workspace: Arc<dyn cognicode_explorer::facades::WorkspaceService> = Arc::new(
            cognicode_explorer::facades::workspace::WorkspaceServiceImpl::new(
                self.symbol_repo.clone(),
                self.cwd.clone(),
                Some(ws_resolver.clone()),
            ),
        );

        // Persistence facade — always takes 2 args; second is Some when postgres feature is enabled.
        let persistence: Arc<dyn cognicode_explorer::facades::PersistenceService> = Arc::new(
            cognicode_explorer::facades::persistence::PersistenceServiceImpl::new(
                None, // view_spec_store
                // postgres_repo — None when the backend is lbug
                #[cfg(feature = "postgres")]
                self.pg_repo.clone(),
                #[cfg(not(feature = "postgres"))]
                None,
            ),
        );

        // Search facade.
        let view_registry = Arc::new(cognicode_explorer::registry::ViewRegistry::new(None));
        let view_registry_for_search = view_registry.clone();
        #[cfg(feature = "postgres")]
        let quality = quality_repo_arc(
            self.backend
                .as_ref()
                .and_then(|b| b.as_postgres_repo())
                .as_ref(),
        );
        #[cfg(not(feature = "postgres"))]
        let quality = quality_repo_arc();

        // Investigation facade — wired from postgres when available (ADR-005 INV-1)
        #[cfg(feature = "postgres")]
        let investigation: Option<
            Arc<dyn cognicode_explorer::facades::InvestigationFacade>,
        > = if let Some(ref repo) = self.pg_repo {
            let pool = repo.with_pool(|p| p.clone());
            Some(
                cognicode_explorer::facades::investigation::new_investigation_service_from_postgres(
                    &pool,
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
                    cognicode_explorer::adapters::PgGraphRepository::new(
                        pg.with_pool(|p| p.clone()),
                    ),
                ))
            } else {
                None
            };
        #[cfg(all(feature = "multimodal", not(feature = "postgres")))]
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
                view_registry_for_search,
                None,                      // view_spec_store
                quality.clone(),           // quality_repo — wired from PG (PR #55)
                Some(persistence.clone()), // persistence — for SavedExploration search
                // investigation — wired from PG (e13-wave-1). Cloned so
                // the SAME Arc is also wired into ApiState below (e29-7
                // task-3: state.investigation == search investigation).
                #[cfg(feature = "postgres")]
                investigation.clone(),
                #[cfg(not(feature = "postgres"))]
                investigation,
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
        let lens_executor: Arc<dyn cognicode_explorer::facades::LensService> = view_impl;

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
                Some(1),                     // revision_id — default revision
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

        // Investigation service — ADR-005 INV-1. Wired from the SAME
        // Arc passed to the search facade above (e29-7 task-3): the
        // duplicate construction site was deleted, so
        // `state.investigation` and the search facade share one
        // service instance.
        #[cfg(feature = "postgres")]
        if let Some(investigation) = investigation {
            state = state.with_investigation(investigation);
        }

        // Wire the shared revision tracker so MoldQL REST endpoints can pin queries.
        state = state.with_revision_tracker(self.revision_tracker.clone());

        state
    }

    pub fn into_mcp_handler(self) -> cognicode_explorer::mcp::ExplorerMcpHandler {
        let view_registry = Arc::new(cognicode_explorer::registry::ViewRegistry::new(None));
        let lens_registry = cognicode_explorer::domain::lens::default_registry();

        #[cfg(feature = "postgres")]
        let quality = quality_repo_arc(
            self.backend
                .as_ref()
                .and_then(|b| b.as_postgres_repo())
                .as_ref(),
        );
        #[cfg(not(feature = "postgres"))]
        let quality = quality_repo_arc();

        #[cfg(feature = "postgres")]
        let quality_write = quality_write_repo_arc(
            self.backend
                .as_ref()
                .and_then(|b| b.as_postgres_repo())
                .as_ref(),
        );
        #[cfg(not(feature = "postgres"))]
        let quality_write = quality_write_repo_arc();

        #[cfg(feature = "postgres")]
        let route_store: Option<Arc<dyn cognicode_explorer::ports::RouteStore>> = self
            .pg_repo
            .as_ref()
            .map(|repo| {
                Arc::new(cognicode_explorer::adapters::PostgresRouteStore::from_pool(
                    repo.with_pool(|p| p.clone()),
                )) as Arc<dyn cognicode_explorer::ports::RouteStore>
            });
        #[cfg(not(feature = "postgres"))]
        let route_store: Option<Arc<dyn cognicode_explorer::ports::RouteStore>> = None;

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
            route_store,
            #[cfg(feature = "postgres")]
            self.pg_repo.clone(),
            #[cfg(not(feature = "postgres"))]
            None,
        )
    }
}

/// Build a `PostgresQualityStore` from the runtime's PG repo.
///
/// Returns `None` when the `postgres` feature is off or when no PG
/// connection is available — in both cases the MCP tools degrade
/// gracefully via the `quality_unavailable` envelope. The previous
/// 3-place `None` pass-through was the source of the v0.22.0
/// "always quality_unavailable" symptom; this helper centralizes the
/// adapter construction so adding a new consumer is a one-liner.
///
/// PR2 relocation: the adapter now lives in `cognicode-core`'s
/// `domain::ports::quality_store` module (10-method unified surface).
#[cfg(feature = "postgres")]
fn quality_repo_arc(
    pg_repo: Option<&Arc<cognicode_core::infrastructure::persistence::PostgresRepository>>,
) -> Option<Arc<dyn cognicode_explorer::ports::QualityStore>> {
    let pg = pg_repo?;
    Some(Arc::new(
        cognicode_core::domain::ports::PostgresQualityStore::new(pg),
    ))
}

#[cfg(not(feature = "postgres"))]
fn quality_repo_arc() -> Option<Arc<dyn cognicode_explorer::ports::QualityStore>> {
    None
}

/// Build a `PostgresQualityStore` wired as a write-capable `QualityStore`.
///
/// Mirrors `quality_repo_arc` — both are backed by the same
/// `PostgresQualityStore` value; the read/write split is preserved at
/// the call-site level (callers choose which methods to invoke).
#[cfg(feature = "postgres")]
fn quality_write_repo_arc(
    pg_repo: Option<&Arc<cognicode_core::infrastructure::persistence::PostgresRepository>>,
) -> Option<Arc<dyn cognicode_explorer::ports::QualityStore>> {
    let pg = pg_repo?;
    Some(Arc::new(
        cognicode_core::domain::ports::PostgresQualityStore::new(pg),
    ))
}

#[cfg(not(feature = "postgres"))]
fn quality_write_repo_arc() -> Option<Arc<dyn cognicode_explorer::ports::QualityStore>> {
    None
}

/// Build a Runtime with an explicit `&dyn PgBackend`. v1 of the
/// `e29-2-final-cutover` migration — the runtime no longer requires
/// a live PG when the caller provides a backend.
///
/// v1: this delegates to the existing `bootstrap()` flow with
/// `postgres_url = None`. The `backend` parameter is accepted for
/// API compatibility but not yet wired into the runtime's port
/// construction. The full migration is tracked as a follow-up
/// PR that rewires the call sites.
///
/// v0.78.0 (PR #205) added the `PgBackend` trait + `LadybugPgBackend`
/// adapter. v0.79+ will switch the runtime default from
/// `postgres` to `ladybug` and route through `bootstrap_with_backend`.
pub async fn bootstrap_with_backend(
    cwd: std::path::PathBuf,
    backend: std::sync::Arc<dyn PgBackend>,
) -> Result<Runtime, anyhow::Error> {
    // v1: accept the backend for API compatibility but delegate
    // to the existing bootstrap flow (which still requires a live
    // PG connection for v0.78.0). The full migration is the
    // follow-up PR.
    let _ = backend;
    Runtime::bootstrap(cwd, None).await
}
