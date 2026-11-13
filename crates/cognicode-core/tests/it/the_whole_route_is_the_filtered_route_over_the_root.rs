//! `build_project_graph` is the filtered route with the project root as the
//! only included directory. This file pins that equivalence on the answer a
//! consumer sees: the graph, the counts, the whole `BuildReport` and the
//! state the service keeps for later queries.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::graph::per_file_graph::BuildReport;
use std::path::Path;

/// Symbols and relations as sorted, comparable lists.
fn shape(graph: &CallGraph) -> (Vec<String>, Vec<String>) {
    let mut symbols: Vec<String> = graph
        .symbols()
        .map(|s| format!("{}@{}", s.fully_qualified_name(), s.location().file()))
        .collect();
    symbols.sort();
    let mut relations: Vec<String> = graph
        .all_dependencies()
        .map(|(source, target, kind)| format!("{source} -> {target} ({kind:?})"))
        .collect();
    relations.sort();
    (symbols, relations)
}

struct Answer {
    graph: CallGraph,
    report: BuildReport,
    coverage: (usize, usize, usize, usize),
}

fn coverage(service: &AnalysisService) -> (usize, usize, usize, usize) {
    let c = service.get_coverage_metrics().expect("coverage published");
    (
        c.total_source_files,
        c.parsed_files,
        c.cached_files,
        c.unresolved_edges,
    )
}

fn whole(root: &Path) -> (Answer, AnalysisService) {
    let service = AnalysisService::new();
    service.build_project_graph(root).expect("whole route");
    let report = service
        .get_last_build_report()
        .expect("whole route publishes its report");
    let answer = Answer {
        graph: (*service.get_project_graph()).clone(),
        report,
        coverage: coverage(&service),
    };
    (answer, service)
}

fn filtered(root: &Path) -> Answer {
    let service = AnalysisService::new();
    let report = service
        .build_project_graph_filtered(root, &[root])
        .expect("filtered route");
    Answer {
        graph: report.graph.clone(),
        report,
        coverage: coverage(&service),
    }
}

fn assert_same_answer(root: &Path) -> Answer {
    let (whole, _) = whole(root);
    let filtered = filtered(root);
    assert_eq!(shape(&whole.graph), shape(&filtered.graph), "graph");
    assert_eq!(whole.report.status, filtered.report.status, "status");
    assert_eq!(
        shape(&whole.report.graph),
        shape(&filtered.report.graph),
        "report graph"
    );
    assert_eq!(whole.coverage, filtered.coverage, "counts");
    whole
}

#[test]
fn a_small_multi_file_project_gets_the_same_answer() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("README.md"), "# fixture\n").unwrap();
    std::fs::write(
        root.join("src/main.rs"),
        "fn main() { helper(); missing_elsewhere(); }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/helper.rs"),
        "pub fn helper() { leaf(); }\npub fn leaf() {}\n",
    )
    .unwrap();
    std::fs::write(root.join("src/util.py"), "def util():\n    return 1\n").unwrap();

    let answer = assert_same_answer(&root);
    let (symbols, relations) = shape(&answer.graph);
    assert!(
        symbols.len() >= 4,
        "the fixture declares symbols: {symbols:?}"
    );
    assert!(!relations.is_empty(), "the fixture calls something");
}

#[test]
fn a_directory_without_code_gets_the_same_answer() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("README.md"), "# nothing to analyse\n").unwrap();

    assert_same_answer(&root);
}

#[test]
fn an_empty_directory_gets_the_same_answer() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).unwrap();

    assert_same_answer(&root);
}

#[test]
fn a_missing_directory_is_refused_with_the_same_reason() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let missing = tmp.path().join("nope");

    let whole = AnalysisService::new()
        .build_project_graph(&missing)
        .expect_err("whole route refuses")
        .to_string();
    let filtered = AnalysisService::new()
        .build_project_graph_filtered(&missing, &[missing.as_path()])
        .expect_err("filtered route refuses")
        .to_string();
    assert_eq!(whole, filtered);
}

/// What the whole route leaves in the service is what later queries read:
/// the cached graph and the last report are the graph and report it built.
#[test]
fn the_whole_route_keeps_its_graph_and_report_in_the_service() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a.rs"), "fn a() { b(); }\nfn b() {}\n").unwrap();

    let (answer, service) = whole(&root);
    assert_eq!(
        shape(&answer.graph),
        shape(&answer.report.graph),
        "the cached graph is the report's graph"
    );
    assert!(service.get_coverage_metrics().is_some());
}
