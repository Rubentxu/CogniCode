//! `graph_query_filtered` runs the query it is asked and says what its
//! filters left out.
//!
//! ## The defect
//!
//! The tool loaded the graph, parsed the question, never walked anything and
//! answered `0 nodes, 0 edges` whatever it was asked, while its summary
//! recorded the provenance, kind and community filters as "applied". An empty
//! answer from a tool that did not look reads as "nothing there".
//!
//! ## What this file measures
//!
//! The real handler over a real `CallGraph`, read the way the MCP wire
//! carries it (the serialized output). The input is built as the JSON a
//! client sends, and the expected values are written here by hand from the
//! way each edge was added.

use cognicode_core::application::graph_snapshot::load_or_build_graph;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::graph::analytics::community_detector::CommunityDetector;
use cognicode_core::interface::cursor::CursorContext;
use cognicode_core::interface::hateoas::envelope_for;
use cognicode_core::interface::mcp::handlers::graph_query_handlers::{
    GraphQueryFilteredInput, GraphQueryInput, handle_graph_query, handle_graph_query_filtered,
};
use cognicode_core::interface::mcp::handlers::{HandlerContext, HandlerError};
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap};

use super::graph_query_says_what_it_traversed::{
    array, context_holding, link, readable_tree, tree_with_an_ambiguous_call,
};

// ---------------------------------------------------------------------------
// Fixtures.
// ---------------------------------------------------------------------------

fn add(
    graph: &mut CallGraph,
    ids: &mut HashMap<&'static str, String>,
    name: &'static str,
    kind: SymbolKind,
    line: u32,
) {
    let id = graph.add_symbol(Symbol::new(name, kind, Location::new("lib.rs", line, 0)));
    ids.insert(name, id.as_str().to_string());
}

/// A graph whose nodes and relations differ in the ways a filter looks at.
///
/// ```text
/// entry --calls (direct)-----> helper      function
/// entry --calls (unresolved)-> guess       function, ambiguous 0.3
/// entry --calls (direct)-----> Widget      class
/// entry --inherits (manual)--> Contract    trait
/// helper --calls (heuristic)-> deep        function, inferred 0.6
/// Widget --calls (direct)----> render      method
/// guess --calls (direct)-----> ghost       function
/// ```
///
/// 8 nodes, 7 edges.
fn filter_graph() -> CallGraph {
    let mut graph = CallGraph::new();
    let mut ids = HashMap::new();
    add(&mut graph, &mut ids, "entry", SymbolKind::Function, 1);
    add(&mut graph, &mut ids, "helper", SymbolKind::Function, 2);
    add(&mut graph, &mut ids, "guess", SymbolKind::Function, 3);
    add(&mut graph, &mut ids, "Widget", SymbolKind::Class, 4);
    add(&mut graph, &mut ids, "Contract", SymbolKind::Trait, 5);
    add(&mut graph, &mut ids, "deep", SymbolKind::Function, 6);
    add(&mut graph, &mut ids, "render", SymbolKind::Method, 7);
    add(&mut graph, &mut ids, "ghost", SymbolKind::Function, 8);

    let edges = [
        (
            "entry",
            "helper",
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        ),
        (
            "entry",
            "guess",
            DependencyType::Calls,
            ExtractionContext::Unresolved,
        ),
        (
            "entry",
            "Widget",
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        ),
        (
            "entry",
            "Contract",
            DependencyType::Inherits,
            ExtractionContext::Manual,
        ),
        (
            "helper",
            "deep",
            DependencyType::Calls,
            ExtractionContext::Heuristic { score: 0.6 },
        ),
        (
            "Widget",
            "render",
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        ),
        (
            "guess",
            "ghost",
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        ),
    ];
    for (from, to, kind, how) in edges {
        let from = cognicode_core::domain::aggregates::call_graph::SymbolId::new(&ids[from]);
        let to = cognicode_core::domain::aggregates::call_graph::SymbolId::new(&ids[to]);
        link(&mut graph, &from, &to, kind, how);
    }
    graph
}

/// Two tight triangles joined by one bridge: two communities.
fn two_communities() -> CallGraph {
    let mut graph = CallGraph::new();
    let mut ids = HashMap::new();
    let names = ["a1", "a2", "a3", "b1", "b2", "b3"];
    for (line, name) in names.into_iter().enumerate() {
        add(
            &mut graph,
            &mut ids,
            name,
            SymbolKind::Function,
            line as u32 + 1,
        );
    }
    let sid =
        |name: &str| cognicode_core::domain::aggregates::call_graph::SymbolId::new(&ids[name]);
    for (from, to) in [
        ("a1", "a2"),
        ("a2", "a3"),
        ("a3", "a1"),
        ("b1", "b2"),
        ("b2", "b3"),
        ("b3", "b1"),
        ("a3", "b1"),
    ] {
        link(
            &mut graph,
            &sid(from),
            &sid(to),
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
    }
    graph
}

async fn try_ask(ctx: &HandlerContext, request: Value) -> Result<Value, HandlerError> {
    let input: GraphQueryFilteredInput =
        serde_json::from_value(request).expect("the request is the JSON a client sends");
    let output = handle_graph_query_filtered(ctx, input).await?;
    Ok(serde_json::to_value(output).expect("the answer is JSON"))
}

async fn ask(ctx: &HandlerContext, request: Value) -> Value {
    try_ask(ctx, request)
        .await
        .expect("the filtered query answers")
}

async fn ask_unfiltered(ctx: &HandlerContext, question: &str, max_depth: usize) -> Value {
    let output = handle_graph_query(
        ctx,
        GraphQueryInput {
            question: question.to_string(),
            max_depth,
            budget: 1500,
        },
    )
    .await
    .expect("the graph query answers");
    serde_json::to_value(output).expect("the answer is JSON")
}

fn node_labels(answer: &Value) -> BTreeSet<String> {
    array(answer, "nodes")
        .iter()
        .map(|n| n["label"].as_str().expect("label").to_string())
        .collect()
}

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// The edges as `source -> target` labels.
fn edge_pairs(answer: &Value) -> BTreeSet<String> {
    let label: HashMap<String, String> = array(answer, "nodes")
        .iter()
        .map(|n| {
            (
                n["id"].as_str().expect("id").to_string(),
                n["label"].as_str().expect("label").to_string(),
            )
        })
        .collect();
    array(answer, "edges")
        .iter()
        .map(|e| {
            format!(
                "{} -> {}",
                label[e["source"].as_str().expect("source")],
                label[e["target"].as_str().expect("target")]
            )
        })
        .collect()
}

fn size(answer: &Value) -> usize {
    array(answer, "nodes").len() + array(answer, "edges").len()
}

// ---------------------------------------------------------------------------
// It is the same query as `graph_query`.
// ---------------------------------------------------------------------------

/// Without filters the answer is the one `graph_query` gives, field by
/// field, and says nothing was excluded: the tool is that traversal and not
/// a second one.
#[tokio::test]
async fn without_filters_it_answers_what_graph_query_answers() {
    let ctx = context_holding(filter_graph());
    let plain = ask_unfiltered(&ctx, "entry", 3).await;
    assert_eq!(size(&plain), 15, "the fixture is not trivial: {plain}");

    for request in [
        json!({ "question": "entry" }),
        json!({ "question": "entry", "filters": {} }),
        json!({ "question": "entry", "filters": { "exclude_kinds": [] } }),
        json!({ "question": "entry", "filters": {
            "provenance": ["extracted", "inferred", "ambiguous", "manual", "tested"],
        } }),
    ] {
        let mut answer = ask(&ctx, request.clone()).await;
        assert_eq!(answer["filtered_out"], json!({ "nodes": 0, "edges": 0 }));
        let object = answer.as_object_mut().expect("an object");
        object.remove("filtered_out");
        object.remove("applied_filters");
        assert_eq!(answer, plain, "{request}");
    }
}

/// The wire shape: the traversal fields are at the top level, as in
/// `graph_query`, next to the two fields the filters add.
#[tokio::test]
async fn the_answer_publishes_the_traversal_fields_of_graph_query() {
    let ctx = context_holding(filter_graph());
    let answer = ask(&ctx, json!({ "question": "entry" })).await;
    for field in [
        "question",
        "nodes",
        "edges",
        "explanation",
        "summary",
        "mode",
        "direction",
        "truncated",
        "truncation_reason",
        "unexpanded_nodes",
        "limits",
        "coverage",
        "basis",
        "applied_filters",
        "filtered_out",
    ] {
        assert!(
            answer.get(field).is_some(),
            "`{field}` is published: {answer}"
        );
    }
    assert_eq!(answer["mode"], "traversal");
    assert_eq!(answer["limits"]["budget"], 1500);
    assert_eq!(answer["limits"]["max_depth"], 3);
}

// ---------------------------------------------------------------------------
// The filters change what the walk follows.
// ---------------------------------------------------------------------------

/// Only the relations of an admitted provenance are followed: the nodes only
/// another provenance reaches are not reached, and the answer counts the
/// relations it did not follow.
#[tokio::test]
async fn the_provenance_filter_decides_which_relations_are_followed() {
    let ctx = context_holding(filter_graph());

    let extracted = ask(
        &ctx,
        json!({ "question": "entry", "filters": { "provenance": ["EXTRACTED"] } }),
    )
    .await;
    assert_eq!(
        node_labels(&extracted),
        names(&["entry", "helper", "Widget", "render"])
    );
    assert_eq!(
        edge_pairs(&extracted),
        BTreeSet::from([
            "entry -> helper".to_string(),
            "entry -> Widget".to_string(),
            "Widget -> render".to_string(),
        ])
    );
    for edge in array(&extracted, "edges") {
        assert_eq!(edge["provenance"], "Extracted");
    }
    // The ambiguous call, the manual inheritance and the inferred call.
    assert_eq!(extracted["filtered_out"], json!({ "nodes": 0, "edges": 3 }));
    assert_eq!(
        extracted["applied_filters"],
        json!(["provenance in [EXTRACTED]"])
    );

    let with_manual = ask(
        &ctx,
        json!({ "question": "entry", "filters": { "provenance": ["Extracted", "Manual"] } }),
    )
    .await;
    assert_eq!(
        node_labels(&with_manual),
        names(&["entry", "helper", "Widget", "render", "Contract"])
    );
    assert_eq!(
        with_manual["filtered_out"],
        json!({ "nodes": 0, "edges": 2 })
    );
}

/// The node filters decide which nodes are returned and walked through: a
/// node that is only reachable through an excluded one is not reached.
#[tokio::test]
async fn the_kind_filters_decide_which_nodes_are_reached() {
    let ctx = context_holding(filter_graph());

    let functions = ask(
        &ctx,
        json!({ "question": "entry", "filters": { "node_kinds": ["Function"] } }),
    )
    .await;
    assert_eq!(
        node_labels(&functions),
        names(&["entry", "helper", "guess", "deep", "ghost"])
    );
    assert_eq!(
        edge_pairs(&functions),
        BTreeSet::from([
            "entry -> helper".to_string(),
            "entry -> guess".to_string(),
            "helper -> deep".to_string(),
            "guess -> ghost".to_string(),
        ])
    );
    // `Widget` and `Contract` were met and excluded; `render` was never met.
    assert_eq!(functions["filtered_out"], json!({ "nodes": 2, "edges": 2 }));

    let no_classes = ask(
        &ctx,
        json!({ "question": "entry", "filters": { "exclude_kinds": ["class"] } }),
    )
    .await;
    assert_eq!(
        node_labels(&no_classes),
        names(&["entry", "helper", "guess", "Contract", "deep", "ghost"])
    );
    assert_eq!(
        no_classes["filtered_out"],
        json!({ "nodes": 1, "edges": 1 })
    );

    // An excluded kind wins over an admitted one.
    let both = ask(
        &ctx,
        json!({ "question": "entry", "filters": {
            "node_kinds": ["function", "trait"], "exclude_kinds": ["trait"],
        } }),
    )
    .await;
    assert_eq!(node_labels(&both), node_labels(&functions));
}

/// Filters compose: each relation has to pass every one of them.
#[tokio::test]
async fn the_filters_compose() {
    let ctx = context_holding(filter_graph());
    let answer = ask(
        &ctx,
        json!({ "question": "entry", "filters": {
            "provenance": ["extracted"], "node_kinds": ["function"],
        } }),
    )
    .await;
    assert_eq!(node_labels(&answer), names(&["entry", "helper"]));
    assert_eq!(
        edge_pairs(&answer),
        BTreeSet::from(["entry -> helper".to_string()])
    );
    // guess (provenance), Widget (kind), Contract (provenance), deep (provenance).
    assert_eq!(answer["filtered_out"], json!({ "nodes": 1, "edges": 4 }));
    assert_eq!(answer["applied_filters"].as_array().expect("list").len(), 2);
}

/// A seed a filter excludes is not returned, and the answer says it was
/// excluded instead of reading as "no such symbol".
#[tokio::test]
async fn an_excluded_seed_is_counted_and_the_empty_answer_says_why() {
    let ctx = context_holding(filter_graph());
    let answer = ask(
        &ctx,
        json!({ "question": "widget", "filters": { "node_kinds": ["function"] } }),
    )
    .await;
    assert!(array(&answer, "nodes").is_empty());
    assert_eq!(answer["filtered_out"], json!({ "nodes": 1, "edges": 0 }));
    let summary = answer["summary"].as_str().expect("summary");
    assert!(summary.contains("left out by the filters"), "{summary}");
    assert!(summary.contains("does not prove absence"), "{summary}");
}

/// The community filter uses the communities `graph_communities` finds, and
/// keeps the nodes of the one asked for.
#[tokio::test]
async fn the_community_filter_keeps_the_nodes_of_that_community() {
    let graph = two_communities();
    let detection = CommunityDetector::detect(&graph, CommunityDetector::MAX_ITERATIONS);
    assert_eq!(
        detection.communities.len(),
        2,
        "the fixture has two communities"
    );
    let ctx = context_holding(graph);

    let mut covered = BTreeSet::new();
    for community in &detection.communities {
        let answer = ask(
            &ctx,
            json!({
                "question": "a1 a2 a3 b1 b2 b3",
                "filters": { "community_id": community.id },
            }),
        )
        .await;
        let expected: BTreeSet<String> = community
            .nodes
            .iter()
            .map(|id| id.as_str().to_string())
            .collect();
        let returned: BTreeSet<String> = array(&answer, "nodes")
            .iter()
            .map(|n| n["id"].as_str().expect("id").to_string())
            .collect();
        assert_eq!(returned, expected, "community {}", community.id);
        assert_eq!(
            answer["filtered_out"]["nodes"],
            6 - community.nodes.len(),
            "the nodes of the other community were met and excluded"
        );
        covered.extend(returned);
    }
    assert_eq!(covered.len(), 6, "every node is in one of the communities");
}

// ---------------------------------------------------------------------------
// An invalid query is refused, not answered with nothing.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_filter_that_names_nothing_real_is_refused() {
    let ctx = context_holding(filter_graph());
    for (filters, expected) in [
        (
            json!({ "provenance": ["certain"] }),
            "unknown provenance 'certain'",
        ),
        (json!({ "node_kinds": ["gizmo"] }), "unknown kind 'gizmo'"),
        (
            json!({ "exclude_kinds": ["gizmo"] }),
            "unknown kind 'gizmo'",
        ),
        (
            json!({ "provenance": [] }),
            "the provenance filter is empty",
        ),
        (
            json!({ "node_kinds": [] }),
            "the node_kinds filter is empty",
        ),
        (json!({ "community_id": 99 }), "community 99 does not exist"),
    ] {
        let refusal = try_ask(&ctx, json!({ "question": "entry", "filters": filters }))
            .await
            .expect_err("the query is refused");
        assert!(
            matches!(&refusal, HandlerError::InvalidInput(message) if message.contains(expected)),
            "{filters}: {refusal:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Limits.
// ---------------------------------------------------------------------------

/// What a filter excludes costs no budget, and the budget still bounds the
/// answer.
#[tokio::test]
async fn the_budget_counts_what_is_returned_and_never_what_is_excluded() {
    let ctx = context_holding(filter_graph());
    let filters = json!({ "provenance": ["extracted"] });

    // 4 nodes and 3 edges: seven elements fit exactly, so nothing was cut.
    let fits = ask(
        &ctx,
        json!({ "question": "entry", "filters": filters, "limit": 7 }),
    )
    .await;
    assert_eq!(size(&fits), 7);
    assert_eq!(fits["truncated"], false);
    assert_eq!(fits["limits"]["budget"], 7);

    let full = fits;
    for limit in 0..7 {
        let cut = ask(
            &ctx,
            json!({ "question": "entry", "filters": filters, "limit": limit }),
        )
        .await;
        assert!(size(&cut) <= limit, "limit {limit}: {cut}");
        assert_eq!(cut["truncated"], true, "limit {limit}");
        assert_eq!(cut["truncation_reason"], "budget", "limit {limit}");
        for field in ["nodes", "edges"] {
            let part = array(&cut, field);
            assert_eq!(part.as_slice(), &array(&full, field)[..part.len()]);
        }
    }
}

/// The depth stops the walk, and a node whose only outgoing relations the
/// filter would not follow is not reported as left unexpanded.
#[tokio::test]
async fn the_depth_limit_reports_only_what_the_filter_would_have_followed() {
    let ctx = context_holding(filter_graph());
    let answer = ask(
        &ctx,
        json!({
            "question": "entry",
            "max_depth": 1,
            "filters": { "provenance": ["extracted"] },
        }),
    )
    .await;
    assert_eq!(node_labels(&answer), names(&["entry", "helper", "Widget"]));
    // `Widget` has an extracted call left; `helper` only an inferred one.
    assert_eq!(answer["unexpanded_nodes"], 1);
    assert_eq!(answer["truncation_reason"], "max_depth");
    assert_eq!(answer["limits"]["max_depth"], 1);
}

// ---------------------------------------------------------------------------
// Coverage and basis.
// ---------------------------------------------------------------------------

/// A graph nobody measured is unknown, and an empty filtered answer over it
/// does not read as absence.
#[tokio::test]
async fn an_unmeasured_graph_is_unknown_and_an_empty_answer_proves_nothing() {
    let ctx = context_holding(filter_graph());
    let answer = ask(
        &ctx,
        json!({ "question": "entry", "filters": { "node_kinds": ["trait"] } }),
    )
    .await;
    assert_eq!(answer["coverage"]["status"], "unknown");
    assert_eq!(answer["basis"], Value::Null);
    assert!(array(&answer, "nodes").is_empty());
    assert!(
        answer["summary"]
            .as_str()
            .expect("summary")
            .contains("does not prove absence"),
        "{answer}"
    );
}

/// The coverage and the basis are the ones `graph_query` publishes for the
/// graph it read, complete or partial.
#[tokio::test]
async fn coverage_and_basis_are_those_of_the_graph_that_was_read() {
    let (_tmp, root) = readable_tree();
    let ctx = HandlerContext::builder().with_working_dir(&root).build();
    load_or_build_graph(ctx.analysis_service(), &root, None, None, false).expect("graph");

    let answer = ask(
        &ctx,
        json!({ "question": "caller", "filters": { "provenance": ["extracted"] } }),
    )
    .await;
    let plain = ask_unfiltered(&ctx, "caller", 3).await;
    assert_eq!(answer["coverage"]["status"], "complete");
    assert_eq!(answer["coverage"], plain["coverage"]);
    assert_eq!(answer["basis"], plain["basis"]);
    assert_eq!(answer["basis"]["graph_source"], "built");

    let (_tmp, root) = tree_with_an_ambiguous_call();
    let ctx = HandlerContext::builder().with_working_dir(&root).build();
    load_or_build_graph(ctx.analysis_service(), &root, None, None, false).expect("graph");
    let partial = ask(
        &ctx,
        json!({ "question": "caller", "filters": { "provenance": ["extracted"] } }),
    )
    .await;
    assert_eq!(partial["coverage"]["status"], "partial");
    let envelope = envelope_for(
        "graph_query_filtered",
        &Map::new(),
        &partial,
        &CursorContext {
            workspace: root.to_string_lossy().into_owned(),
            snapshot: String::new(),
        },
    );
    assert_eq!(envelope.state, "partial");
}

/// The envelope reads the cut from the answer, as it does for `graph_query`.
#[tokio::test]
async fn the_envelope_reports_why_the_answer_was_cut() {
    let ctx = context_holding(filter_graph());
    let answer = ask(&ctx, json!({ "question": "entry", "limit": 3 })).await;
    let envelope = envelope_for(
        "graph_query_filtered",
        &Map::new(),
        &answer,
        &CursorContext {
            workspace: String::new(),
            snapshot: String::new(),
        },
    );
    assert_eq!(envelope.budget.reason.as_deref(), Some("budget"));
}
