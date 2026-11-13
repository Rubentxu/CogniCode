//! The search index follows the files: what a file no longer declares, and
//! what a file that is gone declared, stop being searchable.
//!
//! ## The gap
//!
//! `reparse_on_edit` brings the index up to date with an edit. The index it
//! fed was a map keyed by `name:line:column`, with three consequences that are
//! the same defect seen from three sides:
//!
//! - **A symbol that was renamed stayed.** Indexing a file again inserted the
//!   new symbols and never removed the ones the file used to declare, so a
//!   rename left both names searchable, and only one of them was in the code.
//! - **Two files collided.** The key had no file in it, so two files that
//!   declare a symbol with the same name at the same line and column (every
//!   `fn new()` that opens an `impl` at the same offset) were one entry, and
//!   the second silently replaced the first.
//! - **A deleted file was never forgotten.** The removal path measured the
//!   symbols that went away and rebuilt the graph, and never told the index.
//!
//! ## What this file pins
//!
//! Each case is read through the search a client calls (`smart_search` for the
//! edits, `semantic_search` for the collision, because `smart_search` merges
//! its backends by name). The premise of each case is observed before the
//! edit and not assumed.
//!
//! Nothing here sleeps, and every call carries its own deadline.

#![cfg(feature = "persistence")]

use cognicode_core::infrastructure::semantic::semantic_search::SemanticSearchService;
use cognicode_core::interface::mcp::handlers::aix_handlers::handle_reparse_on_edit;
use cognicode_core::interface::mcp::handlers::consolidated_handlers::handle_smart_search;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph, handle_semantic_search,
};
use cognicode_core::interface::mcp::schemas::{
    ReparseOnEditInput, SemanticSearchInput, SmartSearchInput,
};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

/// How long a call that must answer is given to answer. The healthy path takes
/// milliseconds; the deadline only decides how long a regression takes to be
/// reported.
const DEADLINE: Duration = Duration::from_secs(30);

/// Runs `call` under the deadline, so a hang is a failure and not a stall.
async fn within_the_deadline<T>(call: impl Future<Output = T>) -> T {
    tokio::time::timeout(DEADLINE, call)
        .await
        .expect("the call did not answer within the deadline")
}

/// A workspace with `files` (`path`, `source`) built into a complete graph.
async fn analysed_workspace(files: &[(&str, &str)]) -> (tempfile::TempDir, HandlerContext) {
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    for (path, source) in files {
        let full = workspace.path().join(path);
        std::fs::create_dir_all(full.parent().expect("a file has a parent"))
            .expect("the directory is created");
        std::fs::write(full, source).expect("the source is written");
    }
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .build();
    let build = within_the_deadline(handle_build_graph(
        &ctx,
        BuildGraphInput { directory: None },
    ))
    .await
    .expect("build_graph answers");
    assert_eq!(
        build.status, "complete",
        "the graph has to be complete first: otherwise the manifest is not saved and \
         a deleted file is not one the manifest knew"
    );
    (workspace, ctx)
}

/// The `(name, file)` pairs `smart_search` answers for `query`, sorted.
///
/// Only the hits whose name IS the query are kept: the fuzzy neighbours are
/// not what these cases are about.
async fn found(ctx: &HandlerContext, query: &str) -> Vec<(String, String)> {
    let answer = within_the_deadline(handle_smart_search(
        ctx,
        SmartSearchInput {
            query: query.to_string(),
            limit: Some(20),
        },
    ))
    .await
    .expect("smart_search answers");
    let mut hits: Vec<(String, String)> = answer
        .results
        .iter()
        .filter(|hit| hit.name == query)
        .map(|hit| (hit.name.clone(), hit.file.clone().unwrap_or_default()))
        .collect();
    hits.sort();
    hits.dedup();
    hits
}

async fn reparse(ctx: &HandlerContext, files: &[&str]) {
    within_the_deadline(handle_reparse_on_edit(
        ctx,
        ReparseOnEditInput {
            file_paths: files.iter().map(|file| file.to_string()).collect(),
            edit_ranges: None,
        },
    ))
    .await
    .expect("reparse_on_edit answers");
}

#[tokio::test]
async fn a_symbol_that_was_renamed_is_not_searchable_under_its_old_name() {
    let (workspace, ctx) = analysed_workspace(&[
        ("src/a.rs", "fn old_name() {}\nfn stays() {}\n"),
        ("src/b.rs", "fn other() {}\n"),
    ])
    .await;
    assert_eq!(
        found(&ctx, "old_name").await.len(),
        1,
        "the premise: the symbol is searchable before the edit"
    );

    // The same declaration, renamed: the position does not change, the name does.
    std::fs::write(
        workspace.path().join("src/a.rs"),
        "fn new_name() {}\nfn stays() {}\n",
    )
    .expect("the file is edited");
    reparse(&ctx, &["src/a.rs"]).await;

    assert!(
        found(&ctx, "old_name").await.is_empty(),
        "the file no longer declares `old_name` and search still finds it: {:?}",
        found(&ctx, "old_name").await
    );
    assert_eq!(
        found(&ctx, "new_name").await.len(),
        1,
        "the new name is searchable"
    );
    assert_eq!(
        found(&ctx, "stays").await.len(),
        1,
        "what the edit did not touch is still searchable"
    );
}

#[tokio::test]
async fn two_files_that_declare_the_same_name_at_the_same_place_are_two_results() {
    // `smart_search` merges its backends by name, so the question of whether
    // two files are two entries is asked of the search that lists them.
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    std::fs::create_dir_all(workspace.path().join("src")).expect("the directory is created");
    for file in ["src/a.rs", "src/b.rs"] {
        std::fs::write(workspace.path().join(file), "fn shared_name() {}\n")
            .expect("the source is written");
    }

    let answer = within_the_deadline(handle_semantic_search(
        Arc::new(SemanticSearchService::new()),
        workspace.path().to_path_buf(),
        SemanticSearchInput {
            query: "shared_name".to_string(),
            kinds: None,
            max_results: 20,
        },
    ))
    .await
    .expect("semantic_search answers");
    let files: Vec<&str> = answer
        .results
        .iter()
        .filter(|hit| hit.name == "shared_name")
        .map(|hit| hit.file.as_str())
        .collect();

    assert_eq!(
        files.len(),
        2,
        "two files declare `shared_name` at the same line and column and search lists \
         {} of them: {files:?}",
        files.len()
    );
    assert!(
        files.iter().any(|file| file.ends_with("src/a.rs"))
            && files.iter().any(|file| file.ends_with("src/b.rs")),
        "both files are listed: {files:?}"
    );
}

#[tokio::test]
async fn a_deleted_file_is_not_searchable_after_the_reparse() {
    let (workspace, ctx) = analysed_workspace(&[
        ("src/gone.rs", "fn vanished_symbol() {}\n"),
        ("src/kept.rs", "fn surviving_symbol() {}\n"),
    ])
    .await;
    assert_eq!(
        found(&ctx, "vanished_symbol").await.len(),
        1,
        "the premise: the symbol is searchable before the file is deleted"
    );

    std::fs::remove_file(workspace.path().join("src/gone.rs")).expect("the file is deleted");
    reparse(&ctx, &["src/gone.rs"]).await;

    assert!(
        found(&ctx, "vanished_symbol").await.is_empty(),
        "the file is gone and search still finds its symbol: {:?}",
        found(&ctx, "vanished_symbol").await
    );
    assert_eq!(
        found(&ctx, "surviving_symbol").await.len(),
        1,
        "the file that was not deleted is still searchable"
    );
}
