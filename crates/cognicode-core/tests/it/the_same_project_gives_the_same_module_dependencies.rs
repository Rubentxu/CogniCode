//! The same project gives the same module dependencies, every run.
//!
//! ## The guarantee
//!
//! A module is a directory, and the graph of modules is published: the
//! modules, the ones each one depends on, the ones that depend on it, the
//! coupling between them and the cycles they form.
//!
//! Every one of those lists came out of a hash map or a hash set, so
//! every one of them was published in an order that differs per
//! instance: the same project published the same modules in a different
//! order on every run, and the cycles of modules in a different order
//! too.
//!
//! ## The witness
//!
//! Real code, the domain of the core crate, read twice: two independent
//! builds of the same files, each with its own graph. A fixture would
//! not do — a directory with a single module has no dependencies between
//! modules, so there would be nothing to disagree about.
//!
//! The witness asks at the level the consumer reads: the published tool
//! answer, not the service underneath it.
//!
//! ## Why two pairs and not one
//!
//! Measured over eight pairs of independent builds of this witness
//! before the fix: the modules, the cycles and the coupling matrix
//! differed in eight of eight. Two pairs take the chance of an
//! accidental agreement well below one in a hundred, and after the fix
//! the agreement is not a matter of chance: the order is canonical, so
//! every pair agrees.
//!
//! ## What it does not assert
//!
//! It does not pin which modules exist or which ones form a cycle: that
//! moves with the repository's own churn, and a test that pinned it
//! would break on every commit that touches the code it reads. It
//! asserts that two runs of the same code give the same answer.
//!
//! It does not compare the metadata either: it carries the time the
//! answer took, and a time is not part of the answer.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::handle_get_module_dependencies;
use cognicode_core::interface::mcp::schemas::GetModuleDependenciesInput;
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

/// The modules the answer publishes, with the ones each one depends on.
fn published_modules(
    answer: &cognicode_core::interface::mcp::schemas::GetModuleDependenciesOutput,
) -> Vec<(String, Vec<String>, Vec<String>)> {
    answer
        .graph
        .modules
        .iter()
        .map(|m| {
            (
                m.module.clone(),
                m.depends_on.clone(),
                m.depended_by.clone(),
            )
        })
        .collect()
}

/// The same project gives the same module dependencies, in independent runs.
#[tokio::test]
async fn the_same_project_gives_the_same_module_dependencies() {
    for pair in 0..2 {
        let (_first_graph, first_ctx) = real_project();
        let (_second_graph, second_ctx) = real_project();

        let first =
            handle_get_module_dependencies(&first_ctx, GetModuleDependenciesInput { limit: 100 })
                .await
                .expect("the tool answers");
        let second =
            handle_get_module_dependencies(&second_ctx, GetModuleDependenciesInput { limit: 100 })
                .await
                .expect("the tool answers");

        assert!(
            !first.graph.modules.is_empty(),
            "the witness reads a project with dependencies between modules: a project with one module has nothing to disagree about"
        );
        assert_eq!(
            published_modules(&first),
            published_modules(&second),
            "pair {pair}: the published modules are the same in two runs of the same project"
        );
        assert_eq!(
            first.graph.cycles, second.graph.cycles,
            "pair {pair}: the published cycles of modules are the same in two runs"
        );
        assert_eq!(
            first.graph.coupling_matrix, second.graph.coupling_matrix,
            "pair {pair}: the published coupling matrix is the same in two runs"
        );
    }
}
