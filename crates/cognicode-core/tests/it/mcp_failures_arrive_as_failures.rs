//! A failure of a tool arrives as a failure.
//!
//! The graph analytics handlers answered a missing graph, a store that
//! could not be read and a symbol that does not exist with a successful
//! result carrying an `error` key inside. The transport cannot tell that
//! apart from a result that carries data: the caller read "it worked"
//! where the tool had failed, and the metrics counted those calls as
//! successes.
//!
//! What this file pins: a handler that cannot answer says so with an
//! error, never with a payload that hides the failure inside.

use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::traits::GraphStore;
use cognicode_core::domain::value_objects::declaration::Declaration;
use cognicode_core::domain::value_objects::location::Location;
use cognicode_core::domain::value_objects::symbol_kind::SymbolKind;
use cognicode_core::infrastructure::persistence::InMemoryGraphStore;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::consolidated_handlers::{
    GraphAnalyzeInput, handle_graph_analyze,
};
use cognicode_core::interface::mcp::handlers::graph_handlers::{
    handle_graph_all_paths, handle_graph_communities, handle_graph_community_detail,
    handle_graph_condensed, handle_graph_feedback_arcs, handle_graph_god_nodes,
    handle_graph_insights, handle_graph_pagerank, handle_graph_reduced, handle_graph_search_idf,
    handle_graph_suggest_questions, handle_graph_surprising_connections,
};
use cognicode_core::interface::mcp::schemas::{
    GraphAllPathsInput, GraphCommunitiesInput, GraphCommunityDetailInput, GraphCondensedInput,
    GraphFeedbackArcsInput, GraphGodNodesInput, GraphInsightsInput, GraphPageRankInput,
    GraphReducedInput, GraphSearchIdfInput, GraphSuggestQuestionsInput,
    GraphSurprisingConnectionsInput,
};

/// Asserts that a handler answered with an error and that the error says
/// something.
///
/// A result that carries an `error` key is not an answer: the transport
/// cannot tell it apart from data, and the caller would read it as a
/// success.
macro_rules! answers_with_an_error {
    ($name:literal, $answer:expr) => {
        match $answer {
            Ok(payload) => panic!(
                "{} answers a failure with a successful result: {:?}",
                $name, payload
            ),
            Err(error) => assert!(
                !error.to_string().trim().is_empty(),
                "{} answers a failure with an empty message",
                $name
            ),
        }
    };
}

/// A context whose store holds no graph: the state a caller is in
/// before `build_graph` has ever run.
fn context_without_graph(dir: &std::path::Path) -> HandlerContext {
    HandlerContext::builder().with_working_dir(dir).build()
}

/// A context whose store holds a graph with one symbol in it.
fn context_with_graph(dir: &std::path::Path) -> HandlerContext {
    let mut graph = CallGraph::new();
    graph.add_symbol({
        let mut symbol = Symbol::new(
            "resource.demo".to_string(),
            SymbolKind::Module,
            Location::new("/repo/main.tf", 1, 0),
        )
        .with_declaration(Declaration::with_type("resource", "demo"));
        symbol.set_fqn_override("tf:/repo/main.tf:resource.demo");
        symbol
    });
    let store = InMemoryGraphStore::new();
    store.save_graph(&graph).expect("the graph is stored");
    HandlerContext::builder()
        .with_working_dir(dir)
        .with_graph_store(store)
        .build()
}

#[tokio::test]
async fn every_graph_handler_answers_with_an_error_when_there_is_no_graph() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = context_without_graph(dir.path());

    answers_with_an_error!(
        "graph_pagerank",
        handle_graph_pagerank(
            &ctx,
            GraphPageRankInput {
                alpha: 0.85,
                max_iterations: 100
            }
        )
        .await
    );
    answers_with_an_error!(
        "graph_all_paths",
        handle_graph_all_paths(
            &ctx,
            GraphAllPathsInput {
                from_symbol: "a".to_string(),
                to_symbol: "b".to_string(),
                max_hops: 3
            }
        )
        .await
    );
    answers_with_an_error!(
        "graph_condensed",
        handle_graph_condensed(&ctx, GraphCondensedInput {}).await
    );
    answers_with_an_error!(
        "graph_god_nodes",
        handle_graph_god_nodes(&ctx, GraphGodNodesInput { percentile: 0.95 }).await
    );
    answers_with_an_error!(
        "graph_reduced",
        handle_graph_reduced(&ctx, GraphReducedInput {}).await
    );
    answers_with_an_error!(
        "graph_feedback_arcs",
        handle_graph_feedback_arcs(&ctx, GraphFeedbackArcsInput {}).await
    );
    answers_with_an_error!(
        "graph_communities",
        handle_graph_communities(
            &ctx,
            GraphCommunitiesInput {
                max_iterations: 100
            }
        )
        .await
    );
    answers_with_an_error!(
        "graph_community_detail",
        handle_graph_community_detail(
            &ctx,
            GraphCommunityDetailInput {
                community_id: 0,
                max_iterations: 100
            }
        )
        .await
    );
    answers_with_an_error!(
        "graph_surprising_connections",
        handle_graph_surprising_connections(
            &ctx,
            GraphSurprisingConnectionsInput {
                top_n: 20,
                max_iterations: 100
            }
        )
        .await
    );
    answers_with_an_error!(
        "graph_search_idf",
        handle_graph_search_idf(
            &ctx,
            GraphSearchIdfInput {
                query: "demo".to_string(),
                max_results: 20
            }
        )
        .await
    );
    answers_with_an_error!(
        "graph_insights",
        handle_graph_insights(&ctx, GraphInsightsInput {}).await
    );
    answers_with_an_error!(
        "graph_suggest_questions",
        handle_graph_suggest_questions(&ctx, GraphSuggestQuestionsInput {}).await
    );
    answers_with_an_error!(
        "graph_analyze",
        handle_graph_analyze(
            &ctx,
            GraphAnalyzeInput {
                mode: "scc".to_string()
            }
        )
        .await
    );
}

#[tokio::test]
async fn a_symbol_that_does_not_exist_answers_with_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = context_with_graph(dir.path());

    answers_with_an_error!(
        "graph_all_paths",
        handle_graph_all_paths(
            &ctx,
            GraphAllPathsInput {
                from_symbol: "not_in_the_graph".to_string(),
                to_symbol: "also_not_in_the_graph".to_string(),
                max_hops: 3
            }
        )
        .await
    );
}

#[tokio::test]
async fn a_community_that_does_not_exist_answers_with_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = context_with_graph(dir.path());

    answers_with_an_error!(
        "graph_community_detail",
        handle_graph_community_detail(
            &ctx,
            GraphCommunityDetailInput {
                community_id: 999,
                max_iterations: 100
            }
        )
        .await
    );
}

#[tokio::test]
async fn a_mode_that_does_not_exist_answers_with_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = context_with_graph(dir.path());

    answers_with_an_error!(
        "graph_analyze",
        handle_graph_analyze(
            &ctx,
            GraphAnalyzeInput {
                mode: "not_a_mode".to_string()
            }
        )
        .await
    );
}
