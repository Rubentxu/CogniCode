//! The same project gives the same answer on the surfaces that walk its
//! symbols, every run.
//!
//! ## The guarantee
//!
//! The graph holds its symbols in a map, and a map iterates in an order
//! that differs per instance. Several surfaces walk that map and publish
//! what they find: the project map names five of the symbols it holds, the
//! review of a change names the symbols it affects, and the AIX tool that
//! looks for long parameter lists names the functions it found.
//!
//! Every one of those answers came out in a different order on every run:
//! the same project, published differently each time.
//!
//! ## The witness
//!
//! Real code, the domain of the core crate, read twice: two independent
//! builds of the same files, each with its own graph. A fixture would not
//! do — a directory with a handful of symbols would not fill the answers
//! the surfaces publish.
//!
//! The witness asks at the level the consumer reads: the published tool
//! answers, not the service underneath them.
//!
//! ## Why two pairs and not one
//!
//! Measured over eight pairs of independent builds of this witness before
//! the fix: the project map, the review and the parameter lists each
//! differed in eight of eight. Two pairs take the chance of an accidental
//! agreement well below one in a hundred, and after the fix the agreement
//! is not a matter of chance: the order is canonical, so every pair
//! agrees.
//!
//! ## What it does not assert
//!
//! It does not pin which symbols exist or which ones are affected: that
//! moves with the repository's own churn, and a test that pinned it would
//! break on every commit that touches the code it reads. It asserts that
//! two runs of the same code give the same answer.
//!
//! It does not compare the metadata either: it carries the time the
//! answer took, and a time is not part of the answer.

use cognicode_core::application::dto::LongParamsResult;
use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::aix_handlers::handle_detect_long_parameter_lists;
use cognicode_core::interface::mcp::handlers::consolidated_handlers::{
    CodebaseMapInput, MapFormat, ReviewPrInput, handle_codebase_map, handle_review_pr,
};
use cognicode_core::interface::mcp::schemas::DetectLongParamsInput;
use std::path::PathBuf;
use std::sync::Arc;

/// The real code the witness reads: the domain of the core crate.
fn real_code() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/domain")
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

/// The symbols the parameter-lists answer publishes.
fn published_parameters(answer: &LongParamsResult) -> Vec<(String, String, u32)> {
    answer
        .functions
        .iter()
        .map(|f| (f.symbol.clone(), f.file.clone(), f.line))
        .collect()
}

/// The same project gives the same answer on the surfaces that walk its
/// symbols, in independent runs.
#[tokio::test]
async fn the_same_project_gives_the_same_symbol_surfaces() {
    for pair in 0..2 {
        let (_first_graph, first_ctx) = real_project();
        let (_second_graph, second_ctx) = real_project();

        let first_map = handle_codebase_map(
            &first_ctx,
            CodebaseMapInput {
                format: MapFormat::Compact,
            },
        )
        .await
        .expect("the project map answers");
        let second_map = handle_codebase_map(
            &second_ctx,
            CodebaseMapInput {
                format: MapFormat::Compact,
            },
        )
        .await
        .expect("the project map answers");
        assert_eq!(
            first_map.map, second_map.map,
            "pair {pair}: the published project map is the same in two runs of the same project"
        );

        let first_review = handle_review_pr(
            &first_ctx,
            ReviewPrInput {
                files: vec!["services".to_string()],
            },
        )
        .await
        .expect("the review answers");
        let second_review = handle_review_pr(
            &second_ctx,
            ReviewPrInput {
                files: vec!["services".to_string()],
            },
        )
        .await
        .expect("the review answers");
        assert_eq!(
            first_review.impacted_files, second_review.impacted_files,
            "pair {pair}: the published affected symbols are the same in two runs"
        );

        let first_params =
            handle_detect_long_parameter_lists(&first_ctx, DetectLongParamsInput { max_params: 5 })
                .await
                .expect("the parameter lists answer");
        let second_params = handle_detect_long_parameter_lists(
            &second_ctx,
            DetectLongParamsInput { max_params: 5 },
        )
        .await
        .expect("the parameter lists answer");
        assert!(
            !first_params.functions.is_empty(),
            "the witness reads a project with functions the surface publishes: a project with none has nothing to disagree about"
        );
        assert_eq!(
            published_parameters(&first_params),
            published_parameters(&second_params),
            "pair {pair}: the published parameter lists are the same in two runs"
        );
    }
}
