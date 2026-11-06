//! `project_overview` publishes the level it delivers.
//!
//! ## The guarantee
//!
//! The tool's title promises three levels — `quick`, `medium` and
//! `detailed` — and the argument published those three names. The handler
//! read the argument, copied it into the answer and used it for nothing
//! else: the answer that said "quick" carried the same content as the
//! answer that said "detailed". A client that asked for less got
//! everything, labelled with the name of what it did not get.
//!
//! ## The law this file pins
//!
//! - the answer says the level it delivers;
//! - each level delivers different content; and
//! - a level that does not exist is a refusal, not a silent fallback.
//!
//! ## The witness
//!
//! A small project read through the handler the tool uses: one symbol
//! called from two places, so the project has a hot path, the
//! recommendations are not empty, and there are entry points to publish. A
//! project without them would make the assertions vacuous.
//!
//! ## Why the level travels as JSON
//!
//! Because that is the boundary the server reads the arguments at: the
//! adapter deserializes the arguments before the handler sees them, and a
//! level that does not exist is refused there. Building the arguments the
//! same way a client builds them is what makes the refusal part of this
//! file rather than a comment about it.

use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::consolidated_handlers::{
    ProjectOverviewInput, ProjectOverviewOutput, handle_project_overview,
};
use cognicode_core::interface::mcp::schemas::OverviewDetail;
use serde_json::json;

/// A project with a hot path and with entry points.
///
/// `hub` is called from two places, so it has two callers and the
/// question about the hot paths has an answer. `caller_one` and
/// `caller_two` are called by nobody, so they are the entry points.
fn project_with_a_hot_path() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("lib.rs"),
        "pub fn hub() {}\n\
         pub fn caller_one() { hub(); }\n\
         pub fn caller_two() { hub(); }\n",
    )
    .expect("write file");
    dir
}

/// The tool asked for the given arguments.
async fn overview_at(dir: &std::path::Path, arguments: serde_json::Value) -> ProjectOverviewOutput {
    let input: ProjectOverviewInput =
        serde_json::from_value(arguments).expect("the arguments name a level that exists");
    let ctx = HandlerContext::builder().with_working_dir(dir).build();
    handle_project_overview(&ctx, input)
        .await
        .expect("the tool answers")
}

/// The quick answer carries the statistics and nothing else.
#[tokio::test]
async fn the_quick_answer_carries_the_statistics_and_nothing_else() {
    let dir = project_with_a_hot_path();

    let answer = overview_at(dir.path(), json!({"detail": "quick"})).await;

    assert_eq!(
        answer.detail,
        OverviewDetail::Quick,
        "the answer says the level it delivers: {answer:?}"
    );
    assert!(
        answer.hot_paths.is_empty(),
        "the quick answer does not carry the hot paths: {answer:?}"
    );
    assert!(
        answer.entry_points.is_empty(),
        "the quick answer does not carry the entry points: {answer:?}"
    );
    assert!(
        answer.recommendations.is_empty(),
        "the quick answer does not carry the recommendations: {answer:?}"
    );
    assert!(
        answer.system_prompt_context.is_none(),
        "the quick answer does not carry the context for a prompt: {answer:?}"
    );
    // The measurements travel at every level. An absent score would say
    // "it was not measured", and it was.
    assert!(
        answer.architecture_score.is_some(),
        "the quick answer carries the measurements it has: {answer:?}"
    );
}

/// The medium answer adds the entry points and the hot paths.
#[tokio::test]
async fn the_medium_answer_adds_the_entry_points_and_the_hot_paths() {
    let dir = project_with_a_hot_path();

    let answer = overview_at(dir.path(), json!({"detail": "medium"})).await;

    assert_eq!(
        answer.detail,
        OverviewDetail::Medium,
        "the answer says the level it delivers: {answer:?}"
    );
    assert!(
        !answer.hot_paths.is_empty(),
        "the medium answer carries the hot paths: {answer:?}"
    );
    assert!(
        !answer.entry_points.is_empty(),
        "the medium answer carries the entry points: {answer:?}"
    );
    assert!(
        answer.recommendations.is_empty(),
        "the medium answer does not carry the recommendations: {answer:?}"
    );
    assert!(
        answer.system_prompt_context.is_none(),
        "the medium answer does not carry the context for a prompt: {answer:?}"
    );
}

/// The detailed answer carries everything.
#[tokio::test]
async fn the_detailed_answer_carries_everything() {
    let dir = project_with_a_hot_path();

    let answer = overview_at(dir.path(), json!({"detail": "detailed"})).await;

    assert_eq!(
        answer.detail,
        OverviewDetail::Detailed,
        "the answer says the level it delivers: {answer:?}"
    );
    assert!(
        !answer.hot_paths.is_empty(),
        "the detailed answer carries the hot paths: {answer:?}"
    );
    assert!(
        !answer.entry_points.is_empty(),
        "the detailed answer carries the entry points: {answer:?}"
    );
    assert!(
        !answer.recommendations.is_empty(),
        "the detailed answer carries the recommendations: {answer:?}"
    );
    assert!(
        answer.system_prompt_context.is_some(),
        "the detailed answer carries the context for a prompt: {answer:?}"
    );

    // The name the client reads is the name of the level.
    let wire = serde_json::to_value(&answer).expect("the answer is published as JSON");
    assert_eq!(
        wire["detail"], "detailed",
        "the answer names its level on the wire: {wire}"
    );
}

/// The answer a client gets without asking for a level is the complete one.
///
/// The tool published a complete answer before the levels meant anything,
/// so that is what the default has to keep delivering: the label changes,
/// the content does not.
#[tokio::test]
async fn the_default_answer_is_the_complete_one() {
    let dir = project_with_a_hot_path();

    let answer = overview_at(dir.path(), json!({})).await;

    assert_eq!(
        answer.detail,
        OverviewDetail::Detailed,
        "the default delivers the complete answer and says so: {answer:?}"
    );
    assert!(
        !answer.recommendations.is_empty(),
        "the default answer carries the recommendations: {answer:?}"
    );
    assert!(
        answer.system_prompt_context.is_some(),
        "the default answer carries the context for a prompt: {answer:?}"
    );
}

/// A level that does not exist is refused.
#[test]
fn a_level_that_does_not_exist_is_refused() {
    let refused = serde_json::from_value::<ProjectOverviewInput>(json!({"detail": "full"}));

    assert!(
        refused.is_err(),
        "a level the tool does not have is a refusal, not a silent fallback: {refused:?}"
    );
}
