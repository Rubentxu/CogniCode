//! What the graph reports must not depend on the order the machine listed
//! the directories in.
//!
//! A graph is built by folding the project's files one after another, and
//! the order of the fold is the order in which the index receives the
//! symbols. The index reports the candidates of a repeated name in the
//! order it received them, so the order the files were listed in reaches
//! the answer.
//!
//! A directory listing is the machine's order, not the project's: the same
//! tree can be listed differently on two machines, and a tree that is
//! copied to a different filesystem can be listed differently on the same
//! machine. A graph that answers differently depending on where the tree
//! lives is a graph whose answer cannot be compared with anything.
//!
//! The whole-project graph folds its files in path order, so that what it
//! reports is the same wherever the tree is. The filtered graph — the one
//! the workspace session answers with — folds its files in the order the
//! walk listed them. This file pins that both say the same thing about the
//! same project.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use std::path::PathBuf;

/// How many times the fixture repeats its shape.
const PAIRS: usize = 8;

/// The name every file declares. Repeated on purpose: the order the graph
/// reports them in is the order it received them.
const REPEATED: &str = "handler";

/// A project whose files all declare the same name, arranged so that the
/// order a walk visits them in and the order their paths say are two
/// different things.
///
/// Each repetition is a directory `dN` holding one file, next to a file
/// `dN.rs`. Comparing the paths as whole strings, `dN.rs` comes before
/// `dN/x.rs`, because `.` sorts before `/`. A walk that descends into `dN`
/// when it meets it visits `dN/x.rs` first. So the two orders disagree, and
/// they disagree for a reason that has nothing to do with the graph.
fn project_with_repeated_names() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    std::fs::create_dir_all(&project).expect("create project");

    for pair in 0..PAIRS {
        let directory = project.join(format!("d{pair}"));
        std::fs::create_dir_all(&directory).expect("create directory");
        std::fs::write(
            directory.join("x.rs"),
            format!("pub fn {REPEATED}() {{}}\n"),
        )
        .expect("write file");
        std::fs::write(
            project.join(format!("d{pair}.rs")),
            format!("pub fn {REPEATED}() {{}}\n"),
        )
        .expect("write file");
    }

    (tmp, project)
}

/// The files whose symbols the graph reports under `name`, in the order it
/// reports them.
fn reported_files(graph: &CallGraph) -> Vec<String> {
    graph
        .find_by_name(REPEATED)
        .iter()
        .map(|symbol| symbol.location().file().to_string())
        .collect()
}

/// Whether the files are reported in the order their paths say.
fn in_path_order(files: &[String]) -> bool {
    let mut in_path_order = files.to_vec();
    in_path_order.sort();
    files == in_path_order.as_slice()
}

#[test]
fn the_whole_graph_reports_the_repeated_names_in_path_order() {
    let (_tmp, project) = project_with_repeated_names();

    let service = AnalysisService::new();
    service
        .build_project_graph(&project)
        .expect("the graph builds");

    let files = reported_files(&service.get_project_graph());
    assert_eq!(
        files.len(),
        PAIRS * 2,
        "the project declares {} functions named {REPEATED}",
        PAIRS * 2
    );
    assert!(
        in_path_order(&files),
        "the whole graph reported the symbols in an order the project does not have"
    );
}

#[test]
fn the_filtered_graph_reports_the_repeated_names_in_path_order() {
    let (_tmp, project) = project_with_repeated_names();

    let service = AnalysisService::new();
    let report = service
        .build_project_graph_filtered(&project, &[project.as_path()])
        .expect("the filtered graph builds");

    let files = reported_files(&report.graph);
    assert_eq!(
        files.len(),
        PAIRS * 2,
        "the project declares {} functions named {REPEATED}",
        PAIRS * 2
    );
    assert!(
        in_path_order(&files),
        "the filtered graph reported the symbols in the order the machine listed the files"
    );
}

#[test]
fn both_routes_answer_the_same_question_the_same_way() {
    let (_tmp, project) = project_with_repeated_names();

    let whole = AnalysisService::new();
    whole
        .build_project_graph(&project)
        .expect("the whole graph builds");

    let filtered = AnalysisService::new();
    let filtered_report = filtered
        .build_project_graph_filtered(&project, &[project.as_path()])
        .expect("the filtered graph builds");

    assert_eq!(
        reported_files(&whole.get_project_graph()),
        reported_files(&filtered_report.graph),
        "the whole project and the filtered view of it reported the same symbols in two orders"
    );
}
