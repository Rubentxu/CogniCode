use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use cognicode_explorer::adapters::{CallGraphRepository, FsSourceReader};
#[cfg(feature = "sqlite")]
use cognicode_explorer::adapters::{Fts5SearchAdapter, SqliteQualityAdapter};
use cognicode_explorer::api;
use cognicode_explorer::cli_dispatch::{resolve_backend, Backend, ResolveInput};
use cognicode_explorer::ports::quality_repository::QualityRepository;
use cognicode_explorer::ports::search_repository::SearchRepository;
use cognicode_explorer::ports::symbol_repository::SymbolRepository;
use cognicode_explorer::service::ExplorerService;

#[cfg(feature = "postgres")]
use cognicode_explorer::postgres_bridge::open_graph_from_postgres;

#[derive(Debug, Parser)]
#[command(
    name = "cognicode-explorer-api",
    version,
    about = "CogniCode Explorer API — moldable code exploration HTTP service.\n\n\
             Precedence: --postgres <URL> > DATABASE_URL > --sqlite. \
             No flag and no env is fatal. See docs/postgres-default-config for details."
)]
struct Args {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,

    #[arg(long, default_value = "127.0.0.1:8010")]
    listen: SocketAddr,

    /// Load the call graph from a PostgreSQL database at startup
    /// (instead of the local `.cognicode/cognicode.db`). The pool
    /// is dropped once the graph is loaded; the explorer holds
    /// only the in-memory graph.
    #[cfg(feature = "postgres")]
    #[arg(long)]
    postgres: Option<String>,

    /// Opt back into the local SQLite backend (`.cognicode/cognicode.db`).
    /// Only honored when the `sqlite` feature is enabled; otherwise the
    /// flag is rejected at parse time. Conflicts with `--postgres`.
    #[cfg(feature = "sqlite")]
    #[arg(long, conflicts_with = "postgres")]
    sqlite: bool,
}

fn empty_graph() -> Arc<cognicode_core::domain::aggregates::CallGraph> {
    Arc::new(cognicode_core::domain::aggregates::CallGraph::new())
}

#[cfg(feature = "sqlite")]
fn open_graph(
    db_path: &std::path::Path,
) -> anyhow::Result<Arc<cognicode_core::domain::aggregates::CallGraph>> {
    use cognicode_db::SqliteGraphStore;
    if !db_path.exists() {
        tracing::warn!(db = %db_path.display(), "no cognicode.db — starting empty graph");
        return Ok(empty_graph());
    }
    let store = SqliteGraphStore::open(db_path)
        .map_err(|e| anyhow::anyhow!("opening {}: {}", db_path.display(), e))?;
    match store.load_graph() {
        Ok(Some(graph)) => Ok(Arc::new(graph)),
        Ok(None) => Ok(empty_graph()),
        Err(e) => {
            tracing::warn!(error = %e, "failed to load graph — starting with empty index");
            Ok(empty_graph())
        }
    }
}

/// Build the FTS5 search adapter if the DB exists. Only available
/// with the `sqlite` feature (FTS5 is a SQLite extension).
#[cfg(feature = "sqlite")]
fn maybe_fts5_adapter(db_path: &std::path::Path) -> Option<Arc<dyn SearchRepository>> {
    if !db_path.exists() {
        tracing::info!(
            db = %db_path.display(),
            "no cognicode.db — FTS5 backend disabled (exact-match only)"
        );
        return None;
    }
    Some(Arc::new(Fts5SearchAdapter::new(db_path.to_path_buf())))
}

/// Build the SQLite quality adapter if the DB exists.
#[cfg(feature = "sqlite")]
fn maybe_quality_adapter(db_path: &std::path::Path) -> Option<Arc<dyn QualityRepository>> {
    if !db_path.exists() {
        tracing::info!(
            db = %db_path.display(),
            "no cognicode.db — quality backend disabled"
        );
        return None;
    }
    Some(Arc::new(SqliteQualityAdapter::new(db_path.to_path_buf())))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cwd = args.cwd.clone();
    let db_path = cwd.join(".cognicode/cognicode.db");
    let reader = Arc::new(FsSourceReader::new(cwd.clone()));

    // Resolve which backend to use. Precedence:
    //   1. --postgres <URL>     (always wins if set)
    //   2. DATABASE_URL         (env, non-empty)
    //   3. --sqlite             (only with sqlite feature)
    //   4. error                (fatal)
    let postgres_flag = {
        #[cfg(feature = "postgres")]
        {
            args.postgres.clone()
        }
        #[cfg(not(feature = "postgres"))]
        {
            None
        }
    };
    let sqlite_flag = {
        #[cfg(feature = "sqlite")]
        { args.sqlite }
        #[cfg(not(feature = "sqlite"))]
        { false }
    };
    let input = ResolveInput {
        postgres_flag,
        database_url: std::env::var("DATABASE_URL").ok(),
        sqlite_flag,
    };
    let backend = resolve_backend(&input).map_err(|e| {
        anyhow::anyhow!(
            "{}\n\
             Hint: set DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode, \
             pass --postgres <URL>, or rebuild with --features sqlite and pass --sqlite.",
            e
        )
    })?;

    // Dispatch on the resolved backend. Each branch is feature-gated
    // so the unused branch doesn't drag in the wrong deps.
    let graph = match &backend {
        #[cfg(feature = "postgres")]
        Backend::Postgres(url) => open_graph_from_postgres(url).await?,
        #[cfg(not(feature = "postgres"))]
        Backend::Postgres(url) => {
            return Err(anyhow::anyhow!(
                "postgres feature not enabled (got URL `{}`); rebuild with --features postgres",
                url
            ));
        }
        #[cfg(feature = "sqlite")]
        Backend::Sqlite => open_graph(&db_path)?,
        #[cfg(not(feature = "sqlite"))]
        Backend::Sqlite => {
            return Err(anyhow::anyhow!(
                "sqlite feature not enabled; the helper resolved to Sqlite but the \
                 binary was built without --features sqlite. Rebuild or set DATABASE_URL."
            ));
        }
    };

    // FTS5 + quality adapters only exist with the `sqlite` feature.
    // When running with `Backend::Postgres`, the search/quality
    // backends are simply absent (exact-match only, no quality view).
    #[cfg(feature = "sqlite")]
    let (search, quality) = (maybe_fts5_adapter(&db_path), maybe_quality_adapter(&db_path));
    #[cfg(not(feature = "sqlite"))]
    let (search, quality): (
        Option<Arc<dyn SearchRepository>>,
        Option<Arc<dyn QualityRepository>>,
    ) = (None, None);

    let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph));
    let service = ExplorerService::with_all(repo, reader, cwd, search, quality);
    tracing::info!(listen = %args.listen, backend = ?backend, "starting cognicode explorer API");
    api::serve(service, args.listen).await
}
