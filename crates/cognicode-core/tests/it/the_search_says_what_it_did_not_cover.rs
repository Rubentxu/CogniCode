//! The search that stops at its limit says what it did not cover.
//!
//! ## The defect
//!
//! The content search stops the walk when its matches reach
//! `max_results`, and it stops without annotating anything: the answer
//! publishes `files_scanned` below the number of files of the tree **and**
//! `coverage: "complete"`.
//!
//! The coverage of a search is derived from the holes of the walk, and a
//! walk that stops on purpose leaves no hole: "complete" ends up meaning
//! "nothing was unreadable", which reads as "the search covered the
//! tree". It did not: the tree may hold more matches, and the caller
//! cannot tell.
//!
//! ## What is measured
//!
//! - A search that reaches its limit publishes a coverage that is not
//!   "complete", and the reason names the limit.
//! - A search that fits in its limit still publishes "complete": the fix
//!   does not make every answer partial.
//!
//! ## The shape of the answer does not change
//!
//! The truncation is published as a hole of the walk, the same way the
//! walk that stops because a budget ran out publishes itself. The
//! published shape of the answer is the one it had, and the catalogue and
//! the runtime capture still describe it.

use cognicode_core::interface::mcp::handlers::{HandlerContext, handle_search_content};
use cognicode_core::interface::mcp::schemas::SearchContentInput;
use std::path::PathBuf;

/// How long a test waits for an answer before it gives up on it.
///
/// The rule is the hard one: a test that waits without a deadline is a
/// test that can never finish, and a test that never finishes holds the
/// whole suite.
const DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

/// The marker every file of the workspace names.
const MARKER: &str = "beta";

/// A workspace with `count` files that name the marker.
fn workspace(count: usize) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("the directory is created");
    for index in 0..count {
        std::fs::write(
            src.join(format!("f{index}.rs")),
            format!("pub fn f{index}() -> u32 {{ 1 }}\npub fn {MARKER}() -> u32 {{ 2 }}\n"),
        )
        .expect("written");
    }
    (dir, root)
}

/// The search of the marker over the whole workspace, with a limit.
fn search_input(max_results: usize) -> SearchContentInput {
    SearchContentInput {
        pattern: MARKER.to_string(),
        path: None,
        file_glob: None,
        regex: Some(false),
        case_insensitive: Some(false),
        max_results: Some(max_results),
        context_lines: Some(0),
    }
}

/// A search that reaches its limit does not say it covered the tree.
///
/// The tree holds five matches and the search asks for one: the walk
/// stops with four files unread, and the answer says "complete" — which
/// reads as "the tree holds one match".
#[tokio::test]
async fn a_search_that_reaches_its_limit_does_not_say_it_covered_the_tree() {
    let (_tmp, root) = workspace(5);
    let ctx = HandlerContext::builder().with_working_dir(root).build();

    let answer = tokio::time::timeout(DEADLINE, handle_search_content(&ctx, search_input(1)))
        .await
        .expect("the search answers within its deadline")
        .expect("the search answers");

    assert_eq!(
        answer.total, 1,
        "the search stopped at its limit: {answer:?}"
    );
    assert!(
        answer.coverage.status != "complete",
        "the search says it covered the tree it stopped walking: {answer:?}"
    );
    assert!(
        answer
            .coverage
            .skipped_files
            .iter()
            .any(|file| file.reason.contains("max_results")),
        "the answer says which limit stopped it: {answer:?}"
    );
}

/// A search that fits in its limit still says it covered the tree.
///
/// The fix must not turn every answer into a partial one: a search that
/// read the whole tree and found what it was looking for covers it.
#[tokio::test]
async fn a_search_that_fits_in_its_limit_says_it_covered_the_tree() {
    let (_tmp, root) = workspace(3);
    let ctx = HandlerContext::builder().with_working_dir(root).build();

    let answer = tokio::time::timeout(DEADLINE, handle_search_content(&ctx, search_input(50)))
        .await
        .expect("the search answers within its deadline")
        .expect("the search answers");

    assert_eq!(answer.total, 3, "the search found every match: {answer:?}");
    assert_eq!(
        answer.coverage.status, "complete",
        "the search read the whole tree: {:?}",
        answer.coverage
    );
}
