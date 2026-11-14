//! A name that differs only in the case is two names, not one.
//!
//! ## The defect
//!
//! The symbol index keyed its declarations by the lowercased name, so the
//! fold was the index's property and not the language's. In a language that
//! tells `Describe` from `describe` —Rust, where the capital of a type and
//! the lower case of a function are meaning— the two were one key. A call
//! to one of them found the other, and the graph published the call as
//! ambiguous: a hole where an edge was, and a fan-in that does not count
//! the calls the code really makes.
//!
//! The pattern is not rare: `Config` and `config()`, `Builder` and
//! `builder()`, `Point` and `point()` are the ordinary way of writing a
//! type and its constructor.
//!
//! ## What is measured
//!
//! - The project route ties the call to the declaration spelled the same
//!   way.
//! - The per-file route says the same thing.
//! - A language that does not tell the case apart keeps folding: the fold
//!   is the language's property, not the index's.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::value_objects::pending_reference::PendingClass;
use cognicode_core::infrastructure::graph::per_file_graph::{BuildReport, BuildStatus};
use std::path::Path;

/// A project whose trait and whose method differ only in the case, and
/// whose call names the method.
fn rust_corpus(root: &Path) {
    std::fs::write(
        root.join("lib.rs"),
        "pub trait Describe {\n    fn describe(&self) -> u8 { 1 }\n}\n\
         \npub fn run() -> u8 {\n    Describe::describe(&())\n}\n",
    )
    .expect("write");
}

/// A project in a language that does not tell the case apart: two
/// declarations whose names differ only in the case, and a call that names
/// one of them.
///
/// PHP does not tell `Foo` from `foo` —a function, a method and a class
/// name are all case-insensitive in it— so the call names both
/// declarations and the graph cannot tell which one it means.
fn folding_corpus(root: &Path) {
    std::fs::write(
        root.join("a.php"),
        "<?php\nfunction Foo() { return 1; }\nfunction foo() { return 2; }\n\
         function run() { return foo(); }\n",
    )
    .expect("write");
}

/// The relations of a graph, as a sorted list.
///
/// Sorted, because the order the edges come in is not part of the answer:
/// what has to be the same is which relations exist.
fn relations(graph: &CallGraph) -> Vec<String> {
    let mut out: Vec<String> = graph
        .all_dependencies()
        .map(|(source, target, _)| format!("{source} -> {target}"))
        .collect();
    out.sort();
    out
}

/// The references a build could not tie, with the class that says why.
fn pending(report: &BuildReport) -> Vec<String> {
    match &report.status {
        BuildStatus::Complete => Vec::new(),
        BuildStatus::Partial { pending, .. } => pending
            .iter()
            .map(|reference| {
                let class = match &reference.class {
                    PendingClass::Unresolved => "unresolved".to_string(),
                    PendingClass::Ambiguous { candidates } => format!("ambiguous({candidates})"),
                };
                format!("{}:{}", reference.target, class)
            })
            .collect(),
    }
}

/// The project route ties the call to the declaration spelled the same way.
///
/// The trait is `Describe` and its method is `describe`: the call names the
/// method, and the method is the only declaration spelled `describe`. A
/// fold makes the trait a candidate too, and the call becomes ambiguous —
/// a hole where the code makes an edge.
#[test]
fn the_project_route_ties_the_call_to_the_declaration_spelled_the_same_way() {
    let dir = tempfile::tempdir().expect("tempdir");
    rust_corpus(dir.path());

    let service = AnalysisService::new();
    service
        .build_project_graph(dir.path())
        .expect("the project route builds");
    let report = service
        .get_last_build_report()
        .expect("the project route publishes its report");

    assert!(
        pending(&report).is_empty(),
        "the call names a declaration the project declares: {:?}",
        pending(&report)
    );
    let relations = relations(&service.get_project_graph());
    assert_eq!(
        relations.len(),
        1,
        "the call is one relation, not a hole: {relations:?}"
    );
}

/// The per-file route says the same thing.
///
/// The two routes judge a call by the same rule, and a rule that folds in
/// one route and not in the other is two rules.
#[test]
fn the_per_file_route_ties_the_call_to_the_declaration_spelled_the_same_way() {
    let dir = tempfile::tempdir().expect("tempdir");
    rust_corpus(dir.path());

    let service = AnalysisService::new();
    let report = service
        .build_graph_per_file_with_report(dir.path())
        .expect("the per-file route builds");

    assert!(
        pending(&report).is_empty(),
        "the call names a declaration the file declares: {:?}",
        pending(&report)
    );
    let relations = relations(&report.graph);
    assert_eq!(
        relations.len(),
        1,
        "the call is one relation, not a hole: {relations:?}"
    );
}

/// A language that does not tell the case apart keeps folding.
///
/// The fold is the language's property: PHP does not tell `Foo` from
/// `foo`, so a call that names one of them names both and the graph
/// cannot tell which one it means. That is a hole, and it stays a hole.
#[test]
fn a_language_that_does_not_tell_the_case_apart_keeps_folding() {
    let dir = tempfile::tempdir().expect("tempdir");
    folding_corpus(dir.path());

    let service = AnalysisService::new();
    service
        .build_project_graph(dir.path())
        .expect("the project route builds");
    let report = service
        .get_last_build_report()
        .expect("the project route publishes its report");

    let pending = pending(&report);
    assert_eq!(
        pending.len(),
        1,
        "the call names two declarations the language cannot tell apart: {pending:?}"
    );
    assert!(
        pending[0].contains("ambiguous"),
        "the call is ambiguous, and says so: {pending:?}"
    );
}
