// CogniCode Explorer MCP server binary.
//
// Reads JSON-RPC from stdin, writes responses to stdout. Logs and
// traces go to stderr. The handler follows the CogniCodeHandler
// canonical pattern (see cognicode-core/src/interface/mcp/rmcp_adapter.rs).
//
// CLI dispatch (per `postgres-default-config` PR 2):
//   1. --postgres <URL>   (highest precedence)
//   2. DATABASE_URL       (env, non-empty)
//   3. --sqlite           (only when sqlite feature is on)
//   4. error              (fatal)

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use cognicode_explorer::adapters::{CallGraphRepository, FsSourceReader};
#[cfg(feature = "sqlite")]
use cognicode_explorer::adapters::{Fts5SearchAdapter, SqliteQualityAdapter};
use cognicode_explorer::cli_dispatch::{resolve_backend, Backend, ResolveInput};
use cognicode_explorer::mcp::ExplorerMcpHandler;
use cognicode_explorer::ports::quality_repository::QualityRepository;
use cognicode_explorer::ports::search_repository::SearchRepository;
use cognicode_explorer::ports::symbol_repository::SymbolRepository;
use cognicode_explorer::service::ExplorerService;

#[cfg(feature = "postgres")]
use cognicode_explorer::postgres_bridge::open_graph_from_postgres;

#[derive(Debug, Parser)]
#[command(
    name = "cognicode-explorer-mcp",
    version,
    about = "CogniCode Explorer MCP — JSON-RPC over stdio.\n\n\
             Precedence: --postgres <URL> > DATABASE_URL > --sqlite. \
             No flag and no env is fatal."
)]
struct Args {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,

    /// Load the call graph from a PostgreSQL database at startup.
    #[cfg(feature = "postgres")]
    #[arg(long)]
    postgres: Option<String>,

    /// Opt back into the local SQLite backend. Only honored with the
    /// `sqlite` feature. Conflicts with `--postgres`.
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cwd = args.cwd.clone();
    let db_path = cwd.join(".cognicode/cognicode.db");
    let reader = Arc::new(FsSourceReader::new(cwd.clone()));

    // Resolve the backend via the shared precedence helper.
    let postgres_flag = {
        #[cfg(feature = "postgres")]
        { args.postgres.clone() }
        #[cfg(not(feature = "postgres"))]
        { None }
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

    let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph.clone()));

    // FTS5 + quality adapters only available with the `sqlite` feature.
    #[cfg(feature = "sqlite")]
    let (search, quality) = (
        {
            use cognicode_explorer::adapters::Fts5SearchAdapter;
            if db_path.exists() {
                Some(Arc::new(Fts5SearchAdapter::new(db_path.clone())) as Arc<dyn SearchRepository>)
            } else {
                tracing::info!(
                    db = %db_path.display(),
                    "no cognicode.db — FTS5 backend disabled (exact-match only)"
                );
                None
            }
        },
        {
            use cognicode_explorer::adapters::SqliteQualityAdapter;
            if db_path.exists() {
                Some(Arc::new(SqliteQualityAdapter::new(db_path.clone())) as Arc<dyn QualityRepository>)
            } else {
                tracing::info!(
                    db = %db_path.display(),
                    "no cognicode.db — quality backend disabled"
                );
                None
            }
        },
    );
    #[cfg(not(feature = "sqlite"))]
    let (search, quality): (
        Option<Arc<dyn SearchRepository>>,
        Option<Arc<dyn QualityRepository>>,
    ) = (None, None);

    let service = Arc::new(ExplorerService::with_all(
        repo, reader, cwd, search, quality,
    ));
    let handler = ExplorerMcpHandler::with_graph(service, Some(graph));

    tracing::info!(backend = ?backend, "starting cognicode explorer MCP server on stdio");
    let transport = rmcp::transport::io::stdio();
    let server = rmcp::serve_server(handler, transport).await?;
    server.waiting().await?;
    Ok(())
}
