//! TASK-92 (slice 1) — a live daemon says its index is still fresh, and
//! the client serves it without reading the content of the tree again.
//!
//! ## What is being protected
//!
//! The durable snapshot is already shared by every client (TASK-79), so
//! the parse is paid once. What each *new* client still paid was the
//! freshness check: the content of every file in the tree, hashed, to find
//! out whether the index still describes it. A daemon that watches the
//! tree knows that without reading it.
//!
//! The contract this file protects:
//!
//! 1. With a live daemon, the answer says the daemon served it, and the
//!    graph is the graph of the tree.
//! 2. The daemon's word does not survive a change the client can see: the
//!    client's own check decides, because a daemon that has not rebuilt
//!    yet is a daemon that does not know.
//! 3. Without a daemon, the same tree gives the same graph, and the
//!    answer does not claim that a daemon served it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cognicode_core::application::analysis_basis::{
    AnalysisBasis, AnalysisOp, Analyze, GraphSource, GraphValue,
};
use cognicode_core::application::consumer_analysis::ServiceAnalyzer;
use cognicode_core::application::freshness::FreshnessOracle;
use cognicode_core::application::graph_cache;
use cognicode_core::application::graph_snapshot;
use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::interface::daemon::{self, DaemonOracle, DaemonState};
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};

/// A workspace with one file the product can read.
fn rust_workspace(dir: &Path) {
    std::fs::create_dir_all(dir.join("src")).expect("the workspace is created");
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .expect("the manifest is written");
    std::fs::write(
        dir.join("src").join("main.rs"),
        "fn helper() -> u32 { 1 }\nfn main() { helper(); }\n",
    )
    .expect("the source is written");
}

/// A workspace and the cache that holds what is derived from it.
fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let root = tempfile::tempdir().expect("a temporary root");
    let workspace = root.path().join("ws");
    let cache = root.path().join("cache");
    rust_workspace(&workspace);
    (root, workspace, cache)
}

/// Writes a file and gives it a modification time of its own.
///
/// The freshness check compares modification times, and two writes in the
/// same millisecond leave the same time behind —so the time is set
/// explicitly. A test that depends on how fast the machine writes is not
/// a test.
fn write_with_mtime(path: &Path, contents: &str, millis: u64) {
    std::fs::write(path, contents).expect("the file is written");
    let file = std::fs::File::options()
        .write(true)
        .open(path)
        .expect("the file opens");
    let times = std::fs::FileTimes::new()
        .set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(millis));
    file.set_times(times).expect("the modification time is set");
}

/// The daemon of `workspace`: it keeps the index the client will be
/// served, and it says so on a socket.
///
/// The index is the one the daemon calculated, and the socket is bound
/// before the accept loop starts, so a client that asks right away finds
/// the daemon there.
fn start_daemon(cache: &Path, workspace: &Path) -> (Arc<DaemonState>, PathBuf) {
    // The daemon's own build: the index it keeps is the one the client
    // will find in the cache.
    let service = AnalysisService::new();
    graph_snapshot::load_or_build_graph(&service, workspace, Some(cache), None, false)
        .expect("the daemon's build answers");

    let state = Arc::new(DaemonState::new());
    let record = daemon::index_from_snapshot(cache, workspace).expect("the daemon keeps an index");
    state.publish(record);

    let socket = graph_cache::daemon_socket_path(cache, workspace);
    let listener = daemon::bind(&socket).expect("the socket is bound");
    let state_for_thread = state.clone();
    std::thread::spawn(move || {
        daemon::serve(listener, state_for_thread);
    });
    (state, socket)
}

/// The client: an analyzer that asks the daemon before it reads the tree.
fn analyzer_with_daemon(cache: &Path, workspace: &Path) -> ServiceAnalyzer {
    ServiceAnalyzer::with_cache_root(Some(cache.to_path_buf())).with_oracle(Some(Arc::new(
        DaemonOracle::at(graph_cache::daemon_socket_path(cache, workspace)),
    )))
}

/// The client that has no daemon to ask.
fn analyzer_without_daemon(cache: &Path) -> ServiceAnalyzer {
    ServiceAnalyzer::with_cache_root(Some(cache.to_path_buf()))
}

/// The answer a client gets, and where the graph came from.
struct Answer {
    source: Option<GraphSource>,
    symbols: usize,
}

/// Asks the question the way a client does: a fresh analyzer over the
/// tree, which is what every new process is.
fn ask(analyzer: &ServiceAnalyzer, workspace: &Path) -> Answer {
    let basis = AnalysisBasis::for_workspace(workspace.to_path_buf());
    let outcome = analyzer.execute(
        &basis,
        AnalysisOp::GraphFull {
            path: workspace.to_path_buf(),
            rebuild: false,
        },
    );
    let symbols = match outcome.value() {
        Some(cognicode_core::application::analysis_basis::AnalysisValue::Graph(
            GraphValue::Full { symbols, .. },
        )) => *symbols,
        _ => 0,
    };
    Answer {
        source: outcome.provenance().graph_source,
        symbols,
    }
}

#[test]
fn a_live_daemon_serves_the_index_and_says_so() {
    let (_root, workspace, cache) = fixture();
    let (_state, _socket) = start_daemon(&cache, &workspace);

    let answer = ask(&analyzer_with_daemon(&cache, &workspace), &workspace);

    assert_eq!(
        answer.source,
        Some(GraphSource::Daemon),
        "a live daemon vouched for the index, and the answer says who served it"
    );
    assert!(
        answer.symbols >= 2,
        "the graph is the graph of the tree: {} symbols",
        answer.symbols
    );
}

#[test]
fn the_daemons_word_does_not_survive_a_change_the_client_can_see() {
    let (_root, workspace, cache) = fixture();
    let (_state, _socket) = start_daemon(&cache, &workspace);

    // The tree changes. The daemon has not rebuilt yet, so it does not
    // know; the client can see the change, so the client decides.
    write_with_mtime(
        &workspace.join("src").join("main.rs"),
        "fn helper() -> u32 { 1 }\nfn other() -> u32 { 2 }\nfn main() { helper(); other(); }\n",
        1_000_000,
    );

    let answer = ask(&analyzer_with_daemon(&cache, &workspace), &workspace);

    assert_ne!(
        answer.source,
        Some(GraphSource::Daemon),
        "the daemon's word is about a tree that no longer exists: it is not served"
    );
    assert!(
        answer.symbols >= 3,
        "the tree was read again, and the new symbol is in the graph: {} symbols",
        answer.symbols
    );
}

#[test]
fn without_a_daemon_the_same_tree_gives_the_same_graph_and_claims_no_daemon() {
    let (_root, workspace, cache) = fixture();
    let (_state, _socket) = start_daemon(&cache, &workspace);

    let with_daemon = ask(&analyzer_with_daemon(&cache, &workspace), &workspace);
    let without_daemon = ask(&analyzer_without_daemon(&cache), &workspace);

    assert_eq!(
        with_daemon.symbols, without_daemon.symbols,
        "the same tree gives the same graph, whoever served it"
    );
    assert_eq!(
        without_daemon.source,
        Some(GraphSource::Cache),
        "without a daemon the product serves the snapshot it checked itself: \
         claiming a daemon served it would be a lie"
    );
}

// ---------------------------------------------------------------------------
// The MCP surface: the daemon's word reaches the published basis
// ---------------------------------------------------------------------------

/// The MCP's context over a workspace, with the daemon it can ask.
///
/// The cache is the test's own: the snapshot is the product's state, and a
/// test that wrote it into the cache of whoever runs it would leave entries
/// nobody can clean.
fn mcp_ctx(
    workspace: &Path,
    cache: &Path,
    oracle: Option<Arc<dyn FreshnessOracle>>,
) -> HandlerContext {
    let mut builder = HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .with_cache_root(cache.to_path_buf());
    if let Some(oracle) = oracle {
        builder = builder.with_freshness_oracle(oracle);
    }
    builder.build()
}

/// The word the MCP publishes about where the graph came from.
///
/// The assertion is over the JSON the client sees, and not over the typed
/// field: a test that names a field that does not exist yet fails to
/// compile, and a test that fails to compile does not say what is missing.
async fn published_source(ctx: &HandlerContext) -> String {
    let output = handle_build_graph(ctx, BuildGraphInput { directory: None })
        .await
        .expect("build_graph answers");
    let value = serde_json::to_value(&output).expect("the answer is JSON");
    value["basis"]["graph_source"]
        .as_str()
        .unwrap_or_else(|| panic!("the basis publishes where the graph came from: {value}"))
        .to_string()
}

#[tokio::test]
async fn the_mcp_publishes_the_daemon_that_served_the_graph() {
    let (_root, workspace, cache) = fixture();
    let (_state, socket) = start_daemon(&cache, &workspace);

    let ctx = mcp_ctx(&workspace, &cache, Some(Arc::new(DaemonOracle::at(socket))));

    assert_eq!(
        published_source(&ctx).await,
        "daemon",
        "a live daemon vouched for the index, and the MCP says who served it"
    );
}

#[tokio::test]
async fn without_a_daemon_the_mcp_does_not_claim_one() {
    let (_root, workspace, cache) = fixture();
    let (_state, _socket) = start_daemon(&cache, &workspace);

    // The daemon is there, but this context does not ask it: the answer
    // has to say that the product checked by itself.
    let ctx = mcp_ctx(&workspace, &cache, None);

    assert_eq!(
        published_source(&ctx).await,
        "cache",
        "without a daemon the MCP serves the snapshot it checked itself: \
         claiming a daemon served it would be a lie"
    );
}

#[tokio::test]
async fn the_mcp_does_not_publish_a_daemon_that_cannot_see_the_change() {
    let (_root, workspace, cache) = fixture();
    let (_state, socket) = start_daemon(&cache, &workspace);

    // The tree changes. The daemon has not rebuilt yet, so it does not
    // know; the client can see the change, so the client decides.
    write_with_mtime(
        &workspace.join("src").join("main.rs"),
        "fn helper() -> u32 { 1 }\nfn other() -> u32 { 2 }\nfn main() { helper(); other(); }\n",
        1_000_000,
    );

    let ctx = mcp_ctx(&workspace, &cache, Some(Arc::new(DaemonOracle::at(socket))));

    assert_eq!(
        published_source(&ctx).await,
        "built",
        "the daemon's word is about a tree that no longer exists: the tree \
         was read again, and the answer says so"
    );
}
