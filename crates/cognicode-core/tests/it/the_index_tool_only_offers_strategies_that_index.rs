//! The index tool must not offer a strategy that indexes nothing.
//!
//! Measured: the per-file strategy answered the index contract without
//! building an index. Its `build_index` returned success without opening
//! a single file and its symbol query answered an empty list, so the tool
//! that indexes a project reported "0 symbols indexed" — together with
//! success — about a project full of symbols. The name was on the tool's
//! own list of accepted values, so a client that asked for it had no way
//! of telling that the answer was a fiction.
//!
//! The law this file pins:
//!
//! - the index tool answers with what it indexed, and
//! - a name that does not index is refused, with the names that do.
//!
//! The refusal is the important half. A tool that answered an impossible
//! request with a plausible number teaches its client to believe numbers
//! it cannot check; a tool that refuses leaves the client knowing that
//! it asked for something that does not exist.

use cognicode_core::interface::mcp::handlers::{HandlerContext, handle_build_lightweight_index};
use cognicode_core::interface::mcp::schemas::BuildIndexInput;

/// A project that declares exactly one symbol.
fn project_with_a_symbol() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("lib.rs"), "pub fn declared_symbol() {}\n").expect("write file");
    dir
}

/// The tool asked to index the project with the given strategy.
async fn index_with(
    dir: &std::path::Path,
    strategy: &str,
) -> Result<cognicode_core::interface::mcp::schemas::BuildIndexOutput, String> {
    let ctx = HandlerContext::builder().with_working_dir(dir).build();

    handle_build_lightweight_index(
        &ctx,
        BuildIndexInput {
            directory: Some(dir.to_string_lossy().into_owned()),
            strategy: strategy.to_string(),
        },
    )
    .await
    .map_err(|error| format!("{error:?}"))
}

#[tokio::test]
async fn the_index_tool_reports_the_symbols_it_indexed() {
    let dir = project_with_a_symbol();

    let answer = index_with(dir.path(), "lightweight")
        .await
        .expect("the index builds");

    assert!(answer.success, "the index tool did not succeed: {answer:?}");
    assert!(
        answer.symbols_indexed > 0,
        "the index tool reported no symbols for a project that declares one: {answer:?}"
    );
}

#[tokio::test]
async fn a_strategy_that_indexes_nothing_is_refused() {
    let dir = project_with_a_symbol();

    let answer = index_with(dir.path(), "per_file").await;

    assert!(
        answer.is_err(),
        "the index tool answered a strategy that indexes nothing as a success: {answer:?}"
    );
}

#[tokio::test]
async fn the_refusal_names_the_strategies_that_do_index() {
    let dir = project_with_a_symbol();

    let refusal = index_with(dir.path(), "per_file")
        .await
        .expect_err("the strategy that indexes nothing is refused");

    for name in ["lightweight", "on_demand", "full"] {
        assert!(
            refusal.contains(name),
            "the refusal does not name the strategies that do index: {refusal}"
        );
    }
}
