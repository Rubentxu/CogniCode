//! The public routes do not hold the runtime that serves them.
//!
//! ## The defect
//!
//! TASK-297 moved the walk of the content search and the walk of the
//! listing off the runtime — in the handlers of the MCP. The published
//! routes of the crate that do the same thing were left as they were:
//!
//! ```text
//! pub async fn search_content(&self, request: SearchContentRequest) -> ... {
//!     self.file_ops.search_content(request)   // synchronous, on the runtime
//! }
//! ```
//!
//! They are `async` by signature, but the work does not yield: the walk of
//! the tree and the read of every file of it happen inside the call, on the
//! thread of the runtime that serves it. A consumer that uses
//! `cognicode-core` as a library — the SDK, a consumer that publishes its
//! own MCP surface — gets the same defect the product no longer has.
//!
//! ## What is measured
//!
//! - While a published route is in flight, another one is served.
//! - The answers keep their shape: the walk that moved did not lose a file
//!   nor duplicate a match.
//!
//! ## The runtime the measurement is made on
//!
//! The tests ask for a runtime with one worker thread, and they say so on
//! purpose. The guarantee under test is the one of that runtime: the walk
//! does not occupy the thread that serves the calls. On a runtime with
//! several workers the same walk occupies one of them instead of all of
//! them, and the calls that come in a burst are served until the workers
//! run out — which is the shape in which the defect hurts: a burst of
//! walks wedges a server whose workers are all busy reading files.

use cognicode_core::application::dto::{ListFilesRequest, ReadFileRequest, SearchContentRequest};
use cognicode_core::application::workspace_session::WorkspaceSession;
use cognicode_core::interface::composition::default_capabilities;
use cognicode_core::interface::mcp::security::InputValidator;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// How long a test waits for an answer before it gives up on it.
///
/// The rule is the hard one: a test that waits without a deadline is a
/// test that can never finish, and a test that never finishes holds the
/// whole suite.
const DEADLINE: Duration = Duration::from_secs(30);

/// The marker two files of the workspace name.
const MARKER: &str = "beta";

/// A workspace with `count` files that do not name the marker, plus two
/// that do.
///
/// The files that do not name it are what make the walk take time: the
/// search reads every file of the tree, and a tree of a few files is read
/// before the first poll of the route returns.
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

/// The session over a workspace.
///
/// The session is composed the way the product composes it: the
/// capabilities come from the interface layer, because the application is
/// not allowed to name the implementations.
async fn session(root: &PathBuf) -> WorkspaceSession {
    let policy = Arc::new(InputValidator::new().with_workspace(vec![root.clone()]));
    let capabilities = default_capabilities(root, policy);
    WorkspaceSession::with_capabilities(root, capabilities)
        .await
        .expect("the session is built")
}

/// The search of a literal marker, over the whole workspace.
fn search_input(pattern: &str) -> SearchContentRequest {
    SearchContentRequest {
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
fn list_input() -> ListFilesRequest {
    ListFilesRequest {
        path: None,
        glob: None,
        offset: Some(0),
        limit: Some(100),
        recursive: Some(true),
        max_depth: None,
    }
}

/// The read of one file of the workspace: a call that needs no walk.
fn read_input() -> ReadFileRequest {
    ReadFileRequest::new("src/names.rs")
}

/// A public search in flight does not stop the session from answering
/// somebody else.
///
/// The search is the slow call and the read is the cheap one. If the
/// search holds the runtime, the cheap call cannot be served until the
/// search is over — and the search is over by then, which is what this
/// test notices.
#[tokio::test(flavor = "current_thread")]
async fn a_public_search_in_flight_does_not_stop_the_session_from_answering() {
    let (_tmp, root) = workspace(1_200);
    let session = Arc::new(session(&root).await);

    let search_session = Arc::clone(&session);
    let search =
        tokio::spawn(async move { search_session.search_content(search_input(MARKER)).await });
    let read_session = Arc::clone(&session);
    let read = tokio::spawn(async move { read_session.read_file(read_input()).await });

    let answered = tokio::time::timeout(DEADLINE, read)
        .await
        .expect("the other call answers within its deadline")
        .expect("the other call's task did not panic")
        .expect("the other call answers");
    assert!(
        answered.content.contains("beta"),
        "the other call really answered: {answered:?}"
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

/// A public listing in flight does not stop the session from answering
/// somebody else.
///
/// The listing walks the same tree the search walks, and it was the same
/// defect.
#[tokio::test(flavor = "current_thread")]
async fn a_public_listing_in_flight_does_not_stop_the_session_from_answering() {
    let (_tmp, root) = workspace(1_200);
    let session = Arc::new(session(&root).await);

    let list_session = Arc::clone(&session);
    let listing = tokio::spawn(async move { list_session.list_files(list_input()).await });
    let read_session = Arc::clone(&session);
    let read = tokio::spawn(async move { read_session.read_file(read_input()).await });

    let answered = tokio::time::timeout(DEADLINE, read)
        .await
        .expect("the other call answers within its deadline")
        .expect("the other call's task did not panic")
        .expect("the other call answers");
    assert!(
        answered.content.contains("beta"),
        "the other call really answered: {answered:?}"
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

/// The answer of the public search keeps its shape: the walk that moved off
/// the runtime did not lose a match nor duplicate one.
///
/// The marker is in two files, so a walk that loses one or counts one
/// twice shows up here.
#[tokio::test]
async fn the_answer_of_a_public_search_keeps_its_matches() {
    let (_tmp, root) = workspace(3);
    let session = session(&root).await;

    let answer = tokio::time::timeout(DEADLINE, session.search_content(search_input(MARKER)))
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
}

/// A published route that is awaited where there is no runtime answers all
/// the same.
///
/// The session is built inside a runtime and the call is awaited on a
/// plain thread, where there is no runtime to send the work to. There the
/// route has to do the walk where it is — and answer, because a published
/// route that panics for want of a runtime is not a published route.
///
/// The wait has no deadline of its own: the route answers in the first
/// poll, and a route that yields here would never answer. The poll
/// below is the deadline.
#[test]
fn a_published_route_awaited_where_there_is_no_runtime_answers_all_the_same() {
    let (_tmp, root) = workspace(3);
    let runtime = tokio::runtime::Runtime::new().expect("the runtime is built");
    let session = runtime.block_on(session(&root));

    let answer = std::thread::spawn(move || {
        let mut future = Box::pin(session.search_content(search_input(MARKER)));
        let waker = std::task::Waker::noop();
        let mut context = std::task::Context::from_waker(waker);
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!(
                "the route yielded where there is no runtime to send the work to, \
                 and a route that waits for a runtime that is not there never answers"
            ),
        }
    })
    .join()
    .expect("the thread that awaited the route did not panic")
    .expect("the route answers");

    assert_eq!(
        answer.total, 2,
        "the route answered with the same matches it answers with a runtime: {answer:?}"
    );
}
