//! TASK-86 — the impact answer names the tests it reaches, and the command
//! that runs them.
//!
//! The question «what depends on this name?» is answered by one owner
//! (`AnalysisService::impact_of`), and every surface asks it the same way.
//! Until this unit the answer said *which* symbols depend on the name and
//! how risky the change is, and stopped there: the developer who changes a
//! symbol still had to guess which tests to run.
//!
//! This file pins three things:
//!
//! - a test that reaches the changed symbol is NAMED by the answer, with the
//!   file it lives in and the exact command that runs it;
//! - a dependent that is not a test is not named as one — over-naming is
//!   safe, lying is not;
//! - the reader is what decides: a Rust function marked `#[test]` is a test
//!   because the file says so, not because its name looks like one.
//!
//! The source is parsed, not hand-built: the marker the reader sees is the
//! only thing that can say what a test is, so the test exercises the reader.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::Symbol;
use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};
use cognicode_core::domain::value_objects::{DependencyType, Language};
use cognicode_core::infrastructure::parser::TreeSitterParser;

/// A Rust source with one production function and one test that calls it.
const SOURCE: &str = r#"
pub fn changed_symbol() -> u32 {
    41
}

#[test]
fn the_test_that_watches_the_change() {
    assert_eq!(changed_symbol(), 41);
}

pub fn helper_the_test_uses() -> u32 {
    1
}
"#;

/// The file the source lives in: an integration test of a package called
/// `demo`, in the layout cargo uses for a directory target.
const FILE: &str = "crates/demo/tests/it/checks.rs";

/// The symbols the reader finds in [`SOURCE`], parsed the way the graph
/// parses them.
fn read() -> Vec<Symbol> {
    let parser =
        TreeSitterParser::with_cache(Language::Rust).expect("the rust reader is available");
    parser
        .find_all_symbols_with_path(SOURCE, FILE)
        .expect("the source is rust")
}

/// The graph the question is asked on: the production symbol, the test that
/// calls it, and the helper the test uses.
fn graph() -> CallGraph {
    let mut graph = CallGraph::new();
    let mut ids: Vec<SymbolId> = Vec::new();

    for symbol in read() {
        ids.push(graph.add_symbol(symbol));
    }

    // The test calls the changed symbol; the helper is called by the test.
    let edges: Vec<(SymbolId, SymbolId)> = {
        let by_name = |name: &str| -> SymbolId {
            ids.iter()
                .find(|id| {
                    graph
                        .get_symbol(id)
                        .map(|s| s.name() == name)
                        .unwrap_or(false)
                })
                .cloned()
                .unwrap_or_else(|| panic!("{name} is in the source"))
        };
        vec![
            (
                by_name("the_test_that_watches_the_change"),
                by_name("changed_symbol"),
            ),
            (
                by_name("helper_the_test_uses"),
                by_name("the_test_that_watches_the_change"),
            ),
        ]
    };

    for (caller, callee) in edges {
        graph
            .add_dependency(&caller, &callee, DependencyType::Calls)
            .unwrap();
    }

    graph
}

/// The answer to the question, for the symbol the test watches.
fn impact() -> cognicode_core::application::services::impact_of::ImpactOf {
    AnalysisService::impact_of(&graph(), "changed_symbol").expect("the symbol is in the graph")
}

/// A test that reaches the changed symbol is named, with the command that
/// runs it.
#[test]
fn the_impact_names_the_test_that_reaches_the_symbol() {
    let impact = impact();

    let names: Vec<&str> = impact
        .affected_tests
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["the_test_that_watches_the_change"],
        "the answer names the test that reaches the change: {impact:?}"
    );

    let test = &impact.affected_tests[0];
    assert_eq!(
        test.file, FILE,
        "the answer says which file the test lives in"
    );
    assert!(
        test.command.is_some(),
        "the answer carries the command that runs the test: {test:?}"
    );
}

/// The command is exact: it names the package, the target and the test.
#[test]
fn the_command_runs_exactly_that_test() {
    let impact = impact();

    assert_eq!(
        impact.affected_tests[0].command.as_deref(),
        Some(
            "cargo nextest run --manifest-path crates/demo/Cargo.toml --test it \
             -E 'test(=checks::the_test_that_watches_the_change)'"
        ),
        "the command is the one that runs this test and no other"
    );
}

/// A dependent that is not a test is not named as one.
///
/// The helper is a production function the test uses. Naming it a test
/// would send the developer to run a function that is not a test.
#[test]
fn a_dependent_that_is_not_a_test_is_not_named_as_one() {
    let impact = impact();

    let names: Vec<&str> = impact
        .affected_tests
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        !names.contains(&"helper_the_test_uses"),
        "a helper is not a test: {names:?}"
    );
    assert!(
        !names.contains(&"changed_symbol"),
        "the changed symbol is not a test of itself: {names:?}"
    );
}

/// The answer is ordered: the same question gives the same order twice.
#[test]
fn the_named_tests_are_ordered() {
    let first: Vec<(String, u32)> = impact()
        .affected_tests
        .iter()
        .map(|t| (t.file.clone(), t.line))
        .collect();
    let second: Vec<(String, u32)> = impact()
        .affected_tests
        .iter()
        .map(|t| (t.file.clone(), t.line))
        .collect();

    assert_eq!(first, second, "the order of the answer is stable");
}
