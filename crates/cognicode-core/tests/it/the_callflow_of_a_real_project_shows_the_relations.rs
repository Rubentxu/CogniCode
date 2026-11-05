//! The call flow of a real project shows the relations between its modules.
//!
//! ## The guarantee
//!
//! `export_callflow` publishes a community-level Mermaid call-flow
//! diagram. It published something else: ONE box with the number of
//! symbols in it, and a `community_count` that was the constant 1 (or 0
//! for an empty graph). `max_sections` was read and thrown away, and
//! `format` was never read at all. The tool's own published description —
//! "Shows module-level relationships" — was false.
//!
//! ## The witness
//!
//! Real code, the domain of the core crate. A graph of a few symbols has
//! one community, and a diagram of one community has no relations to show,
//! so a fixture could not tell the two apart. The witness asks the two
//! questions that matter:
//!
//! 1. is the published count the count the product's own detection
//!    measures?
//! 2. does the diagram draw the relations between the communities?
//!
//! ## What it does not assert
//!
//! It does not pin how many communities the detection finds: that number
//! moves with the repository's own churn, and a test that pinned it would
//! break on every commit that touches the code it reads. What it asserts
//! is that the published number is the measured one, and that the diagram
//! is not one box.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::graph::analytics::community_detector::CommunityDetector;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::graph_query_handlers::{
    ExportCallflowInput, handle_export_callflow,
};
use std::path::PathBuf;
use std::sync::Arc;

/// The real code the witness reads: the domain of the core crate.
fn real_code() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/domain")
}

/// The real project: its graph, and the context the tool answers from.
///
/// The context holds the same service, so the tool answers from the graph
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

/// The call flow of a real project tells the truth about its communities.
#[tokio::test]
async fn the_callflow_of_a_real_project_shows_the_relations_between_its_modules() {
    let (graph, ctx) = real_project();

    // The detection the product already uses, measured over the same
    // graph the tool answers from.
    let detection = CommunityDetector::detect(&graph, CommunityDetector::MAX_ITERATIONS);
    assert!(
        detection.communities.len() > 1,
        "the witness reads {} symbols and the detection finds {} communities: \
         a graph with one community has no relations to show",
        graph.symbol_count(),
        detection.communities.len()
    );

    let answer = handle_export_callflow(&ctx, ExportCallflowInput { max_sections: None })
        .await
        .expect("the tool answers");

    assert_eq!(
        answer.community_count,
        detection.communities.len(),
        "the published count is the measured one, not a constant"
    );

    let boxes = answer.mermaid.matches('[').count();
    assert!(
        boxes > 1,
        "the diagram draws a section per community, not one box: {}",
        answer.mermaid
    );

    assert!(
        answer.mermaid.contains("-->"),
        "the diagram draws the relations between the sections: {}",
        answer.mermaid
    );
}

/// The diagram draws as many sections as it is asked for, and says how
/// many communities there were all the same.
#[tokio::test]
async fn the_callflow_draws_the_sections_it_is_asked_for_and_says_how_many_there_were() {
    let (graph, ctx) = real_project();
    let detection = CommunityDetector::detect(&graph, CommunityDetector::MAX_ITERATIONS);

    let answer = handle_export_callflow(
        &ctx,
        ExportCallflowInput {
            max_sections: Some(1),
        },
    )
    .await
    .expect("the tool answers");

    assert_eq!(
        answer.community_count,
        detection.communities.len(),
        "the count says how many communities there were, not how many were drawn"
    );
    assert_eq!(answer.sections_drawn, 1, "one section was asked for");
    assert_eq!(
        answer.mermaid.matches('[').count(),
        1,
        "the diagram draws one box: {}",
        answer.mermaid
    );
    assert!(
        answer
            .summary
            .contains(&format!("{} communities", answer.community_count)),
        "the summary says how many communities there were: {}",
        answer.summary
    );
    assert!(
        answer
            .summary
            .contains(&format!("({} drawn)", answer.sections_drawn)),
        "the summary says how many were drawn: {}",
        answer.summary
    );
}

/// A client that still sends the retired `format` argument keeps working.
///
/// The argument was published and never read: the tool has one format. It
/// is out of the published shape now, and a client that still sends it is
/// not broken — the server ignores what it does not know.
#[tokio::test]
async fn a_client_that_still_sends_the_retired_format_argument_still_gets_an_answer() {
    let (_graph, ctx) = real_project();

    let legacy: ExportCallflowInput = serde_json::from_value(serde_json::json!({
        "format": "mermaid",
        "max_sections": 2,
    }))
    .expect("the input a client that follows the retired schema sends");

    let answer = handle_export_callflow(&ctx, legacy)
        .await
        .expect("the tool answers the client that still sends the retired argument");

    assert_eq!(
        answer.sections_drawn, 2,
        "the answer is the one it always was"
    );
}
