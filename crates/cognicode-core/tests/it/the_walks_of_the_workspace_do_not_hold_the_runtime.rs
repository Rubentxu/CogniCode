//! The walks of the workspace do not hold the MCP runtime while they run.
//!
//! ## The defect
//!
//! `handle_search_content` and `handle_list_files` called the
//! application synchronously: a walk of the tree and a read of every
//! file of it, inside an `async` handler, on the thread of the runtime
//! that serves it. While the walk ran, the runtime answered nobody else
//! — not even a call that needs no disk — and the deadline that was
//! supposed to cut the call short could not fire: a deadline is
//! observed when the future is polled again, and the poll was inside the
//! walk until it finished. The answer then arrived after the deadline
//! that should have replaced it, and the work that was done was thrown
//! away.
//!
//! ## What is measured
//!
//! - While a walk is in flight, another call is served.
//! - The answers keep their shape: the walk that moved did not lose a
//!   file nor duplicate a match.
//!
//! ## The runtime the measurement is made on
//!
//! The tests ask for a runtime with one worker thread, and they say so
//! on purpose. The guarantee under test is the one of that runtime: the
//! walk does not occupy the thread that serves the calls. On a runtime
//! with several workers the same walk occupies one of them instead of
//! all of them, and the calls that come in a burst are served until the
//! workers run out — which is the shape in which the defect hurts:
//! a burst of walks wedges a server whose workers are all busy
//! reading files.

use cognicode_core::interface::mcp::handlers::{
    HandlerContext, handle_get_outline, handle_list_files, handle_retrieve_and_verify,
    handle_search_content,
};
use cognicode_core::interface::mcp::schemas::{
    ListFilesInput, OutlineInput, RetrieveAndVerifyInput, SearchContentInput,
};
use std::path::PathBuf;
use std::time::Duration;

/// How long a test waits for an answer before it gives up on it.
///
/// The rule is the hard one: a test that waits without a deadline is a
/// test that can never finish, and a test that never finishes holds
/// the whole suite.
const DEADLINE: Duration = Duration::from_secs(30);

/// The marker two files of the workspace name.
const MARKER: &str = "beta";

/// A workspace with `count` files that do not name the marker, plus two
/// that do.
///
/// The files that do not name it are what make the walk take time: the
/// search reads every file of the tree, and a tree of a few files is
/// read before the first poll of the handler returns.
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
    std::fs::write(src.join("more.rs"), "pub fn beta() -> u32 { 2 }\n").expect("written");
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

/// The listing of the whole workspace.
fn list_input() -> ListFilesInput {
    ListFilesInput {
        path: None,
        glob: None,
        offset: Some(0),
        limit: Some(100),
        recursive: Some(true),
        max_depth: None,
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

/// The retrieval of a literal marker over the whole workspace.
///
/// The verification is off: the work of this call is the walk, and the
/// walk is what has to stop occupying the runtime. The verification is
/// the other half of the tool, and it is measured on its own.
fn retrieve_input(query: &str) -> RetrieveAndVerifyInput {
    RetrieveAndVerifyInput {
        query: query.to_string(),
        language: "rust".to_string(),
        max_results: 20,
        verify: false,
    }
}

/// A content search in flight does not stop the server from answering
/// somebody else.
///
/// The search is the slow call and the outline is the cheap one. If the
/// search holds the runtime, the cheap call cannot be served until the
/// search is over — and the search is over by then, which is what this
/// test notices.
#[tokio::test(flavor = "current_thread")]
async fn a_content_search_in_flight_does_not_stop_the_server_from_answering() {
    let (_tmp, root) = workspace(1_200);
    let ctx = HandlerContext::builder().with_working_dir(root).build();

    let search_ctx = ctx.clone();
    let search =
        tokio::spawn(async move { handle_search_content(&search_ctx, search_input(MARKER)).await });
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
    assert_eq!(answer.total, 2, "the search found the two markers");
}

/// A listing in flight does not stop the server from answering somebody
/// else.
///
/// The listing walks the same tree the search walks, and it was the
/// same defect.
#[tokio::test(flavor = "current_thread")]
async fn a_listing_in_flight_does_not_stop_the_server_from_answering() {
    let (_tmp, root) = workspace(1_200);
    let ctx = HandlerContext::builder().with_working_dir(root).build();

    let list_ctx = ctx.clone();
    let listing = tokio::spawn(async move { handle_list_files(&list_ctx, list_input()).await });
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
        !listing.is_finished(),
        "the listing held the runtime: it only finished after the other call was served"
    );

    let answer = tokio::time::timeout(DEADLINE, listing)
        .await
        .expect("the listing answers within its deadline")
        .expect("the listing's task did not panic")
        .expect("the listing answers");
    assert!(
        answer.total > 1_200,
        "the listing walked the tree: {answer:?}"
    );
}

/// A retrieval in flight does not stop the server from answering
/// somebody else.
///
/// The retrieval walks the same tree the search walks, and it walked it
/// in the same place: inside the handler `async`, on the thread of the
/// runtime that serves it. The verification that follows the walk is
/// the other half of the same tool, and it is the half that makes the
/// call long; the walk is the half that occupies the runtime.
#[tokio::test(flavor = "current_thread")]
async fn a_retrieval_in_flight_does_not_stop_the_server_from_answering() {
    let (_tmp, root) = workspace(1_200);
    let ctx = HandlerContext::builder().with_working_dir(root).build();

    let retrieve_ctx = ctx.clone();
    let retrieve = tokio::spawn(async move {
        handle_retrieve_and_verify(&retrieve_ctx, retrieve_input(MARKER)).await
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
        !retrieve.is_finished(),
        "the retrieval held the runtime: it only finished after the other call was served"
    );

    let answer = tokio::time::timeout(DEADLINE, retrieve)
        .await
        .expect("the retrieval answers within its deadline")
        .expect("the retrieval's task did not panic")
        .expect("the retrieval answers");
    assert_eq!(answer.total, 2, "the retrieval found the two markers");
}

/// The answer of the search keeps its shape: the walk that moved off
/// the runtime did not lose a match nor duplicate one.
///
/// The marker is in two files, so a walk that loses one or counts one
/// twice shows up here.
#[tokio::test]
async fn the_answer_of_a_content_search_keeps_its_matches() {
    let (_tmp, root) = workspace(3);
    let ctx = HandlerContext::builder().with_working_dir(root).build();

    let answer = tokio::time::timeout(DEADLINE, handle_search_content(&ctx, search_input(MARKER)))
        .await
        .expect("the search answers within its deadline")
        .expect("the search answers");

    assert_eq!(
        answer.total, 2,
        "the search found the marker in the two files that name it: {answer:?}"
    );
    assert_eq!(
        answer.matches.len(),
        2,
        "the answer has no duplicated match: {answer:?}"
    );
    let mut files: Vec<&str> = answer.matches.iter().map(|m| m.file.as_str()).collect();
    files.sort_unstable();
    assert!(
        files[0].ends_with("more.rs") && files[1].ends_with("names.rs"),
        "the matches are the two files that name the marker: {answer:?}"
    );
    assert_eq!(
        answer.files_scanned, 5,
        "the search read every file of the tree: {answer:?}"
    );
    // The word "complete" says that nothing was left unread. It does
    // not say that the walk was not truncated: a search that stops at
    // `max_results` publishes the same word. This one did not stop
    // early — two matches, and room for fifty.
    assert!(
        answer.matches.len() < 50,
        "the search was not truncated by its own limit: {answer:?}"
    );
    assert_eq!(
        answer.coverage.status, "complete",
        "the search read every file it could read: {:?}",
        answer.coverage
    );
}

/// The answer of the listing keeps its shape: the walk that moved off
/// the runtime did not lose a file.
#[tokio::test]
async fn the_answer_of_a_listing_keeps_its_files() {
    let (_tmp, root) = workspace(3);
    let ctx = HandlerContext::builder().with_working_dir(root).build();

    let answer = tokio::time::timeout(DEADLINE, handle_list_files(&ctx, list_input()))
        .await
        .expect("the listing answers within its deadline")
        .expect("the listing answers");

    // The root, the directory of the sources, the three files that do
    // not name the marker and the two that do.
    assert_eq!(
        answer.total, 7,
        "the listing walked the whole tree: {answer:?}"
    );
    let mut paths: Vec<&str> = answer.files.iter().map(|f| f.path.as_str()).collect();
    paths.sort_unstable();
    assert!(
        paths.iter().any(|p| p.ends_with("names.rs")),
        "the listing names the file that names the marker: {answer:?}"
    );
    assert!(
        paths.iter().any(|p| p.ends_with("more.rs")),
        "the listing names the second file that names the marker: {answer:?}"
    );
    assert_eq!(
        paths.len(),
        7,
        "the listing has no duplicated entry: {answer:?}"
    );
    assert_eq!(
        answer.coverage.status, "complete",
        "the listing covered the tree it walked: {:?}",
        answer.coverage
    );
}
