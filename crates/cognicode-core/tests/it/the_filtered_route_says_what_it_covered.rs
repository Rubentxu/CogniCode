//! The filtered route must say what it covered.
//!
//! Measured: the filtered route walked the project on its own, dropped
//! every error of the walk, dropped every file whose reader failed, and
//! answered with a bare graph. The whole-project route answered the
//! same project with a graph *and* the report of what it could not
//! read. Two routes of one product, two different guarantees: whoever
//! consumed the filtered one could not tell a complete project from a
//! truncated one.
//!
//! The law this file pins:
//!
//! - the filtered route publishes a report, and
//! - the filtered route and the whole-project route name the same
//!   files they could not read.
//!
//! The second half is the one that matters. A report nobody compares
//! against anything drifts: the two routes answer the same question
//! about the same tree and say different things, and the consumer has
//! no way of telling which one is right.

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::infrastructure::graph::per_file_graph::{BuildStatus, SkipReason};

/// The corpus with a readable file, a file whose reader fails and a
/// file with broken syntax.
fn corpus() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates/cognicode-core/testdata/per_file_partial_corpus")
}

/// The paths the report names as not covered.
fn skipped_paths(status: &BuildStatus) -> Vec<String> {
    match status {
        BuildStatus::Partial { skipped, .. } => skipped.iter().map(|s| s.path.clone()).collect(),
        BuildStatus::Complete => Vec::new(),
    }
}

#[test]
fn the_filtered_route_names_the_file_it_could_not_read() {
    let corpus = corpus();
    assert!(
        corpus.join("src/broken_syntax.rs").exists(),
        "the corpus must carry a file the reader cannot read"
    );

    let report = AnalysisService::new()
        .build_project_graph_filtered(&corpus, &[corpus.as_path()])
        .expect("the filtered route builds");

    assert!(
        matches!(report.status, BuildStatus::Partial { .. }),
        "a project with a file the reader cannot read is not fully covered; \
         got {:?}",
        report.status
    );

    let broken = skipped_paths(&report.status)
        .into_iter()
        .find(|path| path.ends_with("broken_syntax.rs"))
        .unwrap_or_else(|| {
            panic!(
                "the file the reader could not read must be named in the report, \
                 got {:?}",
                report.status
            )
        });
    assert!(
        broken.ends_with("broken_syntax.rs"),
        "the report names the file it could not read: {broken}"
    );
}

#[test]
fn the_filtered_route_and_the_whole_route_agree_on_what_they_could_not_read() {
    let corpus = corpus();

    let filtered = AnalysisService::new()
        .build_project_graph_filtered(&corpus, &[corpus.as_path()])
        .expect("the filtered route builds");

    let whole = AnalysisService::new();
    whole
        .build_project_graph(&corpus)
        .expect("the whole-project route builds");
    let whole = whole
        .get_last_build_report()
        .expect("the whole-project route publishes a report");

    let mut filtered_paths = skipped_paths(&filtered.status);
    let mut whole_paths = skipped_paths(&whole.status);
    filtered_paths.sort();
    whole_paths.sort();

    assert_eq!(
        filtered_paths, whole_paths,
        "the two routes of one product named different files they could not read"
    );
}

#[test]
fn the_whole_route_names_the_file_it_could_not_read() {
    let corpus = corpus();

    let service = AnalysisService::new();
    service
        .build_project_graph(&corpus)
        .expect("the whole-project route builds");
    let report = service
        .get_last_build_report()
        .expect("the whole-project route publishes a report");

    assert!(
        matches!(report.status, BuildStatus::Partial { .. }),
        "a project with a file the reader cannot read is not fully covered; \
         got {:?}",
        report.status
    );
    assert!(
        skipped_paths(&report.status)
            .iter()
            .any(|path| path.ends_with("broken_syntax.rs")),
        "the whole-project route must name the file it could not read, got {:?}",
        report.status
    );
}

#[test]
fn the_filtered_route_reports_a_readable_project_as_complete() {
    // `Complete` is more than «every file was read»: it is also «every
    // reference was tied to a declaration». This corpus declares one
    // function and calls nothing, so there is nothing to tie and
    // nothing to publish — a corpus with a call the project cannot
    // tell apart would be `Partial`, and would publish that call.
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("lib.rs"),
        "pub fn readable() -> u32 { 1 }\n",
    )
    .expect("write file");

    let report = AnalysisService::new()
        .build_project_graph_filtered(tmp.path(), &[tmp.path()])
        .expect("the filtered route builds");

    assert!(
        matches!(report.status, BuildStatus::Complete),
        "a project the route could read completely is fully covered; got {:?}",
        report.status
    );
    assert!(
        report.graph.symbol_count() > 0,
        "the route must still build the graph of the project it was given"
    );
}

#[test]
fn the_filtered_route_classifies_the_failure_it_reports() {
    let corpus = corpus();

    let report = AnalysisService::new()
        .build_project_graph_filtered(&corpus, &[corpus.as_path()])
        .expect("the filtered route builds");

    let BuildStatus::Partial { skipped, .. } = &report.status else {
        panic!("the route must report Partial; got {:?}", report.status)
    };
    let broken = skipped
        .iter()
        .find(|s| s.path.ends_with("broken_syntax.rs"))
        .expect("the file the reader could not read must be named in the report");
    assert!(
        matches!(broken.reason, SkipReason::Parse(_)),
        "the report must say why the file was not read, got {:?}",
        broken.reason
    );
}
