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
        cursor: None,
        offset: None,
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
async fn paginated_content_search_concatenates_to_the_unpaginated_order() {
    let (_tmp, root) = workspace(5);
    let ctx = HandlerContext::builder().with_working_dir(root).build();
    let all = handle_search_content(&ctx, search_input(50))
        .await
        .expect("full search");
    let first = handle_search_content(&ctx, search_input(2))
        .await
        .expect("first page");
    let first_json = serde_json::to_value(&first).expect("serialize page");
    let cursor = first_json
        .get("next_cursor")
        .and_then(serde_json::Value::as_str)
        .expect("a truncated page publishes a continuation cursor");
    let second_input = serde_json::from_value(serde_json::json!({
        "pattern": MARKER, "regex": false, "max_results": 2, "context_lines": 0,
        "cursor": cursor
    }))
    .expect("continuation input");
    let second = handle_search_content(&ctx, second_input)
        .await
        .expect("second page");
    let third_json = serde_json::to_value(&second).expect("serialize second page");
    let third_cursor = third_json
        .get("next_cursor")
        .and_then(serde_json::Value::as_str)
        .expect("another page remains");
    let third_input = serde_json::from_value(serde_json::json!({
        "pattern": MARKER, "regex": false, "max_results": 2, "context_lines": 0,
        "cursor": third_cursor
    }))
    .expect("third page input");
    let third = handle_search_content(&ctx, third_input)
        .await
        .expect("third page");
    assert!(third.next_cursor.is_none(), "last page has no continuation");
    let mut pages = first.matches;
    pages.extend(second.matches);
    pages.extend(third.matches);
    assert_eq!(
        serde_json::to_value(pages).expect("serialize paginated matches"),
        serde_json::to_value(all.matches).expect("serialize full matches")
    );
}

#[tokio::test]
async fn continuation_is_refused_after_the_search_tree_changes() {
    let (_tmp, root) = workspace(5);
    let ctx = HandlerContext::builder()
        .with_working_dir(root.clone())
        .build();
    let first = handle_search_content(&ctx, search_input(2))
        .await
        .expect("first page");
    let cursor = serde_json::to_value(&first).expect("serialize page")["next_cursor"]
        .as_str()
        .expect("continuation cursor")
        .to_owned();
    std::fs::write(root.join("src/new.rs"), "pub fn beta() {}\n").expect("mutate tree");
    let continuation = serde_json::from_value(serde_json::json!({
        "pattern": MARKER, "regex": false, "max_results": 2, "cursor": cursor
    }))
    .expect("continuation input");
    let error = handle_search_content(&ctx, continuation)
        .await
        .expect_err("changed tree rejects continuation");
    assert!(
        error.to_string().to_lowercase().contains("revision"),
        "{error}"
    );
}

#[tokio::test]
async fn same_size_edits_and_changed_queries_reject_a_cursor() {
    let (_tmp, root) = workspace(5);
    let ctx = HandlerContext::builder()
        .with_working_dir(root.clone())
        .build();
    let first = handle_search_content(&ctx, search_input(2)).await.unwrap();
    let cursor = first.next_cursor.unwrap();
    let mut changed_query = search_input(2);
    changed_query.cursor = Some(cursor.clone());
    changed_query.pattern = "f0".into();
    assert!(
        handle_search_content(&ctx, changed_query)
            .await
            .unwrap_err()
            .to_string()
            .contains("definition")
    );
    let path = root.join("src/f0.rs");
    let content = std::fs::read_to_string(&path).unwrap();
    let edited = content.replace("beta", "zeta");
    assert_eq!(content.len(), edited.len());
    std::fs::write(path, edited).unwrap();
    let mut continuation = search_input(2);
    continuation.cursor = Some(cursor);
    assert!(
        handle_search_content(&ctx, continuation)
            .await
            .unwrap_err()
            .to_string()
            .contains("revision")
    );
}

#[tokio::test]
async fn offset_and_zero_limit_have_explicit_boundaries() {
    let (_tmp, root) = workspace(5);
    let ctx = HandlerContext::builder().with_working_dir(root).build();
    let all = handle_search_content(&ctx, search_input(50)).await.unwrap();
    let mut input = search_input(2);
    input.offset = Some(2);
    let page = handle_search_content(&ctx, input).await.unwrap();
    assert_eq!(
        serde_json::to_value(page.matches).unwrap(),
        serde_json::to_value(&all.matches[2..4]).unwrap()
    );
    assert!(page.next_cursor.is_some());
    assert!(handle_search_content(&ctx, search_input(0)).await.is_err());
}

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
