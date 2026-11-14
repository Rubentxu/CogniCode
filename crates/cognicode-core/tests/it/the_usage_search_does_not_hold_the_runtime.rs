//! The usage search does not hold the MCP runtime while it runs.
//!
//! ## The defect
//!
//! `handle_find_usages` called the search directly: a blocking call
//! inside an async handler, on the runtime's thread. While it ran, the
//! server answered nobody else — and its dispatch deadline could not
//! even fire: a deadline is observed when the future is polled again,
//! and the poll was stuck inside the search. The search of a real
//! repository takes hundreds of milliseconds, so the tool that answers
//! "where is this symbol used" was the one that stalled the server.
//!
//! ## What is measured
//!
//! - While a search is in flight, another call is served.
//! - A search that ran out of budget answers `partial`, with the reason
//!   and the count of the files it left out.

use cognicode_core::application::analysis_basis::Budget;
use cognicode_core::interface::mcp::handlers::{
    HandlerContext, handle_find_usages, handle_get_outline,
};
use cognicode_core::interface::mcp::schemas::{FindUsagesInput, OutlineInput};
use std::path::PathBuf;
use std::time::Duration;

/// How long a test waits for an answer before it gives up on it.
///
/// The rule is the hard one: a test that waits without a deadline is a
/// test that can never finish, and a test that never finishes holds
/// the whole suite.
const DEADLINE: Duration = Duration::from_secs(30);

/// A workspace with `count` files that do not name the symbol, plus one
/// that does.
///
/// The files that do not name it are what make the search take time:
/// the search reads every file of the tree, and a tree of three files
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

/// The search of a symbol, over a workspace of `count` files.
fn search_input(symbol: &str) -> FindUsagesInput {
    FindUsagesInput {
        symbol_name: symbol.to_string(),
        include_declaration: true,
        context_lines: None,
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
async fn a_search_in_flight_does_not_stop_the_server_from_answering() {
    let (_tmp, root) = workspace(1_200);
    let ctx = HandlerContext::builder().with_working_dir(root).build();

    let search_ctx = ctx.clone();
    let search =
        tokio::spawn(async move { handle_find_usages(&search_ctx, search_input("beta")).await });
    let other_ctx = ctx.clone();
    let other = tokio::spawn(async move {
        handle_get_outline(
            &other_ctx,
            OutlineInput {
                file_path: "src/names.rs".to_string(),
                include_private: false,
                include_tests: false,
            },
        )
        .await
    });

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
    assert_eq!(answer.total, 1, "the search found the declaration");
}

/// A search that ran out of budget answers `partial`, with the reason
/// and the count of the files it left out.
///
/// The budget here is one file: the search stops before the tree ends,
/// and the answer says so instead of reading as an answer about the
/// whole tree.
#[tokio::test]
async fn a_search_that_ran_out_of_budget_answers_partial_with_its_reason() {
    let (_tmp, root) = workspace(3);
    let ctx = HandlerContext::builder()
        .with_working_dir(root)
        .with_usage_search_budget(Budget {
            max_elapsed: None,
            max_entities: Some(1),
        })
        .build();

    let answer = tokio::time::timeout(DEADLINE, handle_find_usages(&ctx, search_input("beta")))
        .await
        .expect("the search answers within its deadline")
        .expect("the search answers");

    assert_eq!(
        answer.coverage.status, "partial",
        "an answer about a part of the tree does not say it covered the tree"
    );
    assert_eq!(answer.omitted, Some(3), "three files were not read");
    assert!(
        answer
            .truncation_reason
            .as_deref()
            .unwrap_or_default()
            .contains("budget"),
        "the answer says why it stopped: {:?}",
        answer.truncation_reason
    );
}
