//! TASK-77 — one owner answers the impact question, and the copies that had
//! no caller stay gone.
//!
//! Three surfaces put the same question — «what depends on this name?» — and
//! they have to give the same answer: the MCP tool, the CLI command and the
//! workspace session. Before this unit they did not. The CLI matched by
//! substring, so `foo` also matched `foo_bar` and reported its dependents as
//! dependents of `foo`; the MCP matched exactly; and the session answered an
//! empty Low impact for a name the graph did not hold. Three copies, three
//! answers.
//!
//! `AnalysisService::impact_of` is the owner. This file pins three things:
//!
//! - the three surfaces call the owner;
//! - the copies that had no production caller are gone, and do not come back;
//! - the service that stayed alive answers a DIFFERENT question, so that
//!   nobody reads it as a second copy of this one.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::application::services::impact_analysis::ImpactAnalysisService;
use cognicode_core::domain::aggregates::Symbol;
use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::graph::project_call_graph;

use crate::source_scan::{declares_word, rust_sources};

/// The three surfaces that ask the impact question.
const SURFACES: [&str; 3] = [
    "crates/cognicode-core/src/interface/mcp/handlers/mod.rs",
    "crates/cognicode-core/src/interface/cli/commands.rs",
    "crates/cognicode-core/src/application/workspace_session.rs",
];

/// The copies of the impact question that had no production caller.
///
/// The tokens are assembled at runtime: written out whole they would be
/// literals in this very file, and the scan would find the scan.
///
/// They are matched as whole words, so `AnalyzeImpactResult` — a
/// different type, and one that lives — is not one of them.
fn dead_copies() -> Vec<String> {
    [
        ["is_safe", "to_change"].join("_"),
        ["build", "impact", "result"].join("_"),
        ["Impact", "Result"].join(""),
        ["Impact", "Path"].join(""),
        ["Impact", "Edge"].join(""),
        ["Impact", "Limits"].join(""),
        ["Impact", "Analyzer"].join(""),
        ["Impact", "Level"].join(""),
        ["Impact", "Report"].join(""),
        ["Impact", "Threshold"].join(""),
    ]
    .into_iter()
    .collect()
}

/// The three surfaces reach the owner, not a copy of it.
///
/// The behavioural proof is the CLI↔MCP equivalence (in
/// `cognicode-cli/tests/graph_impact_equivalence.rs`); this is the
/// structural half: the third surface, the workspace session, is the one
/// that had no test of its own, and a surface that quietly grows its own
/// resolution is how this duplication started.
#[test]
fn the_three_surfaces_call_the_one_owner() {
    let sources = rust_sources();

    for surface in SURFACES {
        let content = sources
            .iter()
            .find(|(path, _)| path == surface)
            .map(|(_, content)| content.as_str())
            .unwrap_or_else(|| panic!("{surface} is part of the tree"));
        assert!(
            declares_word(content, "impact_of"),
            "{surface} does not call the owner (`AnalysisService::impact_of`); \
             a surface with its own resolution is a second answer to the same \
             question"
        );
    }
}

/// The copies with no production caller are gone, and they do not come back.
///
/// The task named three dead copies. Measurement on today's tree found
/// two of them dead — the `AnalysisService::analyze_impact` wrapper with
/// its `is_safe_to_change`, and the B4.W1 impact payload with the types
/// only it produced — and one alive: `ImpactAnalysisService`, which
/// `cognicode-explorer` consumes and which answers another question; the
/// test below pins that difference.
///
/// A fourth copy, which the task did not name, turned out to be dead as
/// well: the domain service that answered the impact question with a
/// five-bucket scale of its own, with no production caller anywhere.
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
    let ghost = ["an", "impact", "question", "nobody", "asks"].join("_");
    assert!(
        !sources
            .iter()
            .any(|(_, content)| declares_word(content, &ghost)),
        "a name no file declares must not be reported as present"
    );

    let owner = sources
        .iter()
        .filter(|(_, content)| declares_word(content, "impact_of"))
        .count();
    assert!(
        owner >= 3,
        "the owner is called from at least three places; the scan found {owner}"
    );
}

/// A chain of two hops: `leaf` calls `middle`, `middle` calls `target`.
fn chain() -> (CallGraph, SymbolId) {
    let mut graph = CallGraph::new();
    let function =
        |name: &str| Symbol::new(name, SymbolKind::Function, Location::new("a.rs", 1, 1));

    let target = graph.add_symbol(function("target"));
    let middle = graph.add_symbol(function("middle"));
    let leaf = graph.add_symbol(function("leaf"));
    graph
        .add_dependency(&middle, &target, DependencyType::Calls)
        .unwrap();
    graph
        .add_dependency(&leaf, &middle, DependencyType::Calls)
        .unwrap();

    (graph, target)
}

/// The service that stayed alive answers another question.
///
/// `ImpactAnalysisService` is a graph-traversal API: it walks a projection,
/// it is bounded by a depth, it resolves by `SymbolId`, and a root it does
/// not hold yields an empty answer. The owner resolves by NAME, is not
/// bounded, derives a risk, and REFUSES a name the graph does not hold.
/// Those are different questions, and the same graph proves it.
#[test]
fn the_service_that_stays_answers_another_question() {
    let (graph, target) = chain();

    // The owner: unbounded, by name, with a risk.
    let owned = AnalysisService::impact_of(&graph, "target").expect("target is in the graph");
    let mut owned_names: Vec<String> = owned.symbols.clone();
    owned_names.sort();
    assert_eq!(
        owned_names,
        vec!["leaf".to_string(), "middle".to_string()],
        "the owner answers with every transitive dependent: {owned:?}"
    );

    // The service that stayed: bounded by depth.
    let projection = project_call_graph(&graph);
    let service = ImpactAnalysisService::new();
    let one_hop: Vec<String> = service
        .impact_radius(projection.as_ref(), &target, 1)
        .iter()
        .map(|id| graph.get_symbol(id).map(|s| s.name().to_string()))
        .collect::<Option<Vec<String>>>()
        .expect("every id is a symbol of the graph");
    assert_eq!(
        one_hop,
        vec!["middle".to_string()],
        "the radius is bounded by depth: one hop reaches the direct caller only"
    );

    // The two differ on the same graph and the same root: a bound is a
    // question of its own, and that is why this service is not a second copy
    // of the owner.
    assert!(
        !one_hop.contains(&"leaf".to_string()) && owned_names.contains(&"leaf".to_string()),
        "the radius is bounded and the owner is not; they answer different \
         questions: {one_hop:?} / {owned_names:?}"
    );
}

/// A name the graph does not hold is refused by the owner and ignored by the
/// service.
///
/// This is the sharpest difference between the two, and it is the one that
/// made the session wrong: it used to answer an empty Low impact for a
/// symbol nobody has.
#[test]
fn an_absent_name_is_refused_by_the_owner_and_empty_by_the_service() {
    let (graph, _) = chain();

    let refused = AnalysisService::impact_of(&graph, "nobody");
    assert!(
        refused.is_err(),
        "the owner refuses a name the graph does not hold: {refused:?}"
    );

    let projection = project_call_graph(&graph);
    let service = ImpactAnalysisService::new();
    let absent = SymbolId::new("nobody");
    assert!(
        service
            .impact_radius(projection.as_ref(), &absent, 1)
            .is_empty(),
        "the service answers empty for a root it does not hold; that is its \
         documented behaviour and it is not the owner's"
    );
}
