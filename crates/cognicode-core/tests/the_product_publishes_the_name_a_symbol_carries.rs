//! The product must publish the name a thing carries, not a piece of its
//! identity.
//!
//! Measured: the published hot paths and the suggested questions took the
//! name out of the identity string by splitting it. For a symbol that
//! happens to work — the name sits in the middle of the identity — but for
//! a declaration an infrastructure reader named, the piece that survived
//! the split was the file the thing lives in. The product answered that
//! "main.tf is the most depended-upon symbol" about a bucket.
//!
//! The law this file pins: the name the product publishes is the name the
//! symbol carries.

use cognicode_core::application::services::graph_insights::GraphInsightsService;
use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::value_objects::Language;
use cognicode_core::infrastructure::graph::project_call_graph;
use cognicode_core::infrastructure::graph::semantic_layer::{call_graph_from, semantic_facts};
use cognicode_core::interface::mcp::handlers::consolidated_handlers::hot_paths;

/// The corpus: one file per grammar the product writes.
fn corpus() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates/cognicode-core/testdata/identity_corpus")
}

/// The graph the product builds out of one file of the corpus.
fn graph_of(language: Language, file: &str) -> CallGraph {
    let source = std::fs::read_to_string(corpus().join(file)).expect("the corpus file is readable");
    let facts = semantic_facts(language, &source, file)
        .unwrap_or_else(|| panic!("the reader must declare things in {file}"));
    call_graph_from(&facts)
}

/// The names the product publishes as the hot paths of a graph.
fn published_names(graph: &CallGraph) -> Vec<String> {
    let projection = project_call_graph(graph);
    let report = GraphInsightsService::analyze(graph, projection.as_ref());
    hot_paths(graph, &report)
        .into_iter()
        .map(|path| path.symbol_id)
        .collect()
}

#[test]
fn the_published_hot_paths_name_the_thing_and_not_the_file_it_lives_in() {
    let graph = graph_of(Language::Hcl, "main.tf");
    let names = published_names(&graph);

    assert!(
        !names.is_empty(),
        "non-vacuity: the corpus must produce a hot symbol, got no published name"
    );
    assert!(
        names.iter().any(|name| name == "aws_s3_bucket.app_bucket"),
        "the published hot paths must name the thing, got {names:?}"
    );
    assert!(
        !names.iter().any(|name| name == "main.tf"),
        "the published hot paths must not name the file the thing lives in, got {names:?}"
    );
}

#[test]
fn the_suggested_questions_name_the_thing_and_not_the_file_it_lives_in() {
    let graph = graph_of(Language::Hcl, "main.tf");
    let projection = project_call_graph(&graph);
    let report = GraphInsightsService::analyze(&graph, projection.as_ref());

    let question = report
        .suggested_questions
        .iter()
        .find(|question| question.contains("most depended-upon symbol"))
        .unwrap_or_else(|| {
            panic!(
                "the report must ask about the most depended-upon symbol, got {:?}",
                report.suggested_questions
            )
        });

    assert!(
        question.contains("aws_s3_bucket.app_bucket"),
        "the question must name the thing, got {question:?}"
    );
    assert!(
        !question.contains("'main.tf'"),
        "the question must not name the file the thing lives in, got {question:?}"
    );
}
