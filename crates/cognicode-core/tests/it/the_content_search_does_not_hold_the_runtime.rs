//! The content search does not hold the MCP runtime while it runs.
//!
//! ## The defect
//!
//! `handle_search_content` called the application synchronously: a walk
//! of the tree and a read of every file of it, inside an `async`
//! handler, on the thread of the runtime that serves it. While the
//! walk ran, the server answered nobody else — not even a call that
//! needs no disk — and the deadline that was supposed to cut the call
//! short could not fire: a deadline is observed when the future is
//! polled again, and the poll was inside the walk until it finished.
//! The answer then arrived after the deadline that should have
//! replaced it, and the work that was done was thrown away.
//!
//! ## What is measured
//!
//! - While a search is in flight, another call is served.
//! - The answer keeps its shape and its coverage: the walk that moved
//!   did not lose a match nor duplicate one.

use cognicode_core::interface::mcp::handlers::{
    HandlerContext, handle_get_outline, handle_search_content,
};
use cognicode_core::interface::mcp::schemas::{OutlineInput, SearchContentInput};
use std::path::PathBuf;
use std::time::Duration;

/// How long a test waits for an answer before it gives up on it.
///
/// The rule is the hard one: a test that waits without a deadline is a
/// test that can never finish, and a test that never finishes holds
/// the whole suite.
const DEADLINE: Duration = Duration::from_secs(30);

/// A workspace with `count` files that do not name the marker, plus one
/// that does.
///
/// The files that do not name it are what make the search take time:
/// the search reads every file of the tree, and a tree of a few files
/// is read before the first poll of the handler returns.
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
    std::fs::write(src.join("names.rs"), "pub fn beta() -> u32 { 1 }\n").expect("written");
    (dir, root)
}

/// The search of a literal marker, over the whole workspace.
fn search_input(pattern: &str) -> SearchContentInput {
    SearchContentInput {
        pattern: pattern.to_string(),
        path: None,
        file_glob: None,
        regex: Some(false),
        case_insensitive: Some(false),
        max_results: Some(50),
        context_lines: Some(0),
    }
}

/// The outline of a file of the workspace: a call that needs no walk.
fn outline_input() -> OutlineInput {
    OutlineInput {
        file_path: "src/names.rs".to_string(),
        include_private: false,
        include_tests: false,
    }
}

/// A search in flight does not stop the server from answering somebody
/// else.
///
/// The search is the slow call and the outline is the cheap one. If the
/// search holds the runtime, the cheap call cannot be served until the
/// search is over — and the search is over by then, which is what this
/// test notices.
#[tokio::test]
async fn a_content_search_in_flight_does_not_stop_the_server_from_answering() {
    let (_tmp, root) = workspace(1_200);
    let ctx = HandlerContext::builder().with_working_dir(root).build();

    let search_ctx = ctx.clone();
    let search =
        tokio::spawn(async move { handle_search_content(&search_ctx, search_input("beta")).await });
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
        "the search held the runtime: it only finished after the other call was served"
    );

    let answer = tokio::time::timeout(DEADLINE, search)
        .await
        .expect("the search answers within its deadline")
        .expect("the search's task did not panic")
        .expect("the search answers");
    assert_eq!(answer.total, 1, "the search found the marker");
}

/// The answer of the search keeps its shape and its coverage: the walk
/// that moved off the runtime did not lose a match nor duplicate one.
#[tokio::test]
async fn the_answer_of_a_content_search_keeps_its_matches_and_its_coverage() {
    let (_tmp, root) = workspace(3);
    let ctx = HandlerContext::builder().with_working_dir(root).build();

    let answer = tokio::time::timeout(DEADLINE, handle_search_content(&ctx, search_input("beta")))
        .await
        .expect("the search answers within its deadline")
        .expect("the search answers");

    assert_eq!(
        answer.total, 1,
        "the search found the one match: {answer:?}"
    );
    assert_eq!(
        answer.matches.len(),
        1,
        "the answer has no duplicated match: {answer:?}"
    );
    assert!(
        answer.matches[0].file.ends_with("names.rs"),
        "the match is the one that names the marker: {answer:?}"
    );
    assert_eq!(
        answer.files_scanned, 4,
        "the search read every file of the tree: {answer:?}"
    );
    assert_eq!(
        answer.coverage.status, "complete",
        "the search covered the tree it walked: {:?}",
        answer.coverage
    );
}
