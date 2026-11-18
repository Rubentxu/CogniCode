//! Behaviour of the typed read operators: what a query answers, what the
//! plan costs, and what the explain says about both.

use super::*;
use crate::domain::aggregates::Symbol;
use crate::domain::services::ExtractionContext;
use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

fn function(name: &str, file: &str, line: u32) -> Symbol {
    Symbol::new(name, SymbolKind::Function, Location::new(file, line, 0))
}

fn trait_named(name: &str, file: &str) -> Symbol {
    Symbol::new(name, SymbolKind::Trait, Location::new(file, 0, 0))
}

fn link(graph: &mut CallGraph, from: &SymbolId, to: &SymbolId, kind: DependencyType) {
    graph.add_dependency(from, to, kind).expect("edge");
}

fn names(graph: &CallGraph, answer: &Answer) -> Vec<String> {
    answer
        .rows
        .iter()
        .map(|row| {
            graph
                .get_symbol(&row.id)
                .expect("symbol")
                .name()
                .to_string()
        })
        .collect()
}

/// `Parse` is implemented in two modules and called from three; `ping` and
/// `pong` call each other; a second `helper` lives in another file.
struct World {
    graph: CallGraph,
    parse: SymbolId,
}

fn world() -> World {
    let mut graph = CallGraph::new();
    let parse = graph.add_symbol(trait_named("Parse", "src/core/parse.rs"));
    let json = graph.add_symbol(function("JsonParse", "src/core/json.rs", 0));
    let yaml = graph.add_symbol(function("YamlParse", "src/adapters/yaml.rs", 0));
    let a = graph.add_symbol(function("call_a", "src/core/a.rs", 0));
    let b = graph.add_symbol(function("call_b", "src/adapters/b.rs", 0));
    let c = graph.add_symbol(function("call_c", "src/adapters/c.rs", 0));
    let ping = graph.add_symbol(function("ping", "src/core/loop.rs", 0));
    let pong = graph.add_symbol(function("pong", "src/core/loop.rs", 10));
    graph.add_symbol(function("helper", "src/core/h1.rs", 0));
    graph.add_symbol(function("helper", "src/adapters/h2.rs", 0));
    link(&mut graph, &json, &parse, DependencyType::Inherits);
    link(&mut graph, &yaml, &parse, DependencyType::Inherits);
    for caller in [&a, &b, &c] {
        link(&mut graph, caller, &parse, DependencyType::Calls);
    }
    link(&mut graph, &ping, &pong, DependencyType::Calls);
    link(&mut graph, &pong, &ping, DependencyType::Calls);
    link(&mut graph, &a, &ping, DependencyType::Calls);
    World { graph, parse }
}

fn query(target: &str, relation: Relation) -> TypedQuery {
    TypedQuery::new(Target::Name(target.to_string()), vec![relation])
}

#[test]
fn implementors_filtered_by_module_run_lookup_relation_filter_and_top_k() {
    let World { graph, .. } = world();
    let q = query("Parse", Relation::Implementors)
        .in_module("src/core")
        .with_top_k(1)
        .with_budget(100);

    let answer = run(&graph, &q, None).expect("the query is valid");

    assert_eq!(names(&graph, &answer), ["JsonParse"]);
    let operators: Vec<_> = answer.explain.steps.iter().map(|s| s.operator).collect();
    assert_eq!(
        operators,
        [
            Operator::Lookup,
            Operator::Expand,
            Operator::Filter,
            Operator::TopK
        ]
    );
    assert_eq!(answer.explain.completeness, Completeness::Complete);
    assert!(answer.explain.spent_visits > 0);
    assert!(answer.explain.spent_visits <= answer.explain.budget.unwrap());
    assert!(!answer.explain.steps[1].index.is_empty());
}

#[test]
fn callers_filtered_by_module_keep_only_that_module() {
    let World { graph, .. } = world();
    let answer = run(
        &graph,
        &query("Parse", Relation::Callers).in_module("src/adapters"),
        None,
    )
    .expect("valid");

    let mut found = names(&graph, &answer);
    found.sort();
    assert_eq!(found, ["YamlParse", "call_b", "call_c"]);
    assert_eq!(answer.explain.matched, 3);
    assert_eq!(
        answer.explain.filtered_out, 2,
        "json and call_a are in core"
    );
}

#[test]
fn the_explain_reports_estimate_and_actual_spend() {
    let World { graph, .. } = world();
    let answer = run(&graph, &query("Parse", Relation::Callers), None).expect("valid");

    let explain = &answer.explain;
    assert_eq!(
        explain.estimated_visits, explain.spent_visits,
        "the first hop is estimated from the index, so the estimate is the spend"
    );
    assert_eq!(explain.estimate_basis, EstimateBasis::Structural);
}

#[test]
fn a_measured_cost_replaces_the_structural_estimate() {
    let World { graph, .. } = world();
    let estimator = CostEstimator::new();
    let q = query("Parse", Relation::Callers);

    let first = run(&graph, &q, Some(&estimator)).expect("valid");
    let second = run(&graph, &q, Some(&estimator)).expect("valid");

    assert_eq!(first.explain.estimate_basis, EstimateBasis::Structural);
    assert_eq!(second.explain.estimate_basis, EstimateBasis::Measured);
    assert_eq!(second.explain.estimated_visits, first.explain.spent_visits);
}

#[test]
fn a_budget_that_runs_out_gives_a_partial_answer_that_says_so() {
    let World { graph, .. } = world();
    let unbounded = run(&graph, &query("Parse", Relation::Callers), None).expect("valid");
    let bounded = run(
        &graph,
        &query("Parse", Relation::Callers).with_budget(4),
        None,
    )
    .expect("valid");

    assert!(bounded.rows.len() < unbounded.rows.len());
    assert!(
        bounded.explain.spent_visits <= 4,
        "the budget is never exceeded"
    );
    match &bounded.explain.completeness {
        Completeness::Partial { reasons } => {
            assert!(reasons.iter().any(|r| r.contains("budget")), "{reasons:?}");
        }
        Completeness::Complete => panic!("a stopped query must not claim to be complete"),
    }
    for row in &bounded.rows {
        assert!(
            unbounded.rows.iter().any(|r| r.id == row.id),
            "a partial answer holds rows of the full one and nothing else"
        );
    }
}

#[test]
fn a_cycle_terminates_and_lists_each_symbol_once() {
    let World { graph, .. } = world();
    let answer = run(
        &graph,
        &query("ping", Relation::Callees).with_depth(10),
        None,
    )
    .expect("valid");

    assert_eq!(
        names(&graph, &answer),
        ["pong"],
        "ping is the root, not a row"
    );
    assert_eq!(answer.explain.completeness, Completeness::Complete);
}

#[test]
fn a_cycle_walked_through_both_sides_terminates() {
    let World { graph, .. } = world();
    let answer = run(
        &graph,
        &TypedQuery::new(
            Target::Name("ping".into()),
            vec![Relation::Callers, Relation::Callees],
        )
        .with_depth(5),
        None,
    )
    .expect("valid");

    let found = names(&graph, &answer);
    assert!(found.contains(&"pong".to_string()));
    assert!(found.contains(&"call_a".to_string()));
    assert!(!found.contains(&"ping".to_string()));
}

#[test]
fn a_homonym_is_not_picked_for_the_reader() {
    let World { graph, .. } = world();
    let error = run(&graph, &query("helper", Relation::Callers), None).expect_err("two helpers");

    match error {
        QueryError::Graph(GraphQueryError::Ambiguous { candidates, .. }) => {
            assert_eq!(candidates.len(), 2);
        }
        other => panic!("expected an ambiguity, got {other:?}"),
    }
}

#[test]
fn an_id_target_names_the_symbol_a_homonym_could_not() {
    let World { graph, .. } = world();
    let helper = graph
        .find_by_name("helper")
        .into_iter()
        .next()
        .map(|s| SymbolId::new(s.fully_qualified_name()))
        .expect("helper");

    let answer = run(
        &graph,
        &TypedQuery::new(Target::Id(helper), vec![Relation::Callers]),
        None,
    )
    .expect("an id is not ambiguous");

    assert!(answer.rows.is_empty());
}

#[test]
fn an_unknown_name_is_not_found() {
    let World { graph, .. } = world();
    let error = run(&graph, &query("nowhere", Relation::Callers), None).expect_err("unknown");
    assert!(matches!(
        error,
        QueryError::Graph(GraphQueryError::NotFound { .. })
    ));
}

#[test]
fn an_invalid_query_is_a_typed_refusal_and_runs_nothing() {
    let World { graph, .. } = world();
    let cases = [
        (
            TypedQuery::new(Target::Name("Parse".into()), vec![]),
            InvalidQuery::NoRelation,
        ),
        (
            query("Parse", Relation::Callers).with_depth(0),
            InvalidQuery::ZeroDepth,
        ),
        (
            query("Parse", Relation::Callers).with_top_k(0),
            InvalidQuery::ZeroTopK,
        ),
        (
            query("Parse", Relation::Callers).with_budget(0),
            InvalidQuery::ZeroBudget,
        ),
        (
            query("Parse", Relation::Callers).in_module("  "),
            InvalidQuery::EmptyModule,
        ),
        (
            TypedQuery::new(Target::Name(" ".into()), vec![Relation::Callers]),
            InvalidQuery::EmptyTarget,
        ),
    ];
    for (q, expected) in cases {
        assert_eq!(
            run(&graph, &q, None).expect_err("invalid"),
            QueryError::Invalid(expected)
        );
    }
}

#[test]
fn a_relation_is_read_from_its_wire_name_or_refused() {
    assert_eq!(Relation::parse("implementors"), Ok(Relation::Implementors));
    assert_eq!(Relation::parse("callers"), Ok(Relation::Callers));
    assert_eq!(
        Relation::parse("MATCH (n) RETURN n"),
        Err(InvalidQuery::UnknownRelation(
            "MATCH (n) RETURN n".to_string()
        ))
    );
}

/// A graph where `Target` has one caller and thousands of unrelated edges.
fn wide_graph() -> (CallGraph, SymbolId) {
    let mut graph = CallGraph::new();
    let target = graph.add_symbol(function("target", "src/t.rs", 0));
    let caller = graph.add_symbol(function("only_caller", "src/c.rs", 0));
    link(&mut graph, &caller, &target, DependencyType::Calls);
    let mut previous = caller;
    for n in 0..300 {
        let next = graph.add_symbol(function(&format!("noise_{n}"), "src/n.rs", n));
        link(&mut graph, &previous, &next, DependencyType::Calls);
        previous = next;
    }
    (graph, target)
}

#[test]
fn the_selective_plan_equals_the_scan_and_visits_far_less() {
    let (graph, target) = wide_graph();
    let q = TypedQuery::new(Target::Id(target), vec![Relation::Callers]);

    let indexed = run_with_access(&graph, &q, Access::Index, None).expect("valid");
    let scanned = run_with_access(&graph, &q, Access::Scan, None).expect("valid");

    assert_eq!(indexed.rows, scanned.rows, "same answer by either route");
    assert!(
        indexed.explain.spent_visits * 50 < scanned.explain.spent_visits,
        "index {} vs scan {}",
        indexed.explain.spent_visits,
        scanned.explain.spent_visits
    );
}

#[test]
fn the_planner_takes_the_index_and_explains_the_route_it_did_not_take() {
    let (graph, target) = wide_graph();
    let answer = run(
        &graph,
        &TypedQuery::new(Target::Id(target), vec![Relation::Callers]),
        None,
    )
    .expect("valid");

    let expand = &answer.explain.steps[1];
    assert!(expand.index.contains("reverse"), "{}", expand.index);
    let rejected = expand.rejected.as_ref().expect("the scan was considered");
    assert!(rejected.contains("scan"), "{rejected}");
}

#[test]
fn a_scan_that_does_not_fit_the_budget_is_partial_where_the_index_is_whole() {
    let (graph, target) = wide_graph();
    let q = TypedQuery::new(Target::Id(target), vec![Relation::Callers]).with_budget(20);

    let indexed = run_with_access(&graph, &q, Access::Index, None).expect("valid");
    let scanned = run_with_access(&graph, &q, Access::Scan, None).expect("valid");

    assert_eq!(indexed.explain.completeness, Completeness::Complete);
    assert!(matches!(
        scanned.explain.completeness,
        Completeness::Partial { .. }
    ));
}

#[test]
fn equivalence_with_a_manual_composition_over_the_graph() {
    let World { graph, parse } = world();

    let mut manual: Vec<SymbolId> = graph
        .incoming_dependencies_with_metadata(&parse)
        .into_iter()
        .filter(|(_, kind, _, _)| **kind == DependencyType::Inherits)
        .map(|(source, _, _, _)| source.clone())
        .collect();
    manual.sort();

    let answer = run(&graph, &query("Parse", Relation::Implementors), None).expect("valid");
    let mut planned: Vec<SymbolId> = answer.rows.iter().map(|r| r.id.clone()).collect();
    planned.sort();

    assert_eq!(planned, manual);
}

#[test]
fn rows_carry_the_edge_the_graph_really_holds() {
    let mut graph = CallGraph::new();
    let target = graph.add_symbol(function("target", "src/t.rs", 0));
    let guess = graph.add_symbol(function("guess", "src/g.rs", 0));
    let sure = graph.add_symbol(function("sure", "src/s.rs", 0));
    link(&mut graph, &sure, &target, DependencyType::Calls);
    graph
        .add_dependency_with_provenance(
            &guess,
            &target,
            DependencyType::Calls,
            ExtractionContext::Heuristic { score: 0.6 },
        )
        .expect("edge");

    let answer = run(
        &graph,
        &TypedQuery::new(Target::Id(target), vec![Relation::Callers]).with_top_k(1),
        None,
    )
    .expect("valid");

    assert_eq!(
        names(&graph, &answer),
        ["sure"],
        "top-k ranks by confidence"
    );
    assert!(answer.rows[0].confidence > 0.6);
    assert_eq!(answer.explain.matched, 2);
    assert_eq!(answer.explain.listed, 1);
}

#[test]
fn tests_can_be_left_out_of_the_rows() {
    let mut graph = CallGraph::new();
    let target = graph.add_symbol(function("target", "src/t.rs", 0));
    let user = graph.add_symbol(function("user", "src/u.rs", 0));
    let test = graph.add_symbol(function("test_target", "tests/t.rs", 0).mark_as_test());
    link(&mut graph, &user, &target, DependencyType::Calls);
    link(&mut graph, &test, &target, DependencyType::Calls);

    let q = TypedQuery::new(Target::Id(target), vec![Relation::Dependents]).without_tests();
    let answer = run(&graph, &q, None).expect("valid");

    assert_eq!(names(&graph, &answer), ["user"]);
    assert_eq!(answer.explain.filtered_out, 1);
}
