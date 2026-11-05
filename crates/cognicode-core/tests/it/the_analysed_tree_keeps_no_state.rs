//! The product's state does not live in the tree it analyses.
//!
//! ## What this measures
//!
//! The durable snapshot of the graph is derived from the tree: it can be
//! thrown away without losing anything, and it says nothing about the tree
//! itself. It used to be written **inside** the tree —
//! `<repo>/.cognicode/graph.cache` — so analysing a repository wrote into
//! it. Analysing a project is a read, and a read that leaves a file behind
//! is not a read.
//!
//! The snapshot now lives in the user's cache, keyed by the workspace: two
//! repositories never share an entry, and the analysed tree is left exactly
//! as it was found.
//!
//! ## Why the assertion is about the tree
//!
//! The guarantee is not "the snapshot is somewhere else": it is that the
//! repository the consumer asked about comes back the way it was given. A
//! test that only looked at the new location would pass while the old one
//! was still being written, so this one looks at the tree — and at the
//! cache, so that "nothing in the tree" cannot be satisfied by "nothing
//! anywhere".

use cognicode_core::application::graph_cache::snapshot_path;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};
use std::path::Path;

/// A context over a workspace, with the cache the caller names.
///
/// The cache is the test's own: a test that wrote in the cache of the
/// user running it would leave entries nobody can clean.
fn ctx_for(workspace: &Path, cache: &Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .with_cache_root(cache.to_path_buf())
        .build()
}

/// A workspace with one source file, and the temporary root that holds it.
fn workspace_with_root() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().expect("a temporary root");
    let ws = root.path().join("ws");
    std::fs::create_dir_all(&ws).expect("the workspace is created");
    std::fs::write(ws.join("a.rs"), "fn a() {}\n").expect("the source is written");
    (root, ws)
}

/// The names the product wrote inside the analysed tree.
fn product_state_inside(workspace: &Path) -> Vec<String> {
    std::fs::read_dir(workspace)
        .expect("the workspace can be read")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name == ".cognicode")
        .collect()
}

#[tokio::test]
async fn the_analysed_tree_keeps_no_product_state() {
    let (_root, workspace) = workspace_with_root();
    let cache = tempfile::tempdir().expect("a temporary cache");

    let ctx = ctx_for(&workspace, cache.path());
    handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("build_graph answers");

    let leftovers = product_state_inside(&workspace);
    assert!(
        leftovers.is_empty(),
        "analysing a repository leaves it as it was found: the product wrote \
         {leftovers:?} inside the tree it was asked to read. The snapshot is \
         the product's own state and belongs in the user's cache."
    );

    assert!(
        snapshot_path(cache.path(), &workspace).exists(),
        "the snapshot is kept, in the cache the session named: a product that \
         keeps nothing would not be a product that reuses what it derived."
    );
}

#[tokio::test]
async fn each_workspace_has_its_own_entry_in_the_cache() {
    let (_root_a, workspace_a) = workspace_with_root();
    let (_root_b, workspace_b) = workspace_with_root();
    let cache = tempfile::tempdir().expect("a temporary cache");

    for workspace in [&workspace_a, &workspace_b] {
        let ctx = ctx_for(workspace, cache.path());
        handle_build_graph(&ctx, BuildGraphInput { directory: None })
            .await
            .expect("build_graph answers");
    }

    let path_a = snapshot_path(cache.path(), &workspace_a);
    let path_b = snapshot_path(cache.path(), &workspace_b);
    assert!(
        path_a.exists() && path_b.exists(),
        "each workspace has its own entry in the cache: {path_a:?}, {path_b:?}"
    );
    assert_ne!(
        path_a, path_b,
        "two repositories never share an entry in the cache"
    );
}

#[tokio::test]
async fn a_workspace_without_a_cache_keeps_no_state_anywhere() {
    let (_root, workspace) = workspace_with_root();

    // A session that was not told where to keep state keeps none: it
    // rebuilds on every request instead of writing into a directory
    // nobody chose.
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build();
    handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("build_graph answers");

    assert!(
        product_state_inside(&workspace).is_empty(),
        "a session without a cache does not write into the tree either"
    );
    assert_eq!(
        ctx.cache_root(),
        None,
        "a context that was not told where to keep state keeps none"
    );
}
