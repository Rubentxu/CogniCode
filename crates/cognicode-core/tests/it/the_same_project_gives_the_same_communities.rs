//! The same project gives the same communities, every run.
//!
//! ## The guarantee
//!
//! The community detection publishes a partition of the project: which
//! symbols belong to which community. That partition is published, so it
//! has to be the same answer every time the same project is analysed.
//!
//! It was not. The projection the detection works in numbered its nodes
//! in the iteration order of a hash map — a different order for every
//! instance of the map — and Label Propagation breaks its ties by node
//! id, so the same code came out partitioned differently on every run
//! (measured: 4.960 vs 4.935 communities for the same project, inside one
//! process).
//!
//! ## The witness
//!
//! Real code, the domain of the core crate, read twice: two independent
//! builds of the same files, each with its own graph. A fixture would not
//! do — a graph of a few symbols has one community, and one community has
//! no partition to disagree about.
//!
//! The witness asks at the two levels there are: the detection itself, and
//! the answer the product publishes (`graph_communities`). A guarantee that
//! only holds one level down is not the guarantee the consumer has.
//!
//! ## What it does not assert
//!
//! It does not pin how many communities there are: that number moves with
//! the repository's own churn, and a test that pinned it would break on
//! every commit that touches the code it reads. It asserts that two runs
//! of the same code give the same partition — the same count, the same
//! communities, and the same members in each of them.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::graph::analytics::community_detector::{
    CommunityDetector, CommunityResult,
};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_communities;
use cognicode_core::interface::mcp::schemas::GraphCommunitiesInput;
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

/// The detection of a graph, as the product publishes it.
fn detection(graph: &CallGraph) -> CommunityResult {
    CommunityDetector::detect(graph, CommunityDetector::MAX_ITERATIONS)
}

/// The communities of a detection, with their members in the order the
/// product publishes them.
fn published(result: &CommunityResult) -> Vec<(u32, Vec<String>)> {
    result
        .communities
        .iter()
        .map(|community| {
            (
                community.id,
                community
                    .nodes
                    .iter()
                    .map(|symbol| symbol.as_str().to_string())
                    .collect(),
            )
        })
        .collect()
}

/// The same project gives the same communities, in two independent runs.
#[tokio::test]
async fn the_same_project_gives_the_same_communities() {
    let (first_graph, first_ctx) = real_project();
    let (second_graph, second_ctx) = real_project();

    let first = detection(&first_graph);
    let second = detection(&second_graph);

    assert!(
        first.communities.len() > 1,
        "the witness reads a project with {} communities: a graph with one community has no partition to disagree about",
        first.communities.len()
    );

    assert_eq!(
        first.communities.len(),
        second.communities.len(),
        "the count of communities is the same in two runs"
    );
    assert_eq!(
        first.node_communities, second.node_communities,
        "the partition is the same in two runs"
    );
    assert_eq!(
        published(&first),
        published(&second),
        "the communities and their members are the same in two runs"
    );

    // And the answer the consumer reads is the same answer: the tool
    // publishes the detection, so it inherits the guarantee only if the
    // publication does not shuffle it on the way out.
    // The tool's own default: 100 iterations, the same number the
    // detector's own default is.
    let first_answer = handle_graph_communities(
        &first_ctx,
        GraphCommunitiesInput {
            max_iterations: 100,
        },
    )
    .await
    .expect("the tool answers");
    let second_answer = handle_graph_communities(
        &second_ctx,
        GraphCommunitiesInput {
            max_iterations: 100,
        },
    )
    .await
    .expect("the tool answers");

    assert_eq!(
        first_answer, second_answer,
        "graph_communities publishes the same answer for the same project in two runs"
    );
}
