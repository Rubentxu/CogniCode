//! The project walkers must only ever index what lives **under the
//! root the caller named**.
//!
//! Every walker was built with `WalkDir::follow_links(true)`, which
//! descends into symlinked directories and yields symlinked files as
//! if they were ordinary entries. The per-entry filter did not close
//! that hole either: `Path::is_file()` calls `fs::metadata`, which
//! *follows* the link, so a symlink to a file outside the root still
//! reported `is_file() == true`.
//!
//! The consequence is a graph that claims to describe the project and
//! actually describes the project plus whatever the machine happens to
//! have reachable next to it: `~/.cargo`, `/etc`, a sibling checkout.
//! The paths recorded in the graph are the symlink paths, so nothing
//! downstream can tell the difference — the report says `Complete`.
//!
//! The law this file pins: **a walk rooted at `R` yields files under
//! `R`, and only files under `R`.** A symlink is a name, not a
//! licence to read through it.
//!
//! Two escapes are covered, because they fail by different mechanisms:
//! a symlinked *file* (caught by the `is_file()` filter following the
//! link) and a symlinked *directory* (caught only by the walker's
//! link-following). The third test is the anti-vacuity guard: the fix
//! must not degrade into "index nothing".

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::infrastructure::graph::per_file_graph::{BuildReport, BuildStatus, SkipReason};
use cognicode_core::infrastructure::graph::{FullGraphStrategy, PerFileStrategy};

/// The names of every symbol in `report`.
///
/// Symbol names, not file paths, are the oracle. A symlinked file is
/// recorded under its *own* in-root path (`src/link.rs`), so a path
/// check sees nothing wrong while the caller receives a symbol that
/// was defined outside the project it asked about. The name is the
/// thing that actually leaked.
fn symbol_names(report: &BuildReport) -> Vec<String> {
    report
        .graph
        .symbols()
        .map(|s| s.name().to_string())
        .collect()
}

fn write(path: &std::path::Path, contents: &str) {
    std::fs::write(path, contents).unwrap();
}

/// A root that contains exactly one indexable source file, plus a
/// sibling directory *outside* the root holding a second one.
///
/// Returns `(outside_dir, root_dir, leaked_filename)`.
fn workspace_with_a_neighbour(outside: &std::path::Path, root: &std::path::Path) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(outside).unwrap();

    write(
        &root.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    );
    write(
        &root.join("src").join("main.rs"),
        "mod inside;\nfn main() { inside::inside_fn(); }\n",
    );
    write(
        &root.join("src").join("inside.rs"),
        "pub fn inside_fn() -> u32 { 1 }\n",
    );

    write(
        &outside.join("outside.rs"),
        "pub fn leaked_fn() -> u32 { 2 }\n",
    );
}

fn assert_only_inside(report: &BuildReport, root: &std::path::Path) {
    let names = symbol_names(report);

    assert!(
        names.iter().any(|n| n == "inside_fn"),
        "the file that really is inside the root must still be indexed; \
         got {names:?} — a fix that indexes nothing is not a fix"
    );

    assert!(
        !names.iter().any(|n| n == "leaked_fn"),
        "the symlink indexed a symbol defined outside {root:?}: {names:?}"
    );
}

#[test]
fn per_file_strategy_does_not_index_through_a_symlinked_file() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside");
    let root = tmp.path().join("root");
    workspace_with_a_neighbour(&outside, &root);

    std::os::unix::fs::symlink(outside.join("outside.rs"), root.join("src").join("link.rs"))
        .unwrap();

    let report = PerFileStrategy::new().build_full_graph_report(&root);
    assert_only_inside(&report, &root);
}

#[test]
fn per_file_strategy_does_not_descend_into_a_symlinked_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside");
    let root = tmp.path().join("root");
    workspace_with_a_neighbour(&outside, &root);

    std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();

    let report = PerFileStrategy::new().build_full_graph_report(&root);
    assert_only_inside(&report, &root);
}

#[test]
fn full_graph_strategy_does_not_index_through_a_symlinked_file() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside");
    let root = tmp.path().join("root");
    workspace_with_a_neighbour(&outside, &root);

    std::os::unix::fs::symlink(outside.join("outside.rs"), root.join("src").join("link.rs"))
        .unwrap();

    let report = FullGraphStrategy::new().build_full_graph_report(&root);
    assert_only_inside(&report, &root);
}

#[test]
fn full_graph_strategy_does_not_descend_into_a_symlinked_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside");
    let root = tmp.path().join("root");
    workspace_with_a_neighbour(&outside, &root);

    std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();

    let report = FullGraphStrategy::new().build_full_graph_report(&root);
    assert_only_inside(&report, &root);
}

#[test]
fn a_declined_symlink_degrades_the_report_from_complete_to_partial() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside");
    let root = tmp.path().join("root");
    workspace_with_a_neighbour(&outside, &root);

    std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();

    let report = PerFileStrategy::new().build_full_graph_report(&root);

    assert!(
        matches!(report.status, BuildStatus::Partial { .. }),
        "declining a symlink leaves the coverage partial; reporting Complete \
         here would be the same lie the walk was fixed to stop telling, \
         got {:?}",
        report.status
    );

    let BuildStatus::Partial { skipped, .. } = &report.status else {
        unreachable!("just asserted Partial")
    };
    assert!(
        skipped
            .iter()
            .any(|s| matches!(s.reason, SkipReason::Symlink { .. })),
        "the symlink must be named in `skipped`, not silently dropped; got {skipped:?}"
    );
}

// ---------------------------------------------------------------------------
// The paths MCP actually takes
// ---------------------------------------------------------------------------
//
// The two strategies above are not the paths a caller reaches. MCP's
// `handle_build_graph` lands on `AnalysisService::build_project_graph`,
// which does *not* go through `strategy.rs`: it runs its own
// `ignore::WalkBuilder` and filtered on `e.path().is_file()` — the
// `fs::metadata` call, so it follows links. Making the strategies
// refuse symlinks while this one still followed them would leave two
// code paths over the same directory disagreeing about coverage, and
// the disagreeing one would be the wired one.

#[test]
fn the_production_graph_path_does_not_read_through_a_symlinked_file() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside");
    let root = tmp.path().join("root");
    workspace_with_a_neighbour(&outside, &root);

    std::os::unix::fs::symlink(outside.join("outside.rs"), root.join("src").join("link.rs"))
        .unwrap();

    let service = AnalysisService::new();
    service.build_project_graph(&root).unwrap();
    let report = service
        .get_last_build_report()
        .expect("build_project_graph always records a report");

    assert_only_inside(&report, &root);
    assert_declined_the_symlink(&report);
}

#[test]
fn the_production_graph_path_does_not_descend_into_a_symlinked_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside");
    let root = tmp.path().join("root");
    workspace_with_a_neighbour(&outside, &root);

    std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();

    let service = AnalysisService::new();
    service.build_project_graph(&root).unwrap();
    let report = service
        .get_last_build_report()
        .expect("build_project_graph always records a report");

    assert_only_inside(&report, &root);
    assert_declined_the_symlink(&report);
}

#[test]
fn the_per_file_report_path_records_the_symlink_it_declined() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside");
    let root = tmp.path().join("root");
    workspace_with_a_neighbour(&outside, &root);

    std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();

    // This method's whole contract is honest gap reporting — it exists
    // so a caller can see what it did not cover. Dropping the symlink
    // without recording it would return `Complete`, which is the same
    // lie the walk was fixed to stop telling, one layer down.
    let report = AnalysisService::new()
        .build_graph_per_file_with_report(&root)
        .expect("the fixture root is a readable Rust project");

    assert_only_inside(&report, &root);
    assert_declined_the_symlink(&report);
}

/// The report must say `Partial` **and** name the symlink. Either half
/// alone is the defect this file exists to prevent: `Complete` with a
/// gap, or `Partial` with no explanation of what was missed.
fn assert_declined_the_symlink(report: &BuildReport) {
    assert!(
        matches!(report.status, BuildStatus::Partial { .. }),
        "declining a symlink leaves the coverage partial; reporting Complete \
         would be the same lie the walk was fixed to stop telling, got {:?}",
        report.status
    );

    let BuildStatus::Partial { skipped, .. } = &report.status else {
        unreachable!("just asserted Partial")
    };
    assert!(
        skipped
            .iter()
            .any(|s| matches!(s.reason, SkipReason::Symlink { .. })),
        "the symlink must be named in `skipped`, not silently dropped; got {skipped:?}"
    );
}

#[test]
fn a_real_file_inside_the_root_is_still_indexed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    workspace_with_a_neighbour(&tmp.path().join("outside"), &root);

    let report = PerFileStrategy::new().build_full_graph_report(&root);
    assert_only_inside(&report, &root);

    // And the report must claim the completeness it actually has.
    // Without this, a walk that flagged every entry as skipped would
    // satisfy every other assertion in this file.
    assert!(
        matches!(report.status, BuildStatus::Complete),
        "a workspace with no symlinks is fully covered; got {:?}",
        report.status
    );
}
