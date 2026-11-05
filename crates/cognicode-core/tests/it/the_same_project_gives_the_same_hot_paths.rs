//! The same project gives the same hot paths, every run.
//!
//! ## The guarantee
//!
//! The hot paths are the ten symbols with the most callers. The list was
//! ordered by that number, descending, and the order was **stable**: a
//! stable sort keeps the order of the elements it receives for the
//! elements that tie. The elements came from the iteration of a hash map,
//! so the ties were resolved in the order of a hash map — an order that
//! differs per instance.
//!
//! Two runs of the same project published the same ten symbols in a
//! different order:
//!
//! ```text
//! ["iter", "as_ref", "default", "match_snapshots", "default", "detector", "new", "caps", "symbol_id", "new"]
//! ["iter", "as_ref", "default", "match_snapshots", "default", "detector", "caps", "new", "symbol_id", "new"]
//! ```
//!
//! And worse: the cut to ten fell **inside** a group of symbols with the
//! same number of callers, so the ten published symbols were not even the
//! same ten. A list that changes between runs is not an answer a
//! consumer can read twice and get the same thing.
//!
//! ## The witness
//!
//! Real code, the services of the core domain, read again and again: each
//! construction is independent, and each one has its own graph, with its
//! own iteration order.
//!
//! A fixture would not do — a graph of a few symbols has no hot paths, so
//! there would be nothing to disagree about.
//!
//! The witness asks at the level the consumer reads: the published tool
//! answer, not the service underneath it.
//!
//! ## Why four pairs and not one
//!
//! One pair is a coin toss. The published list holds a group of symbols
//! with the same number of callers, and two constructions agree only when
//! the two orders of that group coincide — and, before the fix, the
//! published list also differed in *which* ten symbols it held, because
//! the cut fell inside the group.
//!
//! Measured over eight pairs of this witness before the fix: the two
//! lists differed in eight of eight. Four pairs take the chance of an
//! accidental agreement below one in ten thousand, and after the fix the
//! agreement is not a matter of chance: the order is canonical, so every
//! pair agrees.
//!
//! ## What it does not assert
//!
//! It does not pin which ten symbols are the hot ones: that list moves
//! with the repository's own churn, and a test that pinned it would break
//! on every commit that touches the code it reads. It asserts that two
//! runs of the same code give the same answer.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::consolidated_handlers::{
    ProjectOverviewInput, handle_project_overview,
};
use std::path::PathBuf;
use std::sync::Arc;

/// The real code the witness reads: the services of the core domain.
fn real_code() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/domain/services")
}

/// The real project: its graph, and the context the tools answer from.
///
/// The context holds the same service, so the tools answer from the graph
/// the witness measured.
fn real_project() -> (Arc<CallGraph>, HandlerContext) {
    let service = AnalysisService::new();
    service
        .build_project_graph(&real_code())
        .expect("the product graph is built over real code");
    let graph = service.get_project_graph();
    let ctx = HandlerContext::builder()
        .with_analysis_service(service)
        .with_working_dir(real_code())
        .build();
    (graph, ctx)
}

/// The hot paths, as the product publishes them.
async fn published_hot_paths(ctx: &HandlerContext) -> Vec<String> {
    handle_project_overview(
        ctx,
        ProjectOverviewInput {
            detail: "medium".into(),
        },
    )
    .await
    .expect("the tool answers")
    .hot_paths
}

/// The same project gives the same hot paths, in independent runs.
#[tokio::test]
async fn the_same_project_gives_the_same_hot_paths() {
    for pair in 0..4 {
        let (_first_graph, first_ctx) = real_project();
        let (_second_graph, second_ctx) = real_project();

        let first = published_hot_paths(&first_ctx).await;
        let second = published_hot_paths(&second_ctx).await;

        assert!(
            !first.is_empty(),
            "the witness reads a project with hot paths: a graph without callers has nothing to disagree about"
        );
        assert_eq!(
            first, second,
            "pair {pair}: the published hot paths are the same in two runs of the same project"
        );
    }
}
