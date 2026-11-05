//! A new session does not calculate what the last one already calculated.
//!
//! ## What this measures
//!
//! The product keeps a durable snapshot of the graph in the user's cache. It
//! is written by `build_graph` and it is there for one reason: a session that
//! starts later should not have to parse the whole tree again to answer the
//! same question.
//!
//! It did not work that way. `ensure_graph_built` —the function every query
//! tool calls before it reads the graph— rebuilt the graph from the sources
//! whenever the session found its own graph empty, which is exactly what a
//! fresh session does. The snapshot was written and never read.
//!
//! ## Why the assertion is on the published summary
//!
//! The observable is what the consumer receives. The impact summary says
//! `Graph auto-built (…)` when the graph was calculated during the call and
//! says nothing of the sort when the graph was already there. A consumer
//! therefore reads from the answer itself whether the work was repeated, and
//! this test asserts on that answer rather than on an internal flag.

use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_analyze_impact, handle_build_graph,
};
use cognicode_core::interface::mcp::schemas::AnalyzeImpactInput;
use std::path::Path;

/// A context over a workspace, with the cache the caller names.
///
/// The cache is the test's own: the snapshot is the product's state, and a
/// test that wrote it in the cache of whoever runs the suite would leave
/// entries nobody can clean.
fn ctx_for(workspace: &Path, cache: &Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .with_cache_root(cache.to_path_buf())
        .build()
}

/// A workspace with one caller and one callee, and the temporary root that
/// holds it.
fn workspace_with_root() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let root = tempfile::tempdir().expect("a temporary root");
    let ws = root.path().join("ws");
    std::fs::create_dir_all(&ws).expect("the workspace is created");
    std::fs::write(
        ws.join("a.rs"),
        "fn callee() -> u32 { 1 }\nfn caller() -> u32 { callee() }\n",
    )
    .expect("the source is written");
    let cache = root.path().join("cache");
    (root, ws, cache)
}

/// The impact of `callee`, as the consumer receives it.
async fn impact(ctx: &HandlerContext, symbol: &str) -> String {
    let output = handle_analyze_impact(
        ctx,
        AnalyzeImpactInput {
            symbol_name: symbol.to_string(),
            compressed: false,
        },
    )
    .await
    .expect("the impact answers");
    output.summary
}

#[tokio::test]
async fn a_new_session_serves_the_snapshot_instead_of_rebuilding() {
    let (_root, workspace, cache) = workspace_with_root();

    // The first session calculates the graph and leaves the snapshot behind.
    let first = ctx_for(&workspace, &cache);
    handle_build_graph(&first, BuildGraphInput { directory: None })
        .await
        .expect("build_graph answers");

    // The second session is a new one: its own context, its own empty
    // in-memory graph. What it does NOT have is a reason to calculate the
    // tree again.
    let second = ctx_for(&workspace, &cache);
    let summary = impact(&second, "callee").await;

    assert!(
        !summary.contains("auto-built"),
        "the second session served the graph the first one calculated, and \
         the answer says so: a summary that says the graph was built again \
         claims work that was not repeated. Summary: {summary}"
    );
    assert!(
        summary.contains("Impact analysis completed"),
        "the answer is still an answer about the impact: {summary}"
    );
}

#[tokio::test]
async fn the_session_that_calculated_the_graph_still_says_it_did() {
    let (_root, workspace, cache) = workspace_with_root();

    // A session that finds the graph empty calculates it, and says so:
    // the answer has to distinguish "I calculated this" from "I found this".
    let ctx = ctx_for(&workspace, &cache);
    let summary = impact(&ctx, "callee").await;

    assert!(
        summary.contains("auto-built"),
        "the first session calculated the graph, and the answer says so: a \
         summary that stays silent would leave the consumer unable to tell \
         a calculated answer from a found one. Summary: {summary}"
    );
}
