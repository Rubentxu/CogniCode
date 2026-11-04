//! The three routes that build the project graph have to say the same thing.
//!
//! MEDIDO over one corpus, the whole-project route, the filtered route and
//! the async route do not agree:
//!
//!   - the coverage denominator differs. The whole-project route counts the
//!     files the graph never looked at — a `Cargo.toml`, a `README.md` —
//!     while the other two count only the sources the graph was built from;
//!   - a symlink the walk declines is a hole in two routes and silence in the
//!     async one, which can publish `Complete` over a tree it does not
//!     describe;
//!   - a directory that does not exist is refused by one route only;
//!   - the async route does not order its results, so the index of repeated
//!     names inherits the order the machine happened to list the files in.
//!
//! The corpus carries, on purpose, the two things that make the routes
//! separate: files the graph does not analyse, and a symlink the walk
//! declines.
//!
//! The law this file pins: **the same corpus, the same answer.** Whichever
//! route a consumer reaches the product through — the whole graph, the
//! filtered one or the async one — the relations, the coverage and the holes
//! are the same. A consumer that gets a different graph depending on which
//! interface it asked cannot compare its answer with anything.
//!
//! Comparing the routes is the only form that can hold this: a test per
//! route passes while the routes answer differently.

use cognicode_core::application::dto::GraphCoverageMetrics;
use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::graph::per_file_graph::{BuildReport, BuildStatus};
use std::path::{Path, PathBuf};

/// What a route answered.
struct Answer {
    name: &'static str,
    report: BuildReport,
    coverage: Option<GraphCoverageMetrics>,
}

/// The coverage of a route, as the facts about the tree.
///
/// The percentage is left out on purpose: it is a consequence of the two
/// numbers, and a float would make the failure harder to read.
#[derive(Debug, PartialEq)]
struct Coverage {
    total_source_files: usize,
    covered_files: usize,
    unresolved_edges: usize,
}

fn coverage_of(answer: &Answer) -> Coverage {
    let coverage = answer.coverage.as_ref().unwrap_or_else(|| {
        panic!(
            "the route {} published no coverage: a build that does not say what \
             it covered cannot be compared with another build",
            answer.name
        )
    });
    Coverage {
        total_source_files: coverage.total_source_files,
        covered_files: coverage.covered_files,
        unresolved_edges: coverage.unresolved_edges,
    }
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

/// The names of the files whose symbols the graph reports under `name`, in
/// the order it reports them.
fn reported_files(graph: &CallGraph, name: &str) -> Vec<String> {
    graph
        .find_by_name(name)
        .iter()
        .map(|symbol| symbol.location().file().to_string())
        .collect()
}

/// Whether the files are reported in the order their paths say.
fn in_path_order(files: &[String]) -> bool {
    let mut sorted = files.to_vec();
    sorted.sort();
    files == sorted.as_slice()
}

/// A project with two sources that call each other, and two files the graph
/// does not analyse: a manifest and a readme.
///
/// The two extra files are the point. They are walked, they are not sources,
/// and a route that counts them in the denominator of the coverage publishes
/// a percentage that belongs to a different question.
fn corpus_with_files_the_graph_does_not_analyse(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write manifest");
    std::fs::write(root.join("README.md"), "# Fixture\n\nUn proyecto normal.\n")
        .expect("write readme");
    std::fs::write(
        root.join("src").join("main.rs"),
        "mod inside;\nfn main() { inside::inside_fn(); }\n",
    )
    .expect("write main");
    std::fs::write(
        root.join("src").join("inside.rs"),
        "pub fn inside_fn() -> u32 { 1 }\n",
    )
    .expect("write inside");
}

/// A project whose files all declare the same name, arranged so that the
/// order a walk visits them in and the order their paths say are two
/// different things.
///
/// Each repetition is a directory `dN` holding one file, next to a file
/// `dN.rs`. Comparing the paths as whole strings, `dN.rs` comes before
/// `dN/x.rs`, because `.` sorts before `/`. A walk that descends into `dN`
/// when it meets it visits `dN/x.rs` first. So the two orders disagree, and
/// they disagree for a reason that has nothing to do with the graph.
fn corpus_with_repeated_names(root: &Path) {
    std::fs::create_dir_all(root).expect("create root");
    for pair in 0..3 {
        let directory = root.join(format!("d{pair}"));
        std::fs::create_dir_all(&directory).expect("create directory");
        std::fs::write(directory.join("x.rs"), "pub fn handler() {}\n").expect("write nested file");
        std::fs::write(root.join(format!("d{pair}.rs")), "pub fn handler() {}\n")
            .expect("write file");
    }
    std::fs::write(root.join("main.rs"), "fn main() { handler(); }\n").expect("write main");
}

/// The three routes, each with a service of its own, over the same root.
async fn answer_from_all_three(root: &Path) -> Vec<Answer> {
    let whole = {
        let service = AnalysisService::new();
        service
            .build_project_graph(root)
            .expect("the whole-project graph builds");
        let report = service
            .get_last_build_report()
            .expect("the whole-project route publishes its report");
        Answer {
            name: "whole",
            report,
            coverage: service.get_coverage_metrics(),
        }
    };

    let filtered = {
        let service = AnalysisService::new();
        let report = service
            .build_project_graph_filtered(root, &[root])
            .expect("the filtered graph builds");
        Answer {
            name: "filtered",
            report,
            coverage: service.get_coverage_metrics(),
        }
    };

    let asynchronous = {
        let service = AnalysisService::new();
        service
            .build_project_graph_async(root)
            .await
            .expect("the async graph builds");
        let report = service
            .get_last_build_report()
            .expect("the async route publishes its report");
        Answer {
            name: "async",
            report,
            coverage: service.get_coverage_metrics(),
        }
    };

    vec![whole, filtered, asynchronous]
}

/// A rendering of the three coverages, for a message that has to say which
/// route said what.
fn rendered(answers: &[Answer]) -> String {
    answers
        .iter()
        .map(|a| {
            let coverage = a
                .coverage
                .as_ref()
                .map(|c| {
                    format!(
                        "total={} covered={} unresolved={}",
                        c.total_source_files, c.covered_files, c.unresolved_edges
                    )
                })
                .unwrap_or_else(|| "<no coverage>".to_string());
            format!("{}: {coverage}", a.name)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// The coverage is a fact about the tree, not about the route that walked
/// it. The denominator is the files the graph was built from: a file with no
/// recognized language was never a candidate, so it does not count.
#[tokio::test]
async fn the_three_routes_publish_the_same_coverage() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    corpus_with_files_the_graph_does_not_analyse(&root);

    let answers = answer_from_all_three(&root).await;
    let rendered = rendered(&answers);

    assert_eq!(
        coverage_of(&answers[0]),
        coverage_of(&answers[1]),
        "the whole-project route and the filtered route published a different \
         coverage for the same tree: {rendered}"
    );
    assert_eq!(
        coverage_of(&answers[0]),
        coverage_of(&answers[2]),
        "the whole-project route and the async route published a different \
         coverage for the same tree: {rendered}"
    );
}

/// The relations are the answer the consumer asked for. Two routes that
/// report different relations are two graphs.
#[tokio::test]
async fn the_three_routes_produce_the_same_relations() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    corpus_with_files_the_graph_does_not_analyse(&root);

    let answers = answer_from_all_three(&root).await;

    let whole = relations(&answers[0].report.graph);
    let filtered = relations(&answers[1].report.graph);
    let asynchronous = relations(&answers[2].report.graph);

    assert!(
        !whole.is_empty(),
        "the corpus declares a call and the whole-project route did not report \
         it: {whole:?} — a test that compares empty answers proves nothing"
    );
    assert_eq!(
        whole, filtered,
        "the whole-project route and the filtered route reported different \
         relations for the same tree: {whole:?} vs {filtered:?}"
    );
    assert_eq!(
        whole, asynchronous,
        "the whole-project route and the async route reported different \
         relations for the same tree: {whole:?} vs {asynchronous:?}"
    );
}

/// A name that several declarations share is the case where the order the
/// files were read in reaches the answer: the index keeps the candidates in
/// the order it received them. A route that reads the files in the order the
/// machine listed them resolves the call to a different declaration than a
/// route that reads them in the order their paths say.
#[tokio::test]
async fn the_three_routes_resolve_a_repeated_name_the_same_way() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    corpus_with_repeated_names(&root);

    let answers = answer_from_all_three(&root).await;

    let whole = relations(&answers[0].report.graph);
    let filtered = relations(&answers[1].report.graph);
    let asynchronous = relations(&answers[2].report.graph);

    assert_eq!(
        whole, filtered,
        "the whole-project route and the filtered route tied the call to \
         different declarations: {whole:?} vs {filtered:?}"
    );
    assert_eq!(
        whole, asynchronous,
        "the whole-project route and the async route tied the call to \
         different declarations: {whole:?} vs {asynchronous:?}"
    );
}

/// A symlink the walk declines is a hole in the coverage. The route that
/// drops it in silence can say `Complete` over a tree it does not describe,
/// and `Complete` is the direction that costs the most.
#[tokio::test]
async fn a_declined_symlink_is_a_hole_in_every_route() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    corpus_with_files_the_graph_does_not_analyse(&root);

    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(&outside).expect("create outside");
    std::fs::write(
        outside.join("outside.rs"),
        "pub fn outside_fn() -> u32 { 2 }\n",
    )
    .expect("write outside");
    std::os::unix::fs::symlink(outside.join("outside.rs"), root.join("src").join("link.rs"))
        .expect("create symlink");

    let answers = answer_from_all_three(&root).await;

    for answer in &answers {
        let BuildStatus::Partial { skipped, .. } = &answer.report.status else {
            panic!(
                "the route {} declined a symlink and reported {:?}: the graph does \
                 not describe what is behind the link, and a coverage that says \
                 otherwise is a claim nothing backs",
                answer.name, answer.report.status
            );
        };
        assert!(
            skipped.iter().any(|s| s.path.ends_with("link.rs")),
            "the route {} reported a hole without naming the symlink it declined: \
             {skipped:?}",
            answer.name
        );
    }
}

/// A root that does not exist is not a project with zero files. Every route
/// has to refuse it, and with the same reason.
#[tokio::test]
async fn a_missing_directory_is_refused_by_every_route() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let missing = tmp.path().join("does-not-exist");

    let whole = AnalysisService::new().build_project_graph(&missing);
    assert!(
        whole.is_err(),
        "the whole-project route built a graph over a directory that does not \
         exist: {whole:?}"
    );

    let filtered = AnalysisService::new().build_project_graph_filtered(&missing, &[]);
    assert!(
        filtered.is_err(),
        "the filtered route built a graph over a directory that does not exist: \
         {filtered:?}"
    );

    let asynchronous = AnalysisService::new()
        .build_project_graph_async(&missing)
        .await;
    assert!(
        asynchronous.is_err(),
        "the async route built a graph over a directory that does not exist: \
         {asynchronous:?}"
    );
}

/// The order the graph reports the repeated names in is not the order the
/// machine listed the files in. It is the order their paths say, because
/// that order is the same on every machine.
#[tokio::test]
async fn the_three_routes_report_the_repeated_names_in_path_order() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    corpus_with_repeated_names(&root);

    let answers = answer_from_all_three(&root).await;

    for answer in &answers {
        let files = reported_files(&answers[0].report.graph, "handler");
        assert_eq!(
            files.len(),
            6,
            "the project declares six functions named handler"
        );
        let files = reported_files(&answer.report.graph, "handler");
        assert_eq!(
            files.len(),
            6,
            "the route {} reported {} of the six functions named handler",
            answer.name,
            files.len()
        );
        assert!(
            in_path_order(&files),
            "the route {} reported the symbols in the order the machine listed \
             the files in, not the order their paths say: {files:?}",
            answer.name
        );
    }
}

/// The guard: whatever the routes are made to agree on, they still have to
/// build a graph. A fix that makes three routes agree by building nothing
/// agrees with nothing.
#[tokio::test]
async fn the_routes_still_build_a_graph() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    corpus_with_files_the_graph_does_not_analyse(&root);

    let answers = answer_from_all_three(&root).await;

    for answer in &answers {
        let names: Vec<String> = answer
            .report
            .graph
            .symbols()
            .map(|s| s.name().to_string())
            .collect();
        assert!(
            names.iter().any(|n| n == "inside_fn"),
            "the route {} did not index the function that is really there: \
             {names:?} — a fix that indexes nothing is not a fix",
            answer.name
        );
        assert!(
            !names.iter().any(|n| n == "outside_fn"),
            "the route {} indexed a symbol that lives outside the root: {names:?}",
            answer.name
        );
    }
}

/// The paths a route reports, for a message about where a file was seen.
#[allow(dead_code)]
fn reported_paths(report: &BuildReport) -> Vec<PathBuf> {
    report
        .graph
        .symbols()
        .map(|s| PathBuf::from(s.location().file()))
        .collect()
}
