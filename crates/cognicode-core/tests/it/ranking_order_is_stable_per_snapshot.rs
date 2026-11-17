//! The ranking of `graph_search_idf` is a function of the snapshot and the
//! query, and nothing else.

use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::graph::analytics::search_ranker::SearchRanker;

/// A graph with ties (the same name in several files), hubs (a symbol
/// that most others call) and compound names in three spellings.
fn fixture_graph() -> CallGraph {
    let verbs = ["get", "set", "load", "build"];
    let nouns = ["user", "order", "index", "graph"];
    let files = ["a.rs", "b.rs", "c.rs"];
    let mut graph = CallGraph::new();
    let mut ids: Vec<SymbolId> = Vec::new();
    let mut line = 1;
    for file in files {
        for verb in verbs {
            for noun in nouns {
                let name = match file {
                    "a.rs" => format!("{verb}_{noun}"),
                    "b.rs" => format!("{verb}{}{}", &noun[..1].to_uppercase(), &noun[1..]),
                    _ => format!("{verb}::{noun}"),
                };
                ids.push(graph.add_symbol(Symbol::new(
                    &name,
                    SymbolKind::Function,
                    Location::new(file, line, 1),
                )));
                line += 1;
            }
        }
    }
    let hub = graph.add_symbol(Symbol::new(
        "process",
        SymbolKind::Function,
        Location::new("hub.rs", 1, 1),
    ));
    for (i, id) in ids.iter().enumerate() {
        // Most symbols call the hub; a few call a neighbour.
        if i % 5 != 0 {
            graph
                .add_dependency(id, &hub, DependencyType::Calls)
                .unwrap();
        }
        let next = &ids[(i * 7 + 3) % ids.len()];
        if next != id {
            graph
                .add_dependency(id, next, DependencyType::Calls)
                .unwrap();
        }
    }
    graph
}

const QUERIES: &[&str] = &[
    "user",
    "get_user",
    "getUser",
    "process",
    "graph index",
    "BUILD",
    "set",
    "",
    "zzz-no-match",
];

/// `(symbol id, score bits, degree)` of every result: the whole observable
/// outcome, with the score compared bit for bit.
fn observable(graph: &CallGraph, query: &str, k: usize) -> Vec<(String, u64, usize)> {
    SearchRanker::search(graph, query, k)
        .into_iter()
        .map(|r| {
            (
                r.symbol_id.as_str().to_string(),
                r.idf_score.to_bits(),
                r.degree,
            )
        })
        .collect()
}

/// Expected top 5 per query, recorded from the ranking before any
/// statistics were reused: `(query, [(symbol id, score bits, degree)])`.
#[allow(clippy::type_complexity)]
const GOLDEN_TOP5: &[(&str, &[(&str, u64, usize)])] = &[
    (
        "user",
        &[
            ("a.rs:build_user:13", 4644204066119821087, 3),
            ("a.rs:get_user:1", 4644204066119821087, 2),
            ("a.rs:load_user:9", 4644204066119821087, 3),
            ("a.rs:set_user:5", 4644204066119821087, 3),
            ("b.rs:buildUser:29", 4644204066119821087, 3),
        ],
    ),
    (
        "get_user",
        &[
            ("a.rs:get_user:1", 4660027400548313272, 2),
            ("b.rs:getUser:17", 4644204066119821087, 3),
            ("c.rs:get::user:33", 4644204066119821087, 3),
            ("a.rs:build_user:13", 4639700466492450591, 3),
            ("a.rs:get_graph:4", 4639700466492450591, 3),
        ],
    ),
    (
        "getUser",
        &[
            ("b.rs:getUser:17", 4660027400548313272, 3),
            ("a.rs:get_user:1", 4644204066119821087, 2),
            ("c.rs:get::user:33", 4644204066119821087, 3),
            ("a.rs:build_user:13", 4639700466492450591, 3),
            ("a.rs:get_graph:4", 4639700466492450591, 3),
        ],
    ),
    ("process", &[("hub.rs:process:1", 4644974913917147753, 38)]),
    ("graph index", &[]),
    (
        "BUILD",
        &[
            ("a.rs:build_index:15", 4641074578974503143, 3),
            ("b.rs:buildIndex:31", 4641074578974503143, 2),
            ("c.rs:build::index:47", 4641074578974503143, 3),
            ("a.rs:build_user:13", 4641019614475221041, 3),
            ("b.rs:buildUser:29", 4641019614475221041, 3),
        ],
    ),
    (
        "set",
        &[
            ("a.rs:set_graph:8", 4644204066119821087, 3),
            ("a.rs:set_index:7", 4644204066119821087, 3),
            ("a.rs:set_order:6", 4644204066119821087, 2),
            ("a.rs:set_user:5", 4644204066119821087, 3),
            ("b.rs:setGraph:24", 4644204066119821087, 3),
        ],
    ),
    ("", &[]),
    ("zzz-no-match", &[]),
];

/// Every hit of "BUILD", in order, with ties broken by insertion order.
const GOLDEN_BUILD_ORDER: &[&str] = &[
    "a.rs:build_index:15",
    "b.rs:buildIndex:31",
    "c.rs:build::index:47",
    "a.rs:build_user:13",
    "b.rs:buildUser:29",
    "c.rs:build::user:45",
    "a.rs:build_order:14",
    "b.rs:buildOrder:30",
    "c.rs:build::order:46",
    "a.rs:build_graph:16",
    "b.rs:buildGraph:32",
    "c.rs:build::graph:48",
    "a.rs:load_index:11",
    "b.rs:loadIndex:27",
    "c.rs:load::index:43",
    "a.rs:load_user:9",
    "b.rs:loadUser:25",
    "c.rs:load::user:41",
    "a.rs:load_order:10",
    "b.rs:loadOrder:26",
    "c.rs:load::order:42",
    "a.rs:get_index:3",
    "a.rs:load_graph:12",
    "a.rs:set_index:7",
    "b.rs:getIndex:19",
    "b.rs:loadGraph:28",
    "b.rs:setIndex:23",
    "c.rs:get::index:35",
    "c.rs:load::graph:44",
    "c.rs:set::index:39",
    "a.rs:get_user:1",
    "a.rs:set_user:5",
    "b.rs:getUser:17",
    "b.rs:setUser:21",
    "c.rs:get::user:33",
    "c.rs:set::user:37",
    "a.rs:get_order:2",
    "a.rs:set_order:6",
    "b.rs:getOrder:18",
    "b.rs:setOrder:22",
    "c.rs:get::order:34",
    "c.rs:set::order:38",
];

#[test]
fn golden_top5_matches_recorded_ranking() {
    let graph = fixture_graph();
    assert_eq!(GOLDEN_TOP5.len(), QUERIES.len(), "every query has a golden");
    for (query, expected) in GOLDEN_TOP5 {
        let got = observable(&graph, query, 5);
        let want: Vec<(String, u64, usize)> = expected
            .iter()
            .map(|(id, bits, degree)| (id.to_string(), *bits, *degree))
            .collect();
        assert_eq!(got, want, "query {query:?}");
    }
}

#[test]
fn golden_build_order_is_total_and_stable() {
    let graph = fixture_graph();
    let ids: Vec<String> = observable(&graph, "BUILD", 100)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(ids, GOLDEN_BUILD_ORDER);
}
