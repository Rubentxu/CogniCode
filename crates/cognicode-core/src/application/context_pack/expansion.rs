//! The bounded walk of the graph around the seeds of a request.
//!
//! The walk is a breadth-first expansion over the relations the graph holds
//! and believes: it follows no edge the graph marks ambiguous or gives less
//! confidence than the policy asks, and it invents none. It is bounded in
//! the work it does —a number of symbols visited, and an [`Interrupt`] the
//! caller may trip— and it says how it ended:
//!
//! - `expanded`: every symbol within the depth was visited;
//! - `partial`: the visit budget or the deadline stopped it, and what was
//!   found is what the answer uses;
//! - `cancelled`: the caller stopped it;
//! - `unknown`: no relation the graph believes touches the seeds, so the
//!   graph says nothing about them, which is not the same as having no
//!   relations;
//! - `abstained`: the request did not need the walk, or the policy forbids
//!   it, and nothing was visited.
//!
//! A method is read together with the type that declares it: when a seed
//! sits inside a class, struct, trait or enum through a `Contains` relation
//! the graph holds, that type joins the walk at no cost of depth, so the
//! contract the type satisfies and the other types that satisfy it can be
//! reached. Nesting is followed only upwards and only for this; it is never
//! a relation between symbols that behave.
//!
//! Personalized PageRank runs over the local subgraph the walk found, with
//! the function of `cognicode_graph_algos`; there is no second implementation
//! of it here.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::policy::{Role, SelectionPolicy};
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::value_objects::{DependencyType, Provenance, SymbolKind};

/// The most symbols a walk visits when the request sets no other bound.
pub const DEFAULT_MAX_VISITS: usize = 2_000;

/// The damping factor of the PageRank: the usual one.
const PPR_ALPHA: f64 = 0.85;

/// The most iterations of the PageRank.
const PPR_ITERATIONS: usize = 50;

/// Why the work of a request was stopped from outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// The caller cancelled the request.
    Cancelled,
    /// The time the request was given has passed.
    DeadlineExceeded,
}

/// Something that can tell the walk to stop.
///
/// The walk asks it before every visit. The implementation is the edge's:
/// a deadline, a cancellation flag. The core holds no clock.
pub trait Interrupt: std::fmt::Debug + Send + Sync {
    /// Why the work must stop now, or `None` when it may go on.
    fn poll(&self) -> Option<Stop>;
}

/// The interrupt that never fires.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoInterrupt;

impl Interrupt for NoInterrupt {
    fn poll(&self) -> Option<Stop> {
        None
    }
}

/// The work a request may spend walking the graph.
#[derive(Debug, Clone)]
pub struct WorkBudget {
    /// The most symbols the walk visits.
    pub max_visits: usize,
    /// What stops the walk from outside.
    pub interrupt: Arc<dyn Interrupt>,
}

impl Default for WorkBudget {
    fn default() -> Self {
        Self {
            max_visits: DEFAULT_MAX_VISITS,
            interrupt: Arc::new(NoInterrupt),
        }
    }
}

/// How the walk ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExpansionStatus {
    /// Every symbol within the depth was visited. It does not say the
    /// selection is everything relevant.
    Expanded,
    /// The request did not need the walk, or the policy forbids it.
    Abstained,
    /// A bound stopped the walk before it finished.
    Partial,
    /// The caller cancelled the walk.
    Cancelled,
    /// The graph holds no relation it believes around the seeds.
    Unknown,
}

impl ExpansionStatus {
    /// The word the answer uses.
    pub fn code(self) -> &'static str {
        match self {
            ExpansionStatus::Expanded => "expanded",
            ExpansionStatus::Abstained => "abstained",
            ExpansionStatus::Partial => "partial",
            ExpansionStatus::Cancelled => "cancelled",
            ExpansionStatus::Unknown => "unknown",
        }
    }
}

/// What the walk did, in numbers and in words.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExpansionReport {
    /// How the walk ended.
    pub status: ExpansionStatus,
    /// Why, in words.
    pub reason: String,
    /// How many symbols the walk visited.
    pub visited: usize,
    /// The bound on visits the request set.
    pub max_visits: usize,
    /// How many seeds the walk started from.
    pub seeds: usize,
    /// How many relations the walk followed.
    pub edges_followed: usize,
    /// How many relations it did not believe and ignored: ambiguous, or
    /// below the confidence the policy asks.
    pub edges_ignored: usize,
    /// How many symbols had more neighbours than the walk follows.
    pub fanout_capped: usize,
}

impl ExpansionReport {
    /// The report of a walk that did not happen.
    pub fn abstained(reason: impl Into<String>, max_visits: usize, seeds: usize) -> Self {
        Self {
            status: ExpansionStatus::Abstained,
            reason: reason.into(),
            visited: 0,
            max_visits,
            seeds,
            edges_followed: 0,
            edges_ignored: 0,
            fanout_capped: 0,
        }
    }
}

/// What the walk found around the seeds.
#[derive(Debug, Clone)]
pub struct Neighbourhood {
    /// The symbols reached, seeds first, then in discovery order.
    pub nodes: Vec<SymbolId>,
    /// The hops from the nearest seed, by symbol.
    pub hops: HashMap<SymbolId, u32>,
    /// The seed each symbol was first reached from.
    pub from_seed: HashMap<SymbolId, SymbolId>,
    /// The relations followed, as `source -> target`, once each. A
    /// `Contains` relation is here only where a method was read with the type
    /// that declares it.
    pub edges: Vec<(SymbolId, SymbolId, DependencyType)>,
    /// How the walk ended.
    pub report: ExpansionReport,
}

/// The relations that describe how code behaves, not how it is nested.
fn is_behavioural(dependency: DependencyType) -> bool {
    !matches!(
        dependency,
        DependencyType::Contains | DependencyType::Defines
    )
}

/// The class, struct, trait or enum that declares `id` through a `Contains`
/// relation, when `id` is not itself a type.
fn declaring_type(graph: &CallGraph, id: &SymbolId) -> Option<SymbolId> {
    if graph
        .get_symbol(id)
        .is_none_or(|symbol| symbol.kind().is_type_definition())
    {
        return None;
    }
    graph.callers(id).into_iter().find(|caller| {
        graph.get_symbol(caller).is_some_and(|symbol| {
            matches!(
                symbol.kind(),
                SymbolKind::Class
                    | SymbolKind::Struct
                    | SymbolKind::Trait
                    | SymbolKind::Interface
                    | SymbolKind::Enum
            )
        }) && graph
            .dependencies(caller)
            .any(|(target, dependency)| target == id && *dependency == DependencyType::Contains)
    })
}

/// Whether the policy believes an edge of this provenance and confidence.
fn is_reliable(policy: &SelectionPolicy, provenance: Provenance, confidence: f64) -> bool {
    provenance != Provenance::Ambiguous && confidence >= policy.min_edge_confidence
}

/// Walks the graph from `seeds`, breadth first, within `budget`.
///
/// The seeds are the symbols the request is about, in a stable order; the
/// result is the same for the same graph, seeds, policy and visit budget.
/// Only a deadline can make two runs differ, and then the report says
/// `partial`.
pub fn walk(
    graph: &CallGraph,
    seeds: &[SymbolId],
    policy: &SelectionPolicy,
    budget: &WorkBudget,
) -> Neighbourhood {
    let mut found = Neighbourhood {
        nodes: Vec::new(),
        hops: HashMap::new(),
        from_seed: HashMap::new(),
        edges: Vec::new(),
        report: ExpansionReport::abstained("", budget.max_visits, seeds.len()),
    };
    let mut queue: VecDeque<SymbolId> = VecDeque::new();
    // The depth a symbol was reached at: its hops, except that the type of a
    // method costs no depth.
    let mut reach: HashMap<SymbolId, u32> = HashMap::new();
    for seed in seeds {
        if found.hops.insert(seed.clone(), 0).is_none() {
            reach.insert(seed.clone(), 0);
            found.nodes.push(seed.clone());
            found.from_seed.insert(seed.clone(), seed.clone());
            queue.push_back(seed.clone());
        }
    }
    let mut seen_edges: HashSet<(SymbolId, SymbolId, DependencyType)> = HashSet::new();
    let mut stopped: Option<(ExpansionStatus, String)> = None;
    let mut ignored = 0usize;
    let mut capped = 0usize;
    let mut visited = 0usize;

    while let Some(node) = queue.pop_front() {
        if let Some(stop) = budget.interrupt.poll() {
            stopped = Some(match stop {
                Stop::Cancelled => (
                    ExpansionStatus::Cancelled,
                    "the caller cancelled the walk".to_string(),
                ),
                Stop::DeadlineExceeded => (
                    ExpansionStatus::Partial,
                    "the time budget ran out before the walk finished".to_string(),
                ),
            });
            break;
        }
        if visited >= budget.max_visits {
            stopped = Some((
                ExpansionStatus::Partial,
                format!(
                    "the visit budget of {} symbol(s) ran out before the walk finished",
                    budget.max_visits
                ),
            ));
            break;
        }
        visited += 1;
        let hops = found.hops[&node];
        let depth = reach[&node];
        if depth >= policy.depth {
            continue;
        }
        let seed = found.from_seed[&node].clone();
        if let Some(container) = declaring_type(graph, &node)
            && !found.hops.contains_key(&container)
        {
            found
                .edges
                .push((container.clone(), node.clone(), DependencyType::Contains));
            found.hops.insert(container.clone(), hops + 1);
            reach.insert(container.clone(), depth);
            found.nodes.push(container.clone());
            found.from_seed.insert(container.clone(), seed.clone());
            queue.push_back(container);
        }
        let mut neighbours = neighbours_of(graph, &node, policy, &mut ignored);
        if neighbours.len() > policy.max_fanout {
            capped += 1;
            neighbours.truncate(policy.max_fanout);
        }
        for (source, target, dependency) in neighbours {
            let other = if source == node {
                target.clone()
            } else {
                source.clone()
            };
            if seen_edges.insert((source.clone(), target.clone(), dependency)) {
                found.edges.push((source, target, dependency));
            }
            if !found.hops.contains_key(&other) {
                found.hops.insert(other.clone(), hops + 1);
                reach.insert(other.clone(), depth + 1);
                found.nodes.push(other.clone());
                found.from_seed.insert(other.clone(), seed.clone());
                queue.push_back(other);
            }
        }
    }

    let behavioural = found
        .edges
        .iter()
        .filter(|(_, _, dependency)| is_behavioural(*dependency))
        .count();
    found.report = ExpansionReport {
        status: ExpansionStatus::Expanded,
        reason: format!("walked {} hop(s) from the seeds", policy.depth),
        visited,
        max_visits: budget.max_visits,
        seeds: seeds.len(),
        edges_followed: behavioural,
        edges_ignored: ignored,
        fanout_capped: capped,
    };
    match stopped {
        Some((status, reason)) => {
            found.report.status = status;
            found.report.reason = reason;
        }
        None if behavioural == 0 => {
            found.report.status = ExpansionStatus::Unknown;
            found.report.reason = if ignored > 0 {
                format!(
                    "the {ignored} relation(s) around the seeds are ambiguous or below the \
                     confidence the policy asks, so the graph says nothing reliable about them"
                )
            } else {
                "the graph holds no relation around the seeds: it says nothing about them, \
                 which is not the same as having none"
                    .to_string()
            };
        }
        None => {}
    }
    found
}

/// The believed behavioural relations that touch `node`, callees first,
/// then callers, each side in the graph's canonical order.
fn neighbours_of(
    graph: &CallGraph,
    node: &SymbolId,
    policy: &SelectionPolicy,
    ignored: &mut usize,
) -> Vec<(SymbolId, SymbolId, DependencyType)> {
    let mut out = Vec::new();
    for (target, dependency, provenance, confidence) in graph.dependencies_with_metadata(node) {
        if !is_behavioural(*dependency) {
            continue;
        }
        if is_reliable(policy, provenance, confidence) {
            out.push((node.clone(), target.clone(), *dependency));
        } else {
            *ignored += 1;
        }
    }
    for caller in graph.callers(node) {
        for (target, dependency, provenance, confidence) in
            graph.dependencies_with_metadata(&caller)
        {
            if target != node || !is_behavioural(*dependency) {
                continue;
            }
            if is_reliable(policy, provenance, confidence) {
                out.push((caller.clone(), node.clone(), *dependency));
            } else {
                *ignored += 1;
            }
        }
    }
    out
}

/// Personalized PageRank over the local subgraph the walk found.
///
/// The relations are read in both directions: context flows to what a seed
/// uses and to what uses it. The teleport distribution is `seed_weights`;
/// symbols outside the neighbourhood get no score.
pub fn local_ppr(
    neighbourhood: &Neighbourhood,
    seed_weights: &HashMap<SymbolId, f64>,
) -> HashMap<SymbolId, f64> {
    let index: HashMap<&SymbolId, usize> = neighbourhood
        .nodes
        .iter()
        .enumerate()
        .map(|(at, id)| (id, at))
        .collect();
    let n = neighbourhood.nodes.len();
    let mut in_neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut out_degree = vec![0usize; n];
    let mut pairs: HashSet<(usize, usize)> = HashSet::new();
    for (source, target, _) in &neighbourhood.edges {
        let (Some(&a), Some(&b)) = (index.get(source), index.get(target)) else {
            continue;
        };
        for (from, to) in [(a, b), (b, a)] {
            if from != to && pairs.insert((from, to)) {
                in_neighbors[to].push(from);
                out_degree[from] += 1;
            }
        }
    }
    let personalization: Vec<f64> = neighbourhood
        .nodes
        .iter()
        .map(|id| seed_weights.get(id).copied().unwrap_or(0.0))
        .collect();
    let scores = cognicode_graph_algos::personalized_pagerank(
        &in_neighbors,
        &out_degree,
        n,
        PPR_ALPHA,
        PPR_ITERATIONS,
        Some(&personalization),
    );
    neighbourhood
        .nodes
        .iter()
        .enumerate()
        .map(|(at, id)| (id.clone(), scores.get(&at).copied().unwrap_or(0.0)))
        .collect()
}

/// The part `id` plays around the seeds, from the relations the walk found.
pub fn role_of(
    graph: &CallGraph,
    id: &SymbolId,
    seeds: &HashSet<SymbolId>,
    edges: &[(SymbolId, SymbolId, DependencyType)],
) -> Role {
    if seeds.contains(id) {
        return Role::Seed;
    }
    let Some(symbol) = graph.get_symbol(id) else {
        return Role::Related;
    };
    // A test or a contract is that part of the seeds only when a relation the
    // walk found says it touches them: by its kind alone it is nobody's.
    let touched = edges
        .iter()
        .any(|(source, target, _)| source == id || target == id);
    if !touched {
        return Role::Related;
    }
    if symbol.is_test() {
        return Role::Test;
    }
    if matches!(symbol.kind(), SymbolKind::Trait | SymbolKind::Interface) {
        return Role::Interface;
    }
    let is_contract = |other: &SymbolId| {
        seeds.contains(other)
            || graph
                .get_symbol(other)
                .is_some_and(|s| matches!(s.kind(), SymbolKind::Trait | SymbolKind::Interface))
    };
    let mut role = Role::Related;
    for (source, target, dependency) in edges {
        if !is_behavioural(*dependency) {
            continue;
        }
        let (this_is_source, other) = if source == id {
            (true, target)
        } else if target == id {
            (false, source)
        } else {
            continue;
        };
        if *dependency == DependencyType::Inherits && is_contract(other) {
            return Role::Implementation;
        }
        if seeds.contains(other) {
            let found = if this_is_source {
                Role::Consumer
            } else {
                Role::Dependency
            };
            if role == Role::Related || found == Role::Consumer {
                role = found;
            }
        }
    }
    role
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::context_pack::policy::Purpose;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::services::ExtractionContext;
    use crate::domain::value_objects::Location;

    fn symbol(graph: &mut CallGraph, name: &str, line: u32) -> SymbolId {
        graph.add_symbol(Symbol::new(
            name,
            SymbolKind::Function,
            Location::new("src/lib.rs", line, 0),
        ))
    }

    /// `a -> b -> c -> d -> e`, a chain.
    fn chain() -> (CallGraph, Vec<SymbolId>) {
        let mut graph = CallGraph::new();
        let ids: Vec<SymbolId> = ["a", "b", "c", "d", "e"]
            .iter()
            .enumerate()
            .map(|(line, name)| symbol(&mut graph, name, line as u32))
            .collect();
        for pair in ids.windows(2) {
            graph
                .add_dependency(&pair[0], &pair[1], DependencyType::Calls)
                .unwrap();
        }
        (graph, ids)
    }

    fn policy() -> SelectionPolicy {
        SelectionPolicy::for_mode(Purpose::Diagnose)
    }

    #[test]
    fn the_walk_stops_at_the_depth_of_the_policy() {
        let (graph, ids) = chain();

        let found = walk(&graph, &ids[..1], &policy(), &WorkBudget::default());

        assert_eq!(found.report.status, ExpansionStatus::Expanded);
        assert_eq!(found.nodes, ids[..3].to_vec());
        assert_eq!(found.hops[&ids[2]], 2);
    }

    #[test]
    fn a_visit_budget_makes_the_walk_partial_and_says_so() {
        let (graph, ids) = chain();
        let budget = WorkBudget {
            max_visits: 1,
            ..WorkBudget::default()
        };

        let found = walk(&graph, &ids[..1], &policy(), &budget);

        assert_eq!(found.report.status, ExpansionStatus::Partial);
        assert_eq!(found.report.visited, 1);
        assert!(found.report.reason.contains("visit budget"));
    }

    #[derive(Debug)]
    struct StopAfter {
        polls: std::sync::atomic::AtomicUsize,
        allowed: usize,
        stop: Stop,
    }

    impl Interrupt for StopAfter {
        fn poll(&self) -> Option<Stop> {
            let seen = self.polls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            (seen >= self.allowed).then_some(self.stop)
        }
    }

    fn stopping(allowed: usize, stop: Stop) -> WorkBudget {
        WorkBudget {
            max_visits: DEFAULT_MAX_VISITS,
            interrupt: Arc::new(StopAfter {
                polls: 0.into(),
                allowed,
                stop,
            }),
        }
    }

    #[test]
    fn a_cancellation_is_not_a_partial_walk() {
        let (graph, ids) = chain();

        let cancelled = walk(&graph, &ids[..1], &policy(), &stopping(1, Stop::Cancelled));
        let late = walk(
            &graph,
            &ids[..1],
            &policy(),
            &stopping(1, Stop::DeadlineExceeded),
        );

        assert_eq!(cancelled.report.status, ExpansionStatus::Cancelled);
        assert_eq!(late.report.status, ExpansionStatus::Partial);
        assert!(late.report.reason.contains("time budget"));
    }

    #[test]
    fn seeds_without_a_believed_relation_are_unknown_and_nothing_is_invented() {
        let mut graph = CallGraph::new();
        let alone = symbol(&mut graph, "alone", 0);
        let other = symbol(&mut graph, "other", 1);
        graph
            .add_dependency_with_provenance(
                &alone,
                &other,
                DependencyType::Calls,
                ExtractionContext::Unresolved,
            )
            .unwrap();

        let found = walk(
            &graph,
            std::slice::from_ref(&alone),
            &policy(),
            &WorkBudget::default(),
        );

        assert_eq!(found.report.status, ExpansionStatus::Unknown);
        assert_eq!(found.report.edges_ignored, 1);
        assert!(found.edges.is_empty());
        assert_eq!(found.nodes, vec![alone]);
    }

    #[test]
    fn the_ppr_scores_what_is_close_to_the_seed_above_what_is_far() {
        let (graph, ids) = chain();
        let found = walk(&graph, &ids[..1], &policy(), &WorkBudget::default());
        let weights = HashMap::from([(ids[0].clone(), 1.0)]);

        let ppr = local_ppr(&found, &weights);

        assert!(ppr[&ids[0]] > ppr[&ids[2]]);
        assert!(ppr[&ids[1]] > ppr[&ids[2]]);
    }

    /// `Storage` (trait) <- `File` and `Memory` (structs); `File` contains
    /// `save`.
    fn contract() -> (CallGraph, [SymbolId; 4]) {
        let mut graph = CallGraph::new();
        let mut add = |name: &str, kind: SymbolKind, line: u32| {
            graph.add_symbol(Symbol::new(
                name,
                kind,
                Location::new("src/lib.rs", line, 0),
            ))
        };
        let storage = add("Storage", SymbolKind::Trait, 0);
        let file = add("File", SymbolKind::Struct, 1);
        let memory = add("Memory", SymbolKind::Struct, 2);
        let save = add("save", SymbolKind::Method, 3);
        for (from, to, dependency) in [
            (&file, &storage, DependencyType::Inherits),
            (&memory, &storage, DependencyType::Inherits),
            (&file, &save, DependencyType::Contains),
        ] {
            graph.add_dependency(from, to, dependency).unwrap();
        }
        (graph, [storage, file, memory, save])
    }

    #[test]
    fn a_method_is_read_with_its_type_so_the_contract_and_its_implementors_are_reached() {
        let (graph, [storage, file, memory, save]) = contract();

        let found = walk(
            &graph,
            std::slice::from_ref(&save),
            &policy(),
            &WorkBudget::default(),
        );

        assert_eq!(found.report.status, ExpansionStatus::Expanded);
        for reached in [&file, &storage, &memory] {
            assert!(found.hops.contains_key(reached));
        }
        let seeds: HashSet<SymbolId> = [save].into_iter().collect();
        assert_eq!(
            role_of(&graph, &memory, &seeds, &found.edges),
            Role::Implementation
        );
        assert_eq!(
            role_of(&graph, &storage, &seeds, &found.edges),
            Role::Interface
        );
        assert_eq!(
            role_of(&graph, &file, &seeds, &found.edges),
            Role::Implementation
        );
    }

    #[test]
    fn a_symbol_keeps_the_fewest_hops_it_was_reached_at() {
        let (mut graph, [_, file, _, save]) = contract();
        let user = symbol(&mut graph, "user", 9);
        graph
            .add_dependency(&user, &save, DependencyType::Calls)
            .unwrap();
        graph
            .add_dependency(&user, &file, DependencyType::References)
            .unwrap();

        let found = walk(
            &graph,
            std::slice::from_ref(&save),
            &policy(),
            &WorkBudget::default(),
        );

        assert_eq!(found.hops[&user], 1);
        assert_eq!(found.hops[&save], 0);
    }

    #[test]
    fn nesting_alone_is_not_a_relation_the_graph_believes() {
        let mut graph = CallGraph::new();
        let owner = graph.add_symbol(Symbol::new(
            "Owner",
            SymbolKind::Struct,
            Location::new("src/lib.rs", 0, 0),
        ));
        let method = graph.add_symbol(Symbol::new(
            "run",
            SymbolKind::Method,
            Location::new("src/lib.rs", 1, 0),
        ));
        graph
            .add_dependency(&owner, &method, DependencyType::Contains)
            .unwrap();

        let found = walk(&graph, &[method], &policy(), &WorkBudget::default());

        assert_eq!(found.report.status, ExpansionStatus::Unknown);
        assert_eq!(found.report.edges_followed, 0);
    }

    #[test]
    fn roles_come_from_the_relations_the_walk_found() {
        let mut graph = CallGraph::new();
        let seed = symbol(&mut graph, "save", 0);
        let consumer = symbol(&mut graph, "persist", 1);
        let dependency = symbol(&mut graph, "encode", 2);
        let edges = vec![
            (consumer.clone(), seed.clone(), DependencyType::Calls),
            (seed.clone(), dependency.clone(), DependencyType::Calls),
        ];
        let seeds = HashSet::from([seed.clone()]);

        assert_eq!(role_of(&graph, &seed, &seeds, &edges), Role::Seed);
        assert_eq!(role_of(&graph, &consumer, &seeds, &edges), Role::Consumer);
        assert_eq!(
            role_of(&graph, &dependency, &seeds, &edges),
            Role::Dependency
        );
    }

    #[test]
    fn a_trait_or_a_test_no_relation_touches_is_nobodys_contract_or_test() {
        let mut graph = CallGraph::new();
        let seed = symbol(&mut graph, "save", 0);
        let contract = graph.add_symbol(Symbol::new(
            "Storage",
            SymbolKind::Trait,
            Location::new("src/lib.rs", 1, 0),
        ));
        let test = graph.add_symbol(
            Symbol::new(
                "a_test",
                SymbolKind::Function,
                Location::new("src/lib.rs", 2, 0),
            )
            .mark_as_test(),
        );
        let seeds = HashSet::from([seed.clone()]);

        assert_eq!(role_of(&graph, &contract, &seeds, &[]), Role::Related);
        assert_eq!(role_of(&graph, &test, &seeds, &[]), Role::Related);

        let edges = vec![
            (seed.clone(), contract.clone(), DependencyType::Inherits),
            (test.clone(), seed.clone(), DependencyType::Calls),
        ];
        assert_eq!(role_of(&graph, &contract, &seeds, &edges), Role::Interface);
        assert_eq!(role_of(&graph, &test, &seeds, &edges), Role::Test);
    }
}
