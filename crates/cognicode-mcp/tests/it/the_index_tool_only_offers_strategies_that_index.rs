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
//!
//! ## The vocabulary has one owner, and the shape names it
//!
//! The factory owns the vocabulary: `GraphStrategyFactory` is what decides
//! which names index a project. The shape the tool publishes used to be a
//! `string` with a sentence attached — the names were there, but only as
//! prose, and the sentence named a strategy (`per_file`) that the factory
//! refuses. A client that reads a machine-readable shape could not tell
//! which names were real.
//!
//! So the published shape names the values, and a check ties that list to
//! the factory's own: the two are separate lists, and when they separate
//! the check says so instead of letting the published shape rot.
//!
//! The arguments in this file travel as JSON, the way they arrive: the
//! server deserializes them before the handler ever sees them, and that is
//! where a strategy that does not exist is refused.

use cognicode_core::infrastructure::graph::GraphStrategyFactory;
use cognicode_core::interface::mcp::handlers::{HandlerContext, handle_build_lightweight_index};
use cognicode_core::interface::mcp::schemas::{BuildIndexInput, BuildIndexOutput};
use cognicode_mcp::input_contracts::input_schema_for;
use serde_json::json;

/// A project that declares exactly one symbol.
fn project_with_a_symbol() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("lib.rs"), "pub fn declared_symbol() {}\n").expect("write file");
    dir
}

/// The arguments the server reads, built the way the server reads them.
///
/// `strategy` is absent when the caller omits it, so the published default
/// is the one under test rather than a value the test chose.
fn arguments(dir: &std::path::Path, strategy: Option<&str>) -> Result<BuildIndexInput, String> {
    let mut arguments = json!({ "directory": dir.to_string_lossy() });
    if let Some(strategy) = strategy {
        arguments["strategy"] = json!(strategy);
    }
    serde_json::from_value(arguments).map_err(|error| error.to_string())
}

/// The tool asked to index the project with the given strategy.
async fn index_with(
    dir: &std::path::Path,
    strategy: Option<&str>,
) -> Result<BuildIndexOutput, String> {
    let ctx = HandlerContext::builder().with_working_dir(dir).build();

    let input = arguments(dir, strategy)?;

    handle_build_lightweight_index(&ctx, input)
        .await
        .map_err(|error| format!("{error:?}"))
}

/// The strategies the published shape names.
fn published_strategies() -> Vec<String> {
    let published = input_schema_for("build_lightweight_index")
        .expect("the index tool publishes the shape of its arguments");

    published["properties"]["strategy"]["enum"]
        .as_array()
        .expect("the published shape names the strategies it accepts")
        .iter()
        .map(|name| {
            name.as_str()
                .expect("a published strategy name is a string")
                .to_string()
        })
        .collect()
}

/// The strategies the factory offers.
fn offered_strategies() -> Vec<String> {
    GraphStrategyFactory::available_strategies()
        .iter()
        .map(|name| name.to_string())
        .collect()
}

#[tokio::test]
async fn the_index_tool_reports_the_symbols_it_indexed() {
    let dir = project_with_a_symbol();

    let answer = index_with(dir.path(), Some("lightweight"))
        .await
        .expect("the index builds");

    assert!(answer.success, "the index tool did not succeed: {answer:?}");
    assert!(
        answer.symbols_indexed > 0,
        "the index tool reported no symbols for a project that declares one: {answer:?}"
    );
}

/// The published shape names the strategies the factory offers.
///
/// The two are separate lists — one is the published contract, the other is
/// the factory's own — and the check is what keeps them from separating.
#[test]
fn the_published_strategies_are_the_ones_the_factory_offers() {
    let published = published_strategies();
    let offered = offered_strategies();

    let mut published_sorted = published.clone();
    published_sorted.sort();
    let mut offered_sorted = offered.clone();
    offered_sorted.sort();

    assert_eq!(
        published_sorted, offered_sorted,
        "the published strategies and the strategies the factory offers have separated: \
         the shape publishes {published:?} and the factory offers {offered:?}"
    );
}

/// The published shape names the strategies, not a sentence about them.
///
/// A shape that names its values in prose is a shape a machine cannot read:
/// the values were in the sentence, and a sentence is not a contract.
#[test]
fn the_published_shape_names_the_strategies_it_accepts() {
    let published = input_schema_for("build_lightweight_index")
        .expect("the index tool publishes the shape of its arguments");

    let strategy = &published["properties"]["strategy"];
    assert_eq!(
        strategy["type"],
        json!("string"),
        "the strategy is a name, and the shape says so: {strategy:?}"
    );
    assert!(
        strategy["enum"].is_array(),
        "the published shape names the strategies it accepts: {strategy:?}"
    );
}

#[tokio::test]
async fn a_strategy_that_indexes_nothing_is_refused() {
    let dir = project_with_a_symbol();

    let answer = index_with(dir.path(), Some("per_file")).await;

    assert!(
        answer.is_err(),
        "the index tool answered a strategy that indexes nothing as a success: {answer:?}"
    );
}

#[tokio::test]
async fn the_refusal_names_the_strategies_that_do_index() {
    let dir = project_with_a_symbol();

    let refusal = index_with(dir.path(), Some("per_file"))
        .await
        .expect_err("the strategy that indexes nothing is refused");

    for name in ["lightweight", "on_demand", "full"] {
        assert!(
            refusal.contains(name),
            "the refusal does not name the strategies that do index: {refusal}"
        );
    }
}

/// The empty name is refused, like any other name the shape does not publish.
///
/// The empty string used to mean "lightweight" inside the handler — a
/// second, unpublished spelling of the default. The published default is
/// `lightweight`, so a caller that wants the default omits the argument.
#[tokio::test]
async fn the_empty_strategy_is_refused() {
    let dir = project_with_a_symbol();

    let answer = index_with(dir.path(), Some("")).await;

    assert!(
        answer.is_err(),
        "the empty strategy was answered as an index: {answer:?}"
    );
}

/// An omitted strategy indexes with the published default.
#[tokio::test]
async fn an_omitted_strategy_indexes_with_the_published_default() {
    let dir = project_with_a_symbol();

    let answer = index_with(dir.path(), None)
        .await
        .expect("the index builds with the published default");

    assert!(answer.success, "the index tool did not succeed: {answer:?}");
    assert!(
        answer.symbols_indexed > 0,
        "the index tool reported no symbols for a project that declares one: {answer:?}"
    );
}
