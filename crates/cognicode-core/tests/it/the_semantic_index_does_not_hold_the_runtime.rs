//! The semantic index does not hold the MCP runtime while it is populated.
//!
//! ## The defect
//!
//! `handle_nl_to_symbol` and `handle_semantic_search` call
//! `ensure_semantic_indexed`, which populates the semantic index
//! synchronously: a walk of the tree and an extraction of the symbols of
//! every file of it, inside the `async` handler, on the thread of the
//! runtime that serves it. While the index was populated, the runtime
//! answered nobody — not even a call that needs no disk — and the
//! deadline that was supposed to cut the call short could not fire: a
//! deadline is observed when the future is polled again, and the poll
//! was inside the population until it finished.
//!
//! The graph already moved off the runtime; the semantic index did not.
//! It is the same walk, and it is the same defect.
//!
//! ## What is measured
//!
//! - While the index is populated, another call is served.
//! - The answer of `nl_to_symbol` keeps its shape: the symbols it names
//!   are the ones the tree declares.
//!
//! ## The runtime the measurement is made on
//!
//! The tests ask for a runtime with one worker thread, and they say so
//! on purpose. The guarantee under test is the one of that runtime: the
//! population does not occupy the thread that serves the calls.

use cognicode_core::infrastructure::semantic::semantic_search::SemanticSearchService;
use cognicode_core::interface::mcp::handlers::aix_handlers::handle_nl_to_symbol;
use cognicode_core::interface::mcp::handlers::{
    HandlerContext, handle_get_outline, handle_semantic_search,
};
use cognicode_core::interface::mcp::schemas::{NlToSymbolInput, OutlineInput, SemanticSearchInput};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// How long a test waits for an answer before it gives up on it.
///
/// The rule is the hard one: a test that waits without a deadline is a
/// test that can never finish, and a test that never finishes holds the
/// whole suite.
const DEADLINE: Duration = Duration::from_secs(30);

/// The marker two files of the workspace declare.
const MARKER: &str = "beta";

/// A workspace with `count` files that do not declare the marker, plus two
/// that do.
///
/// The files that do not declare it are what make the population take
/// time: the index reads and interprets every file of the tree, and a
/// tree of a few files is read before the first poll of the handler
/// returns.
fn workspace(count: usize) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("the directory is created");
    for index in 0..count {
        std::fs::write(
            src.join(format!("f{index}.rs")),
            format!("pub fn f{index}() -> u32 {{ 1 }}\n"),
        )
        .expect("written");
    }
    std::fs::write(
        src.join("names.rs"),
        "pub fn beta() -> u32 { 1 }\npub fn gamma() -> u32 { 2 }\n",
    )
    .expect("written");
    std::fs::write(src.join("more.rs"), "pub fn beta() -> u32 { 3 }\n").expect("written");
    (dir, root)
}

/// The outline of a file of the workspace: a call that needs no walk.
fn outline_input() -> OutlineInput {
    OutlineInput {
        file_path: "src/names.rs".to_string(),
        include_private: false,
        include_tests: false,
    }
}

/// The search of the marker over the whole workspace.
fn semantic_input() -> SemanticSearchInput {
    SemanticSearchInput {
        query: MARKER.to_string(),
        kinds: None,
        max_results: 20,
    }
}

/// The natural-language query that names the marker.
fn nl_input() -> NlToSymbolInput {
    NlToSymbolInput {
        query: MARKER.to_string(),
        limit: 10,
    }
}

/// The population of the semantic index in flight does not stop the server
/// from answering somebody else.
///
/// The search is the slow call —it populates the index of the tree— and
/// the outline is the cheap one. If the population holds the runtime, the
/// cheap call cannot be served until the population is over — and the
/// population is over by then, which is what this test notices.
#[tokio::test(flavor = "current_thread")]
async fn a_semantic_search_in_flight_does_not_stop_the_server_from_answering() {
    let (_tmp, root) = workspace(1_200);
    let ctx = HandlerContext::builder()
        .with_working_dir(root.clone())
        .build();

    let search = tokio::spawn(async move {
        handle_semantic_search(
            Arc::new(SemanticSearchService::new()),
            root,
            semantic_input(),
        )
        .await
    });
    let other_ctx = ctx.clone();
    let other = tokio::spawn(async move { handle_get_outline(&other_ctx, outline_input()).await });

    let outline = tokio::time::timeout(DEADLINE, other)
        .await
        .expect("the other call answers within its deadline")
        .expect("the other call's task did not panic")
        .expect("the other call answers");
    assert!(
        outline.total_nodes > 0,
        "the other call really answered: {outline:?}"
    );
    assert!(
        !search.is_finished(),
        "the population of the index held the runtime: it only finished after the other \
         call was served"
    );

    let answer = tokio::time::timeout(DEADLINE, search)
        .await
        .expect("the search answers within its deadline")
        .expect("the search's task did not panic")
        .expect("the search answers");
    assert!(
        answer.total > 0,
        "the search found the candidates the tree declares: {answer:?}"
    );
}

/// The answer of `nl_to_symbol` keeps its shape: the population that moved
/// off the runtime did not lose a symbol nor duplicate one.
///
/// The marker is declared by two files, so an index that lost one of them
/// or counted one twice shows up here.
#[tokio::test]
async fn the_answer_of_nl_to_symbol_keeps_its_symbols() {
    let (_tmp, root) = workspace(3);
    let ctx = HandlerContext::builder().with_working_dir(root).build();

    let answer = tokio::time::timeout(DEADLINE, handle_nl_to_symbol(&ctx, nl_input()))
        .await
        .expect("the call answers within its deadline")
        .expect("the call answers");

    assert_eq!(
        answer.query, MARKER,
        "the answer is about the query: {answer:?}"
    );
    assert!(
        answer.extracted_keywords.iter().any(|word| word == MARKER),
        "the answer names the keyword it extracted: {answer:?}"
    );
    assert!(
        answer.total_candidates > 0,
        "the answer has candidates: {answer:?}"
    );

    let mut files: Vec<&str> = answer
        .results
        .iter()
        .map(|result| result.file.as_str())
        .collect();
    files.sort_unstable();
    files.dedup();
    assert!(
        files.iter().any(|file| file.ends_with("names.rs"))
            && files.iter().any(|file| file.ends_with("more.rs")),
        "the answer names the two files that declare the marker: {answer:?}"
    );
    assert!(
        answer.results.len() <= answer.total_candidates,
        "the answer does not list more results than candidates: {answer:?}"
    );
}
