//! The array adjacency of a projection answers exactly what the petgraph
//! baseline answers.
//!
//! The oracle of this file is the petgraph walk the projection used before it
//! read from the array adjacency, written out over `projection.graph()`. Every
//! comparison is exact, in order and bit for bit: the tolerance of the
//! floating point algorithms (PageRank, personalised PageRank) is zero,
//! because the adjacency rows come out in the order petgraph walks them and
//! the sums are therefore taken in the same order.
//!
//! The graphs cover cycles, self loops, parallel edges of different kinds,
//! homonyms (the same name in different files), edges of every kind and
//! confidence, empty and single-node graphs, and seeded random graphs.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::ports::{
    CallGraphProjectionFactory, CallGraphProjectionPort, CompactAdjacency, EdgeSide,
};
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::graph::{
    CallGraphProjection, PetgraphProjectionFactory, SubgraphDirection, SubgraphEdge, SubgraphView,
};
use cognicode_graph_algos::{condensation, page_rank, personalized_pagerank};
use petgraph::Direction;
use petgraph::algo::has_path_connecting;
use petgraph::graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences, NodeIndexable};

const KINDS: [DependencyType; 8] = [
    DependencyType::Calls,
    DependencyType::Imports,
    DependencyType::Inherits,
    DependencyType::UsesGeneric,
    DependencyType::References,
    DependencyType::Defines,
    DependencyType::AnnotatedBy,
    DependencyType::Contains,
];

const DEPTHS: [usize; 5] = [0, 1, 2, 4, usize::MAX];

// ---------------------------------------------------------------------------
// Oracle: the petgraph walks the projection used before the array adjacency.
// ---------------------------------------------------------------------------

type Graph = petgraph::stable_graph::StableGraph<SymbolId, (DependencyType, f64)>;

fn oracle_in_lists(graph: &Graph) -> (Vec<Vec<usize>>, Vec<usize>) {
    let bound = graph.node_bound();
    let mut in_neighbors = vec![Vec::new(); bound];
    let mut out_degree = vec![0; bound];
    for ni in graph.node_indices() {
        in_neighbors[ni.index()] = graph
            .edges_directed(ni, Direction::Incoming)
            .map(|e| e.source().index())
            .collect();
        out_degree[ni.index()] = graph.edges_directed(ni, Direction::Outgoing).count();
    }
    (in_neighbors, out_degree)
}

fn oracle_directed(graph: &Graph) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new(); graph.node_bound()];
    for ni in graph.node_indices() {
        out[ni.index()] = graph
            .edges_directed(ni, Direction::Outgoing)
            .map(|e| e.target().index())
            .collect();
    }
    out
}

fn oracle_out_neighbors(graph: &Graph) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new(); graph.node_bound()];
    for edge in graph.edge_references() {
        out[edge.source().index()].push(edge.target().index());
    }
    out
}

fn oracle_undirected(graph: &Graph) -> Vec<Vec<usize>> {
    let bound = graph.node_bound();
    let (in_neighbors, _) = oracle_in_lists(graph);
    let out_neighbors = oracle_out_neighbors(graph);
    let mut undirected: Vec<Vec<usize>> = vec![Vec::new(); bound];
    for (i, neighbors) in in_neighbors.iter().enumerate() {
        undirected[i].extend(neighbors.iter().copied());
    }
    for (i, neighbors) in out_neighbors.iter().enumerate() {
        for &j in neighbors {
            if !undirected[i].contains(&j) {
                undirected[i].push(j);
            }
        }
    }
    undirected
}

fn oracle_impact_radius(graph: &Graph, root: NodeIndex, max_depth: usize) -> Vec<SymbolId> {
    if max_depth == 0 {
        return Vec::new();
    }
    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut result = Vec::new();
    let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::from([(root, 0)]);
    while let Some((ni, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for edge in graph.edges_directed(ni, Direction::Incoming) {
            let pred = edge.source();
            if visited.insert(pred) {
                result.push(graph[pred].clone());
                queue.push_back((pred, depth + 1));
            }
        }
    }
    result
}

fn oracle_forward_reach(graph: &Graph, root: NodeIndex, max_depth: usize) -> Vec<SymbolId> {
    if max_depth == 0 {
        return Vec::new();
    }
    let mut visited: HashSet<NodeIndex> = HashSet::from([root]);
    let mut result = Vec::new();
    let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::from([(root, 0)]);
    while let Some((ni, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for edge in graph.edges_directed(ni, Direction::Outgoing) {
            let succ = edge.target();
            if visited.insert(succ) {
                result.push(graph[succ].clone());
                queue.push_back((succ, depth + 1));
            }
        }
    }
    result
}

fn oracle_subgraph(
    graph: &Graph,
    root: NodeIndex,
    direction: SubgraphDirection,
    max_depth: usize,
) -> SubgraphView {
    let mut nodes = vec![graph[root].clone()];
    let mut edges = Vec::new();
    let mut visited: HashSet<NodeIndex> = HashSet::from([root]);
    let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::from([(root, 0)]);
    let mut seen_edges: HashSet<(SymbolId, SymbolId, DependencyType)> = HashSet::new();
    while let Some((ni, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        let dirs: &[Direction] = match direction {
            SubgraphDirection::Outgoing => &[Direction::Outgoing],
            SubgraphDirection::Incoming => &[Direction::Incoming],
            SubgraphDirection::Both => &[Direction::Outgoing, Direction::Incoming],
        };
        for dir in dirs {
            for edge in graph.edges_directed(ni, *dir) {
                let other = if matches!(dir, Direction::Outgoing) {
                    edge.target()
                } else {
                    edge.source()
                };
                let (kind, confidence) = *edge.weight();
                let (source, target) = (graph[ni].clone(), graph[other].clone());
                if seen_edges.insert((source.clone(), target.clone(), kind)) {
                    edges.push(SubgraphEdge {
                        source,
                        target,
                        dependency_type: kind,
                        confidence,
                    });
                }
                if visited.insert(other) {
                    nodes.push(graph[other].clone());
                    queue.push_back((other, depth + 1));
                }
            }
        }
    }
    SubgraphView { nodes, edges }
}

// ---------------------------------------------------------------------------
// Graph builders.
// ---------------------------------------------------------------------------

fn symbol(name: &str, file: &str, line: u32) -> Symbol {
    Symbol::new(name, SymbolKind::Function, Location::new(file, line, 1))
}

/// A tiny deterministic generator: the suite takes no random dependency and
/// the same seed always builds the same graph.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

fn context_for(draw: usize) -> ExtractionContext {
    match draw % 4 {
        0 | 1 => ExtractionContext::DirectExtraction,
        2 => ExtractionContext::Heuristic {
            score: (draw % 97) as f64 / 97.0,
        },
        _ => ExtractionContext::Unresolved,
    }
}

/// A graph of `nodes` symbols, a third of them homonyms of another symbol in a
/// different file, and `edges` random edges that may repeat, loop and carry any
/// kind and confidence.
fn random_graph(seed: u64, nodes: usize, edges: usize) -> (CallGraph, Vec<SymbolId>) {
    let mut rng = Seeded(seed | 1);
    let mut graph = CallGraph::new();
    let mut ids = Vec::new();
    for n in 0..nodes {
        let homonym = n % 3 == 2 && n > 0;
        let name = if homonym {
            format!("f{}", n - 1)
        } else {
            format!("f{n}")
        };
        ids.push(graph.add_symbol(symbol(&name, &format!("file{n}.rs"), 1 + n as u32)));
    }
    for _ in 0..edges {
        if nodes == 0 {
            break;
        }
        let (source, target) = (rng.below(nodes), rng.below(nodes));
        let kind = KINDS[rng.below(KINDS.len())];
        let ctx = context_for(rng.below(1000));
        graph
            .add_dependency_with_provenance(&ids[source], &ids[target], kind, ctx)
            .expect("both endpoints exist");
    }
    (graph, ids)
}

/// The hand-written graph that holds every shape at once: a cycle, a self
/// loop, a parallel edge, homonyms, an isolated node and a dangling chain.
fn shapes_graph() -> (CallGraph, Vec<SymbolId>) {
    let mut graph = CallGraph::new();
    let names = [
        ("main", "a.rs"),
        ("run", "a.rs"),
        ("run", "b.rs"), // homonym of the previous one
        ("loop_a", "c.rs"),
        ("loop_b", "c.rs"),
        ("self_call", "d.rs"),
        ("isolated", "e.rs"),
        ("leaf", "f.rs"),
    ];
    let ids: Vec<SymbolId> = names
        .iter()
        .enumerate()
        .map(|(line, (name, file))| graph.add_symbol(symbol(name, file, 1 + line as u32)))
        .collect();
    let edge = |graph: &mut CallGraph, from: usize, to: usize, kind: DependencyType| {
        graph
            .add_dependency(&ids[from], &ids[to], kind)
            .expect("both endpoints exist");
    };
    edge(&mut graph, 0, 1, DependencyType::Calls);
    edge(&mut graph, 0, 2, DependencyType::Calls);
    edge(&mut graph, 0, 2, DependencyType::Imports); // parallel, other kind
    edge(&mut graph, 1, 3, DependencyType::Calls);
    edge(&mut graph, 2, 3, DependencyType::References);
    edge(&mut graph, 3, 4, DependencyType::Calls);
    edge(&mut graph, 4, 3, DependencyType::Calls); // cycle
    edge(&mut graph, 5, 5, DependencyType::Calls); // self loop
    edge(&mut graph, 4, 7, DependencyType::Contains);
    graph
        .add_dependency_with_provenance(
            &ids[1],
            &ids[7],
            DependencyType::Calls,
            ExtractionContext::Heuristic { score: 0.7 },
        )
        .expect("both endpoints exist");
    (graph, ids)
}

fn corpus() -> Vec<(String, CallGraph, Vec<SymbolId>)> {
    let mut graphs = vec![("empty".to_string(), CallGraph::new(), Vec::new())];
    let (single, ids) = random_graph(1, 1, 0);
    graphs.push(("single".into(), single, ids));
    let (looped, ids) = random_graph(2, 1, 3);
    graphs.push(("single node, loops".into(), looped, ids));
    let (shapes, ids) = shapes_graph();
    graphs.push(("shapes".into(), shapes, ids));
    for seed in 1..=24u64 {
        let nodes = 2 + (seed as usize * 3) % 29;
        let edges = nodes * (1 + (seed as usize % 5));
        let (graph, ids) = random_graph(seed * 7919, nodes, edges);
        graphs.push((format!("random seed {seed}"), graph, ids));
    }
    graphs
}

fn bits(scores: &HashMap<usize, f64>) -> Vec<(usize, u64)> {
    let mut all: Vec<(usize, u64)> = scores.iter().map(|(&n, &s)| (n, s.to_bits())).collect();
    all.sort_unstable();
    all
}

// ---------------------------------------------------------------------------
// Equivalence.
// ---------------------------------------------------------------------------

#[test]
fn the_adjacency_lists_are_the_petgraph_lists_in_the_same_order() {
    for (label, graph, _) in corpus() {
        let projection = CallGraphProjection::from_call_graph(&graph);
        let oracle = projection.graph();

        assert_eq!(
            projection.build_adjacency(),
            oracle_in_lists(oracle),
            "{label}: build_adjacency"
        );
        assert_eq!(
            projection.build_directed_adjacency(),
            oracle_directed(oracle),
            "{label}: build_directed_adjacency"
        );
        assert_eq!(
            projection.build_out_neighbors(),
            oracle_out_neighbors(oracle),
            "{label}: build_out_neighbors"
        );
        assert_eq!(
            projection.build_undirected_neighbors(),
            oracle_undirected(oracle),
            "{label}: build_undirected_neighbors"
        );
    }
}

#[test]
fn every_edge_keeps_its_kind_and_confidence_on_both_sides() {
    for (label, graph, _) in corpus() {
        let projection = CallGraphProjection::from_call_graph(&graph);
        let adjacency = projection.compact_adjacency();
        let oracle = projection.graph();
        assert_eq!(adjacency.node_count(), oracle.node_bound(), "{label}");
        assert_eq!(adjacency.edge_count(), oracle.edge_count(), "{label}");

        for ni in oracle.node_indices() {
            for (side, direction) in [
                (EdgeSide::Outgoing, Direction::Outgoing),
                (EdgeSide::Incoming, Direction::Incoming),
            ] {
                let expected: Vec<(usize, DependencyType, u64)> = oracle
                    .edges_directed(ni, direction)
                    .map(|e| {
                        let other = if side == EdgeSide::Outgoing {
                            e.target()
                        } else {
                            e.source()
                        };
                        (other.index(), e.weight().0, e.weight().1.to_bits())
                    })
                    .collect();
                let actual: Vec<(usize, DependencyType, u64)> = adjacency
                    .edges(ni.index(), side)
                    .map(|e| {
                        (
                            e.neighbour as usize,
                            e.dependency_type,
                            e.confidence.to_bits(),
                        )
                    })
                    .collect();
                assert_eq!(actual, expected, "{label}: {side:?} edges of {ni:?}");
            }
        }
    }
}

#[test]
fn the_edge_of_an_incoming_entry_is_the_edge_that_was_inserted() {
    // Two parallel edges of different kinds and confidences between the same
    // pair: the incoming side must pair each source with its own kind.
    let mut graph = CallGraph::new();
    let a = graph.add_symbol(symbol("a", "a.rs", 1));
    let b = graph.add_symbol(symbol("b", "b.rs", 1));
    graph
        .add_dependency_with_provenance(
            &a,
            &b,
            DependencyType::Calls,
            ExtractionContext::Heuristic { score: 0.6 },
        )
        .unwrap();
    graph
        .add_dependency(&a, &b, DependencyType::Imports)
        .unwrap();
    let projection = CallGraphProjection::from_call_graph(&graph);
    let adjacency = projection.compact_adjacency();
    let to = projection.id_to_index()[&b].index();
    let mut incoming: Vec<(DependencyType, u64)> = adjacency
        .edges(to, EdgeSide::Incoming)
        .map(|e| (e.dependency_type, e.confidence.to_bits()))
        .collect();
    incoming.sort();
    let mut expected: Vec<(DependencyType, u64)> = projection
        .graph()
        .edge_references()
        .map(|e| (e.weight().0, e.weight().1.to_bits()))
        .collect();
    expected.sort();
    assert_eq!(incoming, expected);
    assert_eq!(adjacency.in_degree(to), 2);
}

#[test]
fn reachability_matches_the_baseline_for_every_root_depth_and_pair() {
    for (label, graph, ids) in corpus() {
        let projection = CallGraphProjection::from_call_graph(&graph);
        let oracle = projection.graph();

        for id in &ids {
            let root = projection.id_to_index()[id];
            for depth in DEPTHS {
                assert_eq!(
                    projection.find_impact_radius(id, depth),
                    oracle_impact_radius(oracle, root, depth),
                    "{label}: impact radius of {id:?} at depth {depth}"
                );
                assert_eq!(
                    projection.find_forward_reach(id, depth),
                    oracle_forward_reach(oracle, root, depth),
                    "{label}: forward reach of {id:?} at depth {depth}"
                );
            }
            for other in &ids {
                let to = projection.id_to_index()[other];
                assert_eq!(
                    projection.has_path(id, other),
                    has_path_connecting(oracle, root, to, None),
                    "{label}: has_path {id:?} -> {other:?}"
                );
            }
        }
    }
}

#[test]
fn subgraph_views_match_the_baseline_in_every_direction() {
    for (label, graph, ids) in corpus() {
        let projection = CallGraphProjection::from_call_graph(&graph);
        let oracle = projection.graph();
        for id in &ids {
            let root = projection.id_to_index()[id];
            for direction in [
                SubgraphDirection::Outgoing,
                SubgraphDirection::Incoming,
                SubgraphDirection::Both,
            ] {
                for depth in DEPTHS {
                    assert_eq!(
                        projection.extract_subgraph(id, direction, depth),
                        oracle_subgraph(oracle, root, direction, depth),
                        "{label}: {direction:?} subgraph of {id:?} at depth {depth}"
                    );
                }
            }
        }
    }
}

#[test]
fn unknown_ids_answer_empty_like_before() {
    let (graph, ids) = shapes_graph();
    let projection = CallGraphProjection::from_call_graph(&graph);
    let unknown = SymbolId::new("nowhere.rs:ghost:1");
    assert!(projection.find_impact_radius(&unknown, 3).is_empty());
    assert!(projection.find_forward_reach(&unknown, 3).is_empty());
    assert!(!projection.has_path(&unknown, &ids[0]));
    assert!(!projection.has_path(&ids[0], &unknown));
    assert_eq!(
        projection.extract_subgraph(&unknown, SubgraphDirection::Both, 3),
        SubgraphView::default()
    );
}

#[test]
fn a_root_on_a_cycle_is_listed_among_its_callers_and_not_among_its_callees() {
    // Published behaviour of the two walks, kept as it was.
    let (graph, ids) = shapes_graph();
    let projection = CallGraphProjection::from_call_graph(&graph);
    let loop_a = &ids[3];
    assert!(projection.find_impact_radius(loop_a, 3).contains(loop_a));
    assert!(!projection.find_forward_reach(loop_a, 3).contains(loop_a));
}

#[test]
fn ranking_condensation_and_personalisation_are_bit_identical() {
    for (label, graph, _) in corpus() {
        let projection = CallGraphProjection::from_call_graph(&graph);
        let oracle = projection.graph();
        let n = oracle.node_bound();
        let (in_new, degree_new) = projection.build_adjacency();
        let (in_old, degree_old) = oracle_in_lists(oracle);

        assert_eq!(
            bits(&page_rank(&in_new, &degree_new, n, 0.85, 50)),
            bits(&page_rank(&in_old, &degree_old, n, 0.85, 50)),
            "{label}: page rank"
        );

        let personalisation: Vec<f64> =
            (0..n).map(|i| if i % 3 == 0 { 1.0 } else { 0.0 }).collect();
        for personalisation in [None, Some(personalisation.as_slice())] {
            assert_eq!(
                bits(&personalized_pagerank(
                    &in_new,
                    &degree_new,
                    n,
                    0.85,
                    50,
                    personalisation
                )),
                bits(&personalized_pagerank(
                    &in_old,
                    &degree_old,
                    n,
                    0.85,
                    50,
                    personalisation
                )),
                "{label}: personalised page rank"
            );
        }

        assert_eq!(
            condensation(&projection.build_directed_adjacency(), n),
            condensation(&oracle_directed(oracle), n),
            "{label}: condensation"
        );
    }
}

// ---------------------------------------------------------------------------
// Sharing, publication, change, cancellation.
// ---------------------------------------------------------------------------

#[test]
fn the_adjacency_is_built_once_and_lent_to_every_caller() {
    let (graph, ids) = shapes_graph();
    let projection = CallGraphProjection::from_call_graph(&graph);

    let first: *const CompactAdjacency = projection.compact_adjacency();
    // Queries that read the adjacency must not replace it.
    let _ = projection.find_impact_radius(&ids[3], 3);
    let _ = projection.build_adjacency();
    let _ = projection.extract_subgraph(&ids[0], SubgraphDirection::Both, 3);
    let second: *const CompactAdjacency = projection.compact_adjacency();
    assert!(
        std::ptr::eq(first, second),
        "a query rebuilt the adjacency of the projection"
    );
}

#[test]
fn a_published_projection_shares_one_adjacency_across_readers() {
    let (graph, ids) = shapes_graph();
    let published: Arc<dyn CallGraphProjectionPort> = PetgraphProjectionFactory.project(&graph);
    let expected = published.build_adjacency();

    let addresses: Vec<usize> = std::thread::scope(|scope| {
        let readers: Vec<_> = (0..8)
            .map(|_| {
                let projection = Arc::clone(&published);
                let expected = expected.clone();
                let root = ids[3].clone();
                scope.spawn(move || {
                    // Eight first callers race for the lazy build: all of
                    // them must see the same, complete adjacency.
                    let adjacency = projection.compact_adjacency();
                    assert_eq!(projection.build_adjacency(), expected);
                    assert!(!projection.find_forward_reach(&root, 3).is_empty());
                    adjacency as *const CompactAdjacency as usize
                })
            })
            .collect();
        readers.into_iter().map(|r| r.join().unwrap()).collect()
    });
    assert!(addresses.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn a_reader_that_stops_early_or_panics_leaves_the_shared_adjacency_intact() {
    let (graph, ids) = shapes_graph();
    let published: Arc<dyn CallGraphProjectionPort> = PetgraphProjectionFactory.project(&graph);
    let before = published.compact_adjacency().clone();

    // A reader that abandons its iteration half way (the way a cancelled
    // query does) borrows, so it has nothing to undo.
    let node = published.symbol_index()[&ids[0]].index();
    let mut edges = published
        .compact_adjacency()
        .edges(node, EdgeSide::Outgoing);
    let _ = edges.next();
    drop(edges);

    // A reader that panics while holding the borrow does not poison it.
    let crashing = Arc::clone(&published);
    let outcome = std::thread::spawn(move || {
        let _held = crashing.compact_adjacency();
        panic!("cancelled mid-query");
    })
    .join();
    assert!(outcome.is_err());

    assert_eq!(*published.compact_adjacency(), before);
    assert_eq!(
        published.build_adjacency(),
        CallGraphProjection::from_call_graph(&graph).build_adjacency()
    );
}

#[test]
fn a_change_to_the_graph_is_a_new_projection_and_the_published_one_does_not_move() {
    let (mut graph, ids) = shapes_graph();
    let published = CallGraphProjection::from_call_graph(&graph);
    let frozen = published.compact_adjacency().clone();
    let frozen_lists = published.build_adjacency();

    graph
        .add_dependency(&ids[6], &ids[0], DependencyType::Calls)
        .expect("both endpoints exist");
    let next = CallGraphProjection::from_call_graph(&graph);

    assert_eq!(*published.compact_adjacency(), frozen);
    assert_eq!(published.build_adjacency(), frozen_lists);
    assert_eq!(
        next.compact_adjacency().edge_count(),
        frozen.edge_count() + 1
    );
    assert_ne!(*next.compact_adjacency(), frozen);
    assert_eq!(
        next.build_adjacency(),
        oracle_in_lists(next.graph()),
        "the new projection answers for the new graph"
    );
}
