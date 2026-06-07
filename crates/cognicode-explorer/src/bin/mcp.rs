// CogniCode Explorer MCP server binary.
//
// Reads JSON-RPC from stdin, writes responses to stdout. Logs and
// traces go to stderr. The handler follows the CogniCodeHandler
// canonical pattern (see cognicode-core/src/interface/mcp/rmcp_adapter.rs).
//
// Adapter construction mirrors bin/api.rs: open the SQLite store if
// present, build the call graph, wire the optional FTS5 / quality
// adapters, and assemble the ExplorerService.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use cognicode_db::SqliteGraphStore;
use cognicode_explorer::adapters::{
    CallGraphRepository, FsSourceReader, Fts5SearchAdapter, SqliteQualityAdapter,
};
use cognicode_explorer::mcp::ExplorerMcpHandler;
use cognicode_explorer::ports::quality_repository::QualityRepository;
use cognicode_explorer::ports::search_repository::SearchRepository;
use cognicode_explorer::ports::symbol_repository::SymbolRepository;
use cognicode_explorer::service::ExplorerService;

#[derive(Debug, Parser)]
#[command(name = "cognicode-explorer-mcp", version, about)]
struct Args {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,
}

fn empty_graph() -> Arc<cognicode_core::domain::aggregates::CallGraph> {
    Arc::new(cognicode_core::domain::aggregates::CallGraph::new())
}

fn open_graph(
    db_path: &std::path::Path,
) -> anyhow::Result<Arc<cognicode_core::domain::aggregates::CallGraph>> {
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
        .with_writer(std::io::stderr)
        .init();

    let cwd = args.cwd.clone();
    let db_path = cwd.join(".cognicode/cognicode.db");
    let reader = Arc::new(FsSourceReader::new(cwd.clone()));
    let graph = open_graph(&db_path)?;
    let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph));
    let search = maybe_fts5_adapter(&db_path);
    let quality = maybe_quality_adapter(&db_path);

    let service = Arc::new(ExplorerService::with_all(
        repo, reader, cwd, search, quality,
    ));
    let handler = ExplorerMcpHandler::new(service);

    tracing::info!("starting cognicode explorer MCP server on stdio");
    let transport = rmcp::transport::io::stdio();
    let server = rmcp::serve_server(handler, transport).await?;
    server.waiting().await?;
    Ok(())
}
