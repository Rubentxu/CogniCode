//! `graph_query` publishes the relations the graph holds, what it covers and
//! where it stopped.
//!
//! ## The defect
//!
//! The traversal followed every outgoing edge whatever its kind, and then
//! wrote the same three words on all of them: `calls`, `Extracted`, `1.0`. An
//! import, a reference, an inheritance and an ambiguous call all read as a
//! certain call. It also dropped every edge that did not discover a new
//! node (a second kind of relation between the same pair, an edge to a node
//! already reached), counted only nodes against the budget, and said nothing
//! about what the graph covers: an empty answer over a graph with holes read
//! as "nothing there".
//!
//! ## What this file measures
//!
//! The real handler over a real `CallGraph`, read the way the MCP wire
//! carries it (the serialized output). The expected values are written here,
//! by hand, from the way each edge was added: the test does not ask the code
//! under test what it should have said.

use cognicode_core::application::analysis_basis::GraphSource;
use cognicode_core::application::graph_measure::GraphMeasure;
use cognicode_core::application::graph_snapshot::{load_or_build_graph, source_digest};
use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::application::services::analyze::Coverage;
use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::traits::graph_store::GraphStore;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::persistence::InMemoryGraphStore;
use cognicode_core::interface::cursor::CursorContext;
use cognicode_core::interface::hateoas::envelope_for;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::graph_query_handlers::{
    GraphQueryInput, handle_graph_query,
};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Fixtures.
// ---------------------------------------------------------------------------

fn symbol(graph: &mut CallGraph, name: &str, line: u32) -> SymbolId {
    graph.add_symbol(Symbol::new(
        name,
        SymbolKind::Function,
        Location::new("lib.rs", line, 0),
    ))
}

pub(super) fn link(
    graph: &mut CallGraph,
    from: &SymbolId,
    to: &SymbolId,
    kind: DependencyType,
    how: ExtractionContext,
) {
    graph
        .add_dependency_with_provenance(from, to, kind, how)
        .expect("both ends are symbols of the graph");
}

/// A graph with every kind of relation the traversal has to keep apart.
///
/// ```text
/// entry --calls (direct)------> helper
/// entry --references (direct)-> helper          parallel to the call
/// entry --imports (direct)----> util
/// entry --calls (heuristic)---> dynamic_target  inferred, 0.7
/// entry --calls (unresolved)--> maybe_target    ambiguous, 0.3
/// helper --calls (direct)-----> util            reaches a node already known
/// helper --inherits (manual)--> base
/// util --calls (heuristic)----> loader          inferred, 0.6
/// ```
///
/// 7 nodes, 8 edges. The insertion `order` changes how the graph was
/// written, not what it holds.
fn typed_graph(reversed: bool) -> CallGraph {
    let mut graph = CallGraph::new();
    // The line is part of a symbol's identity: it is fixed by the name, so
    // that writing the graph in another order does not name other symbols.
    let mut names: Vec<(u32, &str)> = [
        "entry",
        "helper",
        "util",
        "dynamic_target",
        "maybe_target",
        "base",
        "loader",
    ]
    .into_iter()
    .enumerate()
    .map(|(line, name)| (line as u32 + 1, name))
    .collect();
    if reversed {
        names.reverse();
    }
    let mut ids: HashMap<&str, SymbolId> = HashMap::new();
    for (line, name) in names {
        ids.insert(name, symbol(&mut graph, name, line));
    }
    let id = |name: &str| ids[name].clone();

    let mut edges: Vec<(&str, &str, DependencyType, ExtractionContext)> = vec![
        (
            "entry",
            "helper",
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        ),
        (
            "entry",
            "helper",
            DependencyType::References,
            ExtractionContext::DirectExtraction,
        ),
        (
            "entry",
            "util",
            DependencyType::Imports,
            ExtractionContext::DirectExtraction,
        ),
        (
            "entry",
            "dynamic_target",
            DependencyType::Calls,
            ExtractionContext::Heuristic { score: 0.7 },
        ),
        (
            "entry",
            "maybe_target",
            DependencyType::Calls,
            ExtractionContext::Unresolved,
        ),
        (
            "helper",
            "util",
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        ),
        (
            "helper",
            "base",
            DependencyType::Inherits,
            ExtractionContext::Manual,
        ),
        (
            "util",
            "loader",
            DependencyType::Calls,
            ExtractionContext::Heuristic { score: 0.6 },
        ),
    ];
    if reversed {
        edges.reverse();
    }
    for (from, to, kind, how) in edges {
        link(&mut graph, &id(from), &id(to), kind, how);
    }
    graph
}

/// A context whose graph is `graph`, handed over by nobody who measured it.
pub(super) fn context_holding(graph: CallGraph) -> HandlerContext {
    let ctx = HandlerContext::builder().build();
    ctx.analysis_service().graph_cache().replace(graph);
    ctx
}

async fn ask(ctx: &HandlerContext, question: &str, max_depth: usize, budget: usize) -> Value {
    let output = handle_graph_query(
        ctx,
        GraphQueryInput {
            question: question.to_string(),
            max_depth,
            budget,
            continuation_token: None,
        },
    )
    .await
    .expect("the graph query answers");
    serde_json::to_value(output).expect("the answer is JSON")
}

pub(super) fn array<'a>(answer: &'a Value, field: &str) -> &'a Vec<Value> {
    answer[field]
        .as_array()
        .unwrap_or_else(|| panic!("`{field}` is an array: {answer}"))
}

/// `id -> label` of the nodes the answer returns.
fn labels(answer: &Value) -> HashMap<String, String> {
    array(answer, "nodes")
        .iter()
        .map(|n| {
            (
                n["id"].as_str().expect("id").to_string(),
                n["label"].as_str().expect("label").to_string(),
            )
        })
        .collect()
}

/// The edges of the answer as `(source, target, relation)` labels, with the
/// edge itself.
fn edges_by_label(answer: &Value) -> Vec<((String, String, String), Value)> {
    let names = labels(answer);
    array(answer, "edges")
        .iter()
        .map(|e| {
            let name = |field: &str| {
                let id = e[field].as_str().expect("endpoint");
                names
                    .get(id)
                    .unwrap_or_else(|| panic!("the endpoint {id} is a node of the answer"))
                    .clone()
            };
            (
                (
                    name("source"),
                    name("target"),
                    e["relation"].as_str().expect("relation").to_string(),
                ),
                e.clone(),
            )
        })
        .collect()
}

fn edge<'a>(
    edges: &'a [((String, String, String), Value)],
    source: &str,
    target: &str,
    relation: &str,
) -> &'a Value {
    edges
        .iter()
        .find(|((s, t, r), _)| s == source && t == target && r == relation)
        .map(|(_, e)| e)
        .unwrap_or_else(|| {
            panic!(
                "the edge {source} --{relation}--> {target} is in the answer: {:?}",
                edges.iter().map(|(k, _)| k).collect::<Vec<_>>()
            )
        })
}

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("directory");
    std::fs::write(path, contents).expect("the file is written");
}

/// A tree that is read whole: a function and the one that calls it.
pub(super) fn readable_tree() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    write(
        &root.join("src/lib.rs"),
        "pub fn helper() -> u32 {\n    1\n}\n\npub fn caller() -> u32 {\n    helper()\n}\n",
    );
    (dir, root)
}

/// A tree with a call the graph cannot tie to one declaration: two functions
/// carry the name, so the relation stays loose and the graph lacks an edge.
pub(super) fn tree_with_an_ambiguous_call() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    write(
        &root.join("src/one.rs"),
        "pub fn helper() -> u32 {\n    1\n}\n",
    );
    write(
        &root.join("src/two.rs"),
        "pub fn helper() -> u32 {\n    2\n}\n",
    );
    write(
        &root.join("src/lib.rs"),
        "pub fn caller() -> u32 {\n    helper()\n}\n",
    );
    (dir, root)
}

// ---------------------------------------------------------------------------
// The relations keep what the graph holds.
// ---------------------------------------------------------------------------

/// An edge says what kind of relation it is, how it was established and how
/// sure the graph is: not three words written for every edge.
#[tokio::test]
async fn every_edge_keeps_its_kind_its_provenance_and_its_confidence() {
    let ctx = context_holding(typed_graph(false));
    let answer = ask(&ctx, "entry", 3, 1500).await;
    let edges = edges_by_label(&answer);

    let call = edge(&edges, "entry", "helper", "calls");
    assert_eq!(call["provenance"], "Extracted");
    assert_eq!(call["confidence"], 1.0);

    let import = edge(&edges, "entry", "util", "imports");
    assert_eq!(import["provenance"], "Extracted");

    let inferred = edge(&edges, "entry", "dynamic_target", "calls");
    assert_eq!(inferred["provenance"], "Inferred");
    assert_eq!(inferred["confidence"], 0.7);

    let ambiguous = edge(&edges, "entry", "maybe_target", "calls");
    assert_eq!(ambiguous["provenance"], "Ambiguous");
    assert_eq!(ambiguous["confidence"], 0.3);

    let inherits = edge(&edges, "helper", "base", "inherits");
    assert_eq!(inherits["provenance"], "Manual");

    let deep = edge(&edges, "util", "loader", "calls");
    assert_eq!(deep["provenance"], "Inferred");
    assert_eq!(deep["confidence"], 0.6);
}

/// Two kinds of relation between the same pair are two edges, and an edge
/// that reaches a node the traversal already knew is an edge too.
#[tokio::test]
async fn parallel_relations_and_edges_to_known_nodes_are_kept() {
    let ctx = context_holding(typed_graph(false));
    let answer = ask(&ctx, "entry", 3, 1500).await;
    let edges = edges_by_label(&answer);

    assert_eq!(
        array(&answer, "nodes").len(),
        7,
        "every node reached is returned once: {answer}"
    );
    assert_eq!(
        array(&answer, "edges").len(),
        8,
        "every edge the traversal followed is returned: {answer}"
    );
    // Parallel to the call.
    edge(&edges, "entry", "helper", "references");
    // `util` was discovered through the import; this edge only reaches it.
    let known = edge(&edges, "helper", "util", "calls");
    assert_eq!(
        known["discovers"], false,
        "an edge to a node already reached does not discover it: {known}"
    );
    let first = edge(&edges, "entry", "util", "imports");
    assert_eq!(first["discovers"], true);
}

/// The answer says which of the two it is: a traversal from the seeds, not
/// the subgraph induced by the nodes it returned.
#[tokio::test]
async fn the_answer_says_it_is_a_traversal_and_not_an_induced_subgraph() {
    let ctx = context_holding(typed_graph(false));
    let answer = ask(&ctx, "entry", 1, 1500).await;

    assert_eq!(answer["mode"], "traversal");
    assert_eq!(answer["direction"], "outgoing");
    let edges = edges_by_label(&answer);
    // At depth 1 `helper` and `util` are returned but not expanded, so the
    // edge between them is a relation of the induced subgraph that the
    // traversal did not follow: it is not in the answer, and the answer
    // says it stopped at the depth.
    assert!(
        edges
            .iter()
            .all(|((s, t, _), _)| !(s == "helper" && t == "util")),
        "a traversal does not report edges it did not follow: {answer}"
    );
    assert_eq!(answer["truncated"], true);
    assert_eq!(answer["truncation_reason"], "max_depth");
    assert!(
        answer["unexpanded_nodes"].as_u64().expect("count") >= 2,
        "the nodes whose outgoing edges were not followed are counted: {answer}"
    );
}

// ---------------------------------------------------------------------------
// Limits.
// ---------------------------------------------------------------------------

fn size(answer: &Value) -> usize {
    array(answer, "nodes").len() + array(answer, "edges").len()
}

/// The budget counts seeds, nodes and edges, and the answer never goes over
/// it, whatever the budget is.
#[tokio::test]
async fn the_budget_counts_nodes_and_edges_and_is_never_exceeded() {
    let ctx = context_holding(typed_graph(false));
    let full = ask(&ctx, "entry", 3, 1500).await;
    assert_eq!(size(&full), 15);
    assert_eq!(full["truncated"], false);
    assert_eq!(full["truncation_reason"], Value::Null);
    assert_eq!(full["limits"]["budget_used"], 15);

    for budget in 0..=15 {
        let answer = ask(&ctx, "entry", 3, budget).await;
        assert!(
            size(&answer) <= budget,
            "budget {budget}: the answer holds {} elements: {answer}",
            size(&answer)
        );
        assert_eq!(answer["limits"]["budget"], budget);
        if budget < 15 {
            assert_eq!(answer["truncated"], true, "budget {budget}");
            assert_eq!(answer["truncation_reason"], "budget", "budget {budget}");
        }
    }
}

/// A larger budget extends the answer of a smaller one: the order is the
/// discovery order, and it does not depend on the budget.
#[tokio::test]
async fn a_larger_budget_extends_the_answer_of_a_smaller_one() {
    let ctx = context_holding(typed_graph(false));
    let full = ask(&ctx, "entry", 3, 1500).await;
    for budget in 0..15 {
        let cut = ask(&ctx, "entry", 3, budget).await;
        for field in ["nodes", "edges"] {
            let whole = array(&full, field);
            let part = array(&cut, field);
            assert_eq!(
                part.as_slice(),
                &whole[..part.len()],
                "budget {budget}: `{field}` is a prefix of the full answer"
            );
        }
    }
}

/// Budget 0 returns nothing and says why; depth 0 returns the seeds and says
/// the depth stopped it.
#[tokio::test]
async fn a_zero_budget_and_a_zero_depth_are_bounded_and_say_so() {
    let ctx = context_holding(typed_graph(false));

    let none = ask(&ctx, "entry", 3, 0).await;
    assert!(array(&none, "nodes").is_empty());
    assert!(array(&none, "edges").is_empty());
    assert_eq!(none["truncated"], true);
    assert_eq!(none["truncation_reason"], "budget");

    let seeds_only = ask(&ctx, "entry", 0, 1500).await;
    assert_eq!(array(&seeds_only, "nodes").len(), 1);
    assert!(array(&seeds_only, "edges").is_empty());
    assert_eq!(seeds_only["truncated"], true);
    assert_eq!(seeds_only["truncation_reason"], "max_depth");
    assert_eq!(seeds_only["limits"]["depth_reached"], 0);
}

/// A node with a very large fan-out does not carry the answer past the
/// budget: the budget is checked for every element, not once per node.
#[tokio::test]
async fn a_large_fan_out_stays_inside_the_budget() {
    let mut graph = CallGraph::new();
    let hub = symbol(&mut graph, "hub", 1);
    for i in 0..5_000 {
        let spoke = symbol(&mut graph, &format!("spoke_{i:05}"), i + 2);
        link(
            &mut graph,
            &hub,
            &spoke,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
    }
    let ctx = context_holding(graph);

    let answer = ask(&ctx, "hub", 3, 100).await;
    assert_eq!(size(&answer), 100, "the budget is spent and not exceeded");
    assert_eq!(answer["truncated"], true);
    assert_eq!(answer["truncation_reason"], "budget");
    assert_eq!(answer["limits"]["budget_used"], 100);
}

/// The same question over the same graph is the same answer, however the
/// graph was written, and every time it is asked.
#[tokio::test]
async fn the_answer_does_not_depend_on_how_the_graph_was_written() {
    let one = ask(&context_holding(typed_graph(false)), "entry", 3, 1500).await;
    let other = ask(&context_holding(typed_graph(true)), "entry", 3, 1500).await;
    assert_eq!(one, other);

    let ctx = context_holding(typed_graph(false));
    assert_eq!(
        ask(&ctx, "entry", 3, 1500).await,
        ask(&ctx, "entry", 3, 1500).await
    );
}

/// Seeds that several words of the question match are one node that says
/// every word that matched it.
#[tokio::test]
async fn a_node_matched_by_two_words_says_both() {
    let ctx = context_holding(typed_graph(false));
    let answer = ask(&ctx, "entry helper", 0, 1500).await;
    let nodes = array(&answer, "nodes");
    assert_eq!(nodes.len(), 2, "{answer}");
    let matched: Vec<&str> = nodes
        .iter()
        .map(|n| n["why_matched"][0].as_str().expect("word"))
        .collect();
    assert_eq!(matched, ["entry", "helper"]);
}

// ---------------------------------------------------------------------------
// Coverage and basis.
// ---------------------------------------------------------------------------

/// A graph nobody measured is "unknown": an answer over it is not an answer
/// over a graph found to be complete, and an empty one is not proof of
/// absence.
#[tokio::test]
async fn a_graph_nobody_measured_is_unknown_not_complete() {
    let ctx = context_holding(typed_graph(false));
    let answer = ask(&ctx, "entry", 3, 1500).await;

    assert_eq!(answer["coverage"]["status"], "unknown");
    assert_eq!(
        answer["basis"],
        Value::Null,
        "nothing says which tree or snapshot the graph came from: {answer}"
    );
    assert!(
        answer["summary"]
            .as_str()
            .expect("summary")
            .contains("unknown"),
        "the one line says what the coverage is: {answer}"
    );

    let nothing = ask(&ctx, "does_not_exist_anywhere", 3, 1500).await;
    assert!(array(&nothing, "nodes").is_empty());
    assert_eq!(nothing["coverage"]["status"], "unknown");
    assert!(
        nothing["summary"]
            .as_str()
            .expect("summary")
            .contains("unknown"),
        "an empty answer over an unmeasured graph is not 'nothing there': {nothing}"
    );
}

#[tokio::test]
async fn an_injected_store_does_not_reuse_a_measure_from_the_service_graph() {
    let (_tmp, root) = readable_tree();
    let service = AnalysisService::new();
    let measured_graph = typed_graph(false);
    service.graph_cache().replace(measured_graph.clone());
    service.graph_measures().record(GraphMeasure::new(
        service.current_graph_revision(),
        &measured_graph,
        Coverage::Complete,
        GraphSource::Built,
        "/workspace".into(),
        None,
    ));

    let store = InMemoryGraphStore::new();
    store
        .save_graph(&measured_graph)
        .expect("the injected graph is stored");
    let ctx = HandlerContext::builder()
        .with_working_dir(&root)
        .with_analysis_service(service)
        .with_graph_store(store)
        .build();
    let answer = ask(&ctx, "entry", 3, 1500).await;

    assert_eq!(
        answer["coverage"]["status"], "unknown",
        "an external store's graph has no measure tied to its identity: {answer}"
    );
    assert_eq!(answer["basis"], Value::Null);
}

/// An answer over a graph with holes says so, names the holes, and the
/// envelope reads it as partial.
#[tokio::test]
async fn a_partial_graph_publishes_its_holes_and_the_answer_is_partial() {
    let (_tmp, root) = tree_with_an_ambiguous_call();
    let ctx = HandlerContext::builder().with_working_dir(&root).build();
    load_or_build_graph(ctx.analysis_service(), &root, None, None, false).expect("graph");

    let answer = ask(&ctx, "caller", 3, 1500).await;

    assert_eq!(answer["coverage"]["status"], "partial");
    let pending = answer["coverage"]["pending_references"]
        .as_array()
        .expect("pending references");
    assert!(
        pending
            .iter()
            .any(|p| p["target"] == "helper" && p["reason"] == "ambiguous"),
        "the loose call is named: {answer}"
    );
    assert!(
        answer["summary"]
            .as_str()
            .expect("summary")
            .contains("partial"),
        "{answer}"
    );

    let envelope = envelope_for(
        "graph_query",
        &Map::new(),
        &answer,
        &CursorContext {
            workspace: root.to_string_lossy().into_owned(),
            snapshot: String::new(),
        },
    );
    assert_eq!(envelope.state, "partial");
}

/// The coverage and the basis come from the same read as the graph: the
/// words are the ones that read measured, and the snapshot is the one it
/// read.
#[tokio::test]
async fn coverage_and_basis_describe_the_graph_that_was_read() {
    let (_tmp, root) = readable_tree();
    let ctx = HandlerContext::builder().with_working_dir(&root).build();
    let loaded =
        load_or_build_graph(ctx.analysis_service(), &root, None, None, false).expect("graph");

    let answer = ask(&ctx, "caller", 3, 1500).await;

    assert_eq!(answer["coverage"]["status"], "complete");
    let basis = &answer["basis"];
    assert_eq!(basis["graph_source"], "built");
    assert_eq!(
        basis["workspace"],
        root.canonicalize().unwrap().to_string_lossy().as_ref()
    );
    let manifest = loaded
        .manifest
        .as_ref()
        .expect("a build keeps its manifest");
    assert_eq!(
        basis["source_manifest_digest"],
        source_digest(manifest).as_str()
    );

    let edges = edges_by_label(&answer);
    let call = edge(&edges, "caller", "helper", "calls");
    let held = loaded
        .graph
        .all_dependencies_with_metadata()
        .find(|(_, _, kind, _, _)| **kind == DependencyType::Calls)
        .expect("the graph holds the call");
    assert_eq!(call["provenance"], held.3.to_string().as_str());
    assert_eq!(call["confidence"], held.4);

    let envelope = envelope_for(
        "graph_query",
        &Map::new(),
        &answer,
        &CursorContext {
            workspace: root.to_string_lossy().into_owned(),
            snapshot: String::new(),
        },
    );
    assert_eq!(envelope.state, "complete");
    let freshness = envelope.basis.freshness.expect("the answer is dated");
    assert_eq!(freshness.snapshot, source_digest(manifest));
}

/// A session that serves the graph it already holds keeps saying which
/// snapshot it is.
#[tokio::test]
async fn a_graph_the_session_already_held_keeps_its_snapshot() {
    let (_tmp, root) = readable_tree();
    let ctx = HandlerContext::builder().with_working_dir(&root).build();
    let first =
        load_or_build_graph(ctx.analysis_service(), &root, None, None, false).expect("graph");
    let manifest = first.manifest.clone().expect("manifest");
    let second = load_or_build_graph(ctx.analysis_service(), &root, None, Some(&manifest), false)
        .expect("graph");
    assert!(second.manifest.is_none(), "the session served what it held");

    let answer = ask(&ctx, "caller", 3, 1500).await;
    assert_eq!(
        answer["basis"]["source_manifest_digest"],
        source_digest(&manifest).as_str()
    );
    assert_eq!(answer["coverage"]["status"], "complete");
}

/// A direct build records coverage for the graph it replaces.
#[tokio::test]
async fn a_direct_build_records_coverage_for_the_replaced_graph() {
    let (_tmp, root) = readable_tree();
    let ctx = HandlerContext::builder().with_working_dir(&root).build();
    load_or_build_graph(ctx.analysis_service(), &root, None, None, false).expect("graph");

    let (_other, other_root) = tree_with_an_ambiguous_call();
    ctx.analysis_service()
        .build_project_graph(&other_root)
        .expect("the graph is built");

    let answer = ask(&ctx, "caller", 3, 1500).await;
    assert_eq!(
        answer["coverage"]["status"], "partial",
        "the graph built from an ambiguous call reports its measured coverage: {answer}"
    );
    assert_eq!(
        answer["coverage"]["pending_references"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(answer["basis"], Value::Null);
}

/// The envelope reads the cut from the answer.
#[tokio::test]
async fn the_envelope_reports_why_the_answer_was_cut() {
    let ctx = context_holding(typed_graph(false));
    let answer = ask(&ctx, "entry", 3, 5).await;
    let envelope = envelope_for(
        "graph_query",
        &Map::new(),
        &answer,
        &CursorContext {
            workspace: String::new(),
            snapshot: String::new(),
        },
    );
    assert_eq!(envelope.budget.reason.as_deref(), Some("budget"));
}
