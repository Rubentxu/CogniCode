//! The same project gives the same cycles and the same god nodes, every run.
//!
//! ## The guarantee
//!
//! Two published answers came out different every run, for a project that
//! had not changed: the cycles, and the god nodes with their scores.
//!
//! The cycles are published by the domain's own Tarjan, which walks the
//! graph directly: the symbols came out of a hash map, and so did the
//! successors of each node, so the order the cycles were found in was the
//! order of a hash map — 61 of the 62 published positions differed between
//! two runs of the same project.
//!
//! The god nodes are published with their PageRank scores, and the score
//! is a sum accumulated over the incoming edges of each node: the order of
//! those edges decided the order of the additions, and a sum in floating
//! point is not associative. The scores of the same project differed in the
//! last decimal (5.551e-17), and since the list is ordered by score, the
//! order of the list differed too.
//!
//! ## The witness
//!
//! Real code, the domain of the core crate, read twice: two independent
//! builds of the same files, each with its own graph. A fixture would not
//! do — a graph of a few symbols has no cycles and no god nodes, so there
//! would be nothing to disagree about.
//!
//! The witness asks at the level the consumer reads: the published tool
//! answers, not the services underneath them.
//!
//! ## What it does not assert
//!
//! It does not pin how many cycles there are or how many god nodes: those
//! numbers move with the repository's own churn, and a test that pinned
//! them would break on every commit that touches the code it reads. It
//! asserts that two runs of the same code give the same answer.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::graph::project_call_graph;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_god_nodes;
use cognicode_core::interface::mcp::handlers::handle_check_architecture;
use cognicode_core::interface::mcp::schemas::{
    CheckArchitectureInput, CheckArchitectureOutput, GraphGodNodesInput,
};
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

/// The cycles of an answer, with their members in the published order.
fn cycles_of(answer: &CheckArchitectureOutput) -> Vec<Vec<String>> {
    answer
        .cycles
        .iter()
        .map(|cycle| cycle.symbols.clone())
        .collect()
}

/// The same project gives the same cycles and the same god nodes, in two
/// independent runs.
#[tokio::test]
async fn the_same_project_gives_the_same_cycles_and_god_nodes() {
    let (first_graph, first_ctx) = real_project();
    let (second_graph, second_ctx) = real_project();

    // The cycles, as the product publishes them.
    let first_cycles =
        handle_check_architecture(&first_ctx, CheckArchitectureInput { scope: None })
            .await
            .expect("the tool answers");
    let second_cycles =
        handle_check_architecture(&second_ctx, CheckArchitectureInput { scope: None })
            .await
            .expect("the tool answers");

    assert!(
        !first_cycles.cycles.is_empty(),
        "the witness reads a project with {} cycles: a graph without cycles has nothing to disagree about",
        first_cycles.cycles.len()
    );
    assert_eq!(
        cycles_of(&first_cycles),
        cycles_of(&second_cycles),
        "the published cycles are the same in two runs, in the same order"
    );

    // The god nodes, as the product publishes them: the same ids, in the
    // same order, with the same scores.
    let first_god = handle_graph_god_nodes(&first_ctx, GraphGodNodesInput { percentile: 0.95 })
        .await
        .expect("the tool answers");
    let second_god = handle_graph_god_nodes(&second_ctx, GraphGodNodesInput { percentile: 0.95 })
        .await
        .expect("the tool answers");

    assert_eq!(
        first_god, second_god,
        "graph_god_nodes publishes the same list in two runs: same ids, same order, same scores"
    );

    // The components of the projection, which are what the explorer's
    // `impact_detect_cycles` publishes.
    let first_components = project_call_graph(&first_graph).strongly_connected_components();
    let second_components = project_call_graph(&second_graph).strongly_connected_components();

    assert_eq!(
        first_components, second_components,
        "the projection publishes the same components in the same order"
    );
}
