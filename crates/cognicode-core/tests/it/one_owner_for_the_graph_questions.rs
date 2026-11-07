//! TASK-78 — one owner answers the graph questions, and the copies that had
//! no caller stay gone.
//!
//! The CLI and the MCP are adapters over one service, and the questions they
//! put are the same ones: the hot paths, the entry points, the leaf
//! functions, the path between two names, the hierarchy and the diagram.
//! Before this unit each surface answered on its own, and the answers
//! drifted:
//!
//! - the CLI cut the hot paths to the limit BEFORE it applied the fan-in
//!   threshold, and dropped the symbols nobody calls whatever the
//!   threshold said;
//! - the CLI cut the entry points and the leaf functions at twenty without
//!   saying so;
//! - the CLI asked the trace for a fully qualified name, so the simple name
//!   the MCP answers was "not found" there;
//! - the CLI walked a graph of its own for the hierarchy, cut every bucket
//!   at five, and the MCP ignored the depth and the direction its own tool
//!   declares;
//! - the diagram had three renderers, and the two surfaces that published
//!   one published different documents.
//!
//! `AnalysisService`'s graph questions are the owner. This file pins three
//! things:
//!
//! - every surface reaches the owner;
//! - the copies that had no caller are gone, and do not come back;
//! - the owner answers the questions the way the surfaces promise.
//!
//! The behavioural proof of the first point is the CLI↔MCP equivalence (in
//! `cognicode-cli/tests/it/graph_cli_mcp_equivalence.rs`); this is the
//! structural half, and it also covers the surfaces the equivalence cannot
//! reach — the workspace session, which the rig publishes.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::application::services::graph_queries::{Direction, GraphQueryError};
use cognicode_core::domain::aggregates::Symbol;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};

use crate::source_scan::{declares_word, rust_sources};

/// The surfaces that ask the graph questions, and the owner calls each of
/// them owes.
///
/// Two of the tokens are the door itself rather than the name of a
/// question: `consolidated_handlers` and `aix_handlers` answered the hot
/// paths on their own, and a copy of the question is a copy of the answer
/// — one that ordered the list by the number of callers alone, and so
/// published a different list on every run.
const SURFACES: [(&str, &[&str]); 5] = [
    (
        "crates/cognicode-cli/src/cli/commands.rs",
        &[
            "hot_paths",
            "entry_points",
            "leaf_functions",
            "trace_path_by_name",
            "call_hierarchy",
            "mermaid_diagram",
        ],
    ),
    (
        "crates/cognicode-core/src/interface/mcp/handlers/mod.rs",
        &[
            "hot_paths",
            "entry_points",
            "leaf_functions",
            "call_hierarchy",
            "mermaid_diagram",
            "resolve_symbol",
        ],
    ),
    (
        "crates/cognicode-core/src/interface/mcp/handlers/aix_handlers.rs",
        &[
            "resolve_symbol",
            "trace_path_between",
            "AnalysisService::hot_paths",
        ],
    ),
    (
        "crates/cognicode-core/src/interface/mcp/handlers/consolidated_handlers.rs",
        &["AnalysisService::hot_paths"],
    ),
    (
        "crates/cognicode-core/src/application/workspace_session.rs",
        &["call_hierarchy", "mermaid_diagram"],
    ),
];

/// The copies of the graph questions that had no production caller.
///
/// The tokens are assembled at runtime: written out whole they would be
/// literals in this very file, and the scan would find the scan.
fn dead_copies() -> Vec<String> {
    [
        ["find", "path", "bfs"].join("_"),
        ["find", "all", "paths", "bfs"].join("_"),
        ["collect", "edges", "recursive"].join("_"),
        ["sanitize", "mermaid", "id"].join("_"),
        ["present", "symbol", "ids"].join("_"),
        ["build", "symbol", "name", "index"].join("_"),
        // The renderer that answered without options, next to the one that
        // answers with them: two renderers, two documents.
        ["to", "mermaid"].join("_"),
    ]
    .into_iter()
    .collect()
}

/// A symbol of the given name, in the given file.
fn symbol(name: &str, file: &str, line: u32) -> Symbol {
    Symbol::new(name, SymbolKind::Function, Location::new(file, line, 1))
}

/// A graph with a caller, a middle and a leaf, plus a symbol nobody calls.
///
/// `caller` calls `middle`, `middle` calls `leaf`, and `lonely` is called by
/// nobody: the shape every question in this file needs.
fn graph() -> CallGraph {
    let mut graph = CallGraph::new();
    let caller = graph.add_symbol(symbol("caller", "a.rs", 1));
    let middle = graph.add_symbol(symbol("middle", "a.rs", 2));
    let leaf = graph.add_symbol(symbol("leaf", "a.rs", 3));
    let lonely = graph.add_symbol(symbol("lonely", "b.rs", 1));
    graph
        .add_dependency(&caller, &middle, DependencyType::Calls)
        .unwrap();
    graph
        .add_dependency(&middle, &leaf, DependencyType::Calls)
        .unwrap();
    let _ = lonely;
    graph
}

/// The surfaces reach the owner, not a copy of it.
#[test]
fn the_surfaces_call_the_one_owner() {
    let sources = rust_sources();

    for (surface, questions) in SURFACES {
        let content = sources
            .iter()
            .find(|(path, _)| path == surface)
            .map(|(_, content)| content.as_str())
            .unwrap_or_else(|| panic!("{surface} is part of the tree"));
        for question in questions {
            assert!(
                declares_word(content, question),
                "{surface} does not ask the owner for `{question}`; a surface \
                 with its own answer is a second answer to the same question"
            );
        }
    }
}

/// The copies with no production caller are gone, and they do not come back.
#[test]
fn the_copies_without_a_caller_are_gone() {
    let sources = rust_sources();

    for dead in dead_copies() {
        let still_there: Vec<&str> = sources
            .iter()
            .filter(|(_, content)| declares_word(content, &dead))
            .map(|(path, _)| path.as_str())
            .collect();
        assert!(
            still_there.is_empty(),
            "`{dead}` had no production caller and it is back in: {still_there:?}"
        );
    }
}

/// The scan sees the tree it is scanning.
///
/// A scan that answers «yes» to everything proves nothing, so this test
/// pins both directions: a name that IS there is found, and a name that is
/// NOT there is not reported.
#[test]
fn the_scan_sees_the_tree_and_not_only_its_absence() {
    let sources = rust_sources();

    // The ghost is assembled at runtime: written out, it would be a literal
    // in this very file, and the scan would find the scan.
    let ghost = ["a", "graph", "question", "nobody", "asks"].join("_");
    assert!(
        !sources
            .iter()
            .any(|(_, content)| declares_word(content, &ghost)),
        "a name no file declares must not be reported as present"
    );

    let owner = sources
        .iter()
        .filter(|(_, content)| declares_word(content, "mermaid_diagram"))
        .count();
    assert!(
        owner >= 3,
        "the owner is called from at least three places; the scan found {owner}"
    );
}

/// The threshold is the only rule the hot paths follow.
///
/// The CLI used to answer with the symbols somebody calls whatever the
/// threshold said, and to cut the list to the limit before it applied the
/// threshold. A threshold of zero asks for every symbol, including the ones
/// nothing calls.
#[test]
fn the_threshold_is_the_only_rule_of_the_hot_paths() {
    let graph = graph();

    let every = AnalysisService::hot_paths(&graph, 10, 0);
    let names: Vec<&str> = every.iter().map(|row| row.name.as_str()).collect();
    assert!(
        names.contains(&"lonely"),
        "a threshold of zero asks for every symbol, and the symbol nobody \
         calls is one of them: {names:?}"
    );

    let called = AnalysisService::hot_paths(&graph, 10, 1);
    let called_names: Vec<&str> = called.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(
        called_names,
        vec!["leaf", "middle"],
        "a threshold of one asks for the symbols somebody calls: {called_names:?}"
    );
}

/// The order of the table is a promise.
///
/// The graph holds its symbols in a map, and a map's order is not one. Two
/// runs over the same graph have to publish the same table, and among equal
/// fan-ins the name decides.
#[test]
fn the_order_of_the_hot_paths_is_a_promise() {
    let graph = graph();

    let first = AnalysisService::hot_paths(&graph, 10, 0);
    let second = AnalysisService::hot_paths(&graph, 10, 0);
    let names: Vec<&str> = first.iter().map(|row| row.name.as_str()).collect();
    let again: Vec<&str> = second.iter().map(|row| row.name.as_str()).collect();

    assert_eq!(
        names, again,
        "two runs over the same graph publish different tables: {names:?} / {again:?}"
    );
    assert_eq!(
        names,
        vec!["leaf", "middle", "caller", "lonely"],
        "the table is ordered by fan-in and, among equal fan-ins, by name: {names:?}"
    );
}

/// A name that several symbols carry is refused, and one that none carries
/// is refused too.
///
/// The application layer does not pick between homonyms: it cannot prove
/// which one was meant, and an answer through a symbol nobody named is
/// worse than no answer.
#[test]
fn a_name_is_refused_when_the_graph_does_not_decide_it() {
    let mut graph = graph();
    let other = graph.add_symbol(symbol("middle", "b.rs", 9));

    let refused = AnalysisService::resolve_symbol(&graph, "middle");
    match refused {
        Err(GraphQueryError::Ambiguous { name, candidates }) => {
            assert_eq!(name, "middle");
            assert_eq!(
                candidates.len(),
                2,
                "both homonyms are named: {candidates:?}"
            );
        }
        other => panic!("two symbols carry the name, and it is refused: {other:?}"),
    }

    let absent = AnalysisService::resolve_symbol(&graph, "nobody");
    assert!(
        matches!(absent, Err(GraphQueryError::NotFound { .. })),
        "a name no symbol carries is not found: {absent:?}"
    );
    let _ = other;
}

/// The trace answers the simple name the MCP answers.
///
/// The CLI used to build the symbol id from the name as it was written, so
/// the simple name was "not found" there and the fully qualified one was the
/// only way in.
#[test]
fn the_trace_answers_a_simple_name() {
    let graph = graph();

    let path = AnalysisService::trace_path_by_name(&graph, "middle", "leaf", 10)
        .expect("both names are in the graph")
        .expect("the middle calls the leaf");
    let names: Vec<&str> = path.iter().map(|symbol| symbol.name()).collect();

    assert_eq!(
        names,
        vec!["middle", "leaf"],
        "the path names the symbols it goes through: {names:?}"
    );
}

/// The hierarchy walks the side it is asked about, and up to the depth it
/// is given.
///
/// The MCP used to ignore both the direction and the depth its own tool
/// declares, and to answer for every symbol that carries the name.
#[test]
fn the_hierarchy_walks_the_side_it_is_asked_about() {
    let graph = graph();

    let callees =
        AnalysisService::call_hierarchy(&graph, "middle", 1, Direction::Callees).expect("middle");
    let callee_names: Vec<&str> = callees.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(
        callee_names,
        vec!["leaf"],
        "one hop down from the middle is the leaf: {callee_names:?}"
    );

    let callers =
        AnalysisService::call_hierarchy(&graph, "middle", 1, Direction::Callers).expect("middle");
    let caller_names: Vec<&str> = callers.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(
        caller_names,
        vec!["caller"],
        "one hop up from the middle is the caller: {caller_names:?}"
    );

    let deeper =
        AnalysisService::call_hierarchy(&graph, "leaf", 2, Direction::Callers).expect("leaf");
    let deeper_names: Vec<&str> = deeper.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(
        deeper_names,
        vec!["middle", "caller"],
        "two hops up from the leaf reach the caller: {deeper_names:?}"
    );
}
