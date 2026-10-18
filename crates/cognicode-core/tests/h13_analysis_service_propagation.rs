//! H1.3 (delivery-first-radical, package integration) — propagation
//! of the typed classification from `AnalysisService` call sites.
//!
//! Claim: every `AnalysisService` method that walks the filesystem
//! routes through the same `WalkFilter` + `classify_walk_extension`
//! pipeline, and surfaces its `BuildReport` via the
//! `get_last_build_report` accessor. Two methods were missing
//! this:
//!
//!   - `build_graph_per_file` (line 662): used raw `std::fs::read_dir`
//!     (non-recursive, no WalkFilter, no classifier), called
//!     `Language::from_extension` for filtering (registry-based but
//!     no `.kt`/`.kts` gap surfacing), ignored parse errors, and
//!     did NOT set `last_build_report` (so a caller reading
//!     `get_last_build_report()` after `build_graph_per_file` got
//!     stale data from a previous `build_project_graph`).
//!   - `find_symbol_usages` (line 1639): used a local
//!     `USAGE_SKIP_DIRS` const that duplicated
//!     `WalkFilter::with_performance_skips` (and missed the security
//!     blocklist — `.env` files would be searched).
//!
//! H1.3 test strategy: pin the new typed method, pin the
//! `last_build_report` parity (so a consumer that asks "what was
//! the coverage of the last build" gets an honest answer regardless
//! of which method was called), pin that the security blocklist is
//! now honored, and pin that the `USAGE_SKIP_DIRS` const is no
//! longer the source of truth.

use cognicode_core::application::services::analysis_service::{AnalysisService, UsageSearchParams};
use cognicode_core::infrastructure::graph::per_file_graph::{BuildStatus, FileDisposition};

fn tempdir(suffix: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "h13-an-{suffix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn rust_with_kt_and_readme(dir: &std::path::Path) {
    // Same fixture as h12. Two .rs files (parsed), one .kt
    // (K0 honest gap), one README.md (legit exclusion). Together
    // they exercise the three H1.1 dispositions.
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("main.rs"),
        "mod alpha;\nmod beta;\nfn main() { alpha::helper(); beta::helper(); }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("alpha.rs"),
        "pub fn helper() -> u32 { 1 }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("beta.rs"),
        "pub fn helper() -> u32 { 2 }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("Other.kt"),
        "class Other { fun x() = 1 }\n",
    )
    .unwrap();
    std::fs::write(dir.join("README.md"), "# Fixture\n").unwrap();
}

#[test]
fn build_graph_per_file_with_report_returns_typed_partial_on_kt_corpus() {
    // The new method exists and surfaces the K0 honest gap
    // (.kt → UnsupportedRelevant → BuildStatus::Partial). This
    // is the H1.1 envelope propagated to the per-file pipeline.
    let tmp = tempdir("per-file-with-report");
    rust_with_kt_and_readme(&tmp);
    let svc = AnalysisService::new();
    let report = svc
        .build_graph_per_file_with_report(&tmp)
        .expect("build_graph_per_file_with_report succeeds");
    match &report.status {
        BuildStatus::Partial { skipped } => {
            assert!(
                skipped.iter().any(|s| s.path.ends_with("Other.kt")),
                "the .kt file must surface as a Partial skip via the H1.1 envelope, got {skipped:?}"
            );
        }
        BuildStatus::Complete => panic!(
            "expected Partial because of the .kt file; got Complete (the K0 honest gap was lost)"
        ),
    }
}

#[test]
fn build_graph_per_file_now_sets_last_build_report() {
    // Parity with build_project_graph. Before H1.3, calling
    // build_graph_per_file did NOT update last_build_report, so
    // get_last_build_report() returned stale data from a prior
    // build_project_graph call. The fix: the new internal helper
    // is the single source of truth, and both legacy entry points
    // route through it.
    let tmp = tempdir("per-file-sets-report");
    rust_with_kt_and_readme(&tmp);
    let svc = AnalysisService::new();
    svc.build_graph_per_file(&tmp).expect("ok");
    let report = svc
        .get_last_build_report()
        .expect("build_graph_per_file must now set last_build_report");
    match &report.status {
        BuildStatus::Partial { skipped } => {
            assert!(
                skipped.iter().any(|s| s.path.ends_with("Other.kt")),
                "the K0 gap must survive the legacy entry point, got {skipped:?}"
            );
        }
        BuildStatus::Complete => {
            panic!("build_graph_per_file must record the .kt gap, not silently swallow it")
        }
    }
}

#[test]
fn build_graph_per_file_kt_skip_projects_to_unsupported_relevant() {
    // The H1.1 invariant at the AnalysisService boundary: a .kt
    // skip is UnsupportedRelevant, not ReadFailed (the read
    // succeeds) and not ExcludedByPolicy (the path is a relevant
    // source).
    let tmp = tempdir("per-file-h11-invariant");
    rust_with_kt_and_readme(&tmp);
    let svc = AnalysisService::new();
    svc.build_graph_per_file(&tmp).expect("ok");
    let report = svc.get_last_build_report().expect("ok");
    let kt_skip = match &report.status {
        BuildStatus::Partial { skipped } => skipped
            .iter()
            .find(|s| s.path.ends_with("Other.kt"))
            .expect("the .kt must be in skipped"),
        BuildStatus::Complete => panic!("expected Partial"),
    };
    let disp = FileDisposition::from_skipped(kt_skip);
    assert!(
        matches!(disp, FileDisposition::UnsupportedRelevant { .. }),
        "the .kt skip must project to UnsupportedRelevant, got {disp:?}"
    );
    assert!(!matches!(disp, FileDisposition::ReadFailed { .. }));
    assert!(!matches!(disp, FileDisposition::ExcludedByPolicy { .. }));
}

#[test]
fn find_symbol_usages_still_skips_target_node_modules_and_venv() {
    // Regression: the dedup must not change which dirs are skipped.
    // The previous `USAGE_SKIP_DIRS` const was the contract for
    // dependency/build/cache exclusion; the new WalkFilter-driven
    // path must produce the same set.
    //
    // The token `unique_keep_marker_x` appears in `lib.rs`
    // (which should be found) and inside `target/`,
    // `node_modules/`, and `.venv/` (which must NOT be searched
    // because the directories are pruned).
    let tmp = tempdir("find-skip-perf");
    std::fs::create_dir_all(tmp.join("src")).unwrap();
    std::fs::write(
        tmp.join("src").join("lib.rs"),
        "pub fn unique_keep_marker_x() {}\n",
    )
    .unwrap();
    std::fs::create_dir_all(tmp.join("target")).unwrap();
    std::fs::write(
        tmp.join("target").join("artifact.txt"),
        "this file would match unique_keep_marker_x if walked",
    )
    .unwrap();
    std::fs::create_dir_all(tmp.join("node_modules")).unwrap();
    std::fs::write(
        tmp.join("node_modules").join("dep.txt"),
        "this file would match unique_keep_marker_x if walked",
    )
    .unwrap();
    std::fs::create_dir_all(tmp.join(".venv")).unwrap();
    std::fs::write(
        tmp.join(".venv").join("pkg.txt"),
        "this file would match unique_keep_marker_x if walked",
    )
    .unwrap();

    let svc = AnalysisService::new();
    let params = UsageSearchParams {
        project_dir: tmp.clone(),
        symbol_name: "unique_keep_marker_x".to_string(),
        include_declaration: true,
        context_lines: Some(0),
        first_only_definition: false,
    };
    let usages = svc.find_symbol_usages(params).expect("ok");
    let paths: Vec<String> = usages.iter().map(|u| u.file.clone()).collect();
    assert!(
        paths.iter().any(|p| p.contains("lib.rs")),
        "the kept file must still be found, got {paths:?}"
    );
    for skip_dir in ["target", "node_modules", ".venv"] {
        assert!(
            !paths.iter().any(|p| p.contains(skip_dir)),
            "the {skip_dir} directory must be skipped (regression), got {paths:?}"
        );
    }
}

#[test]
fn find_symbol_usages_now_skips_security_blocklist_dirs() {
    // The dedup adds the security blocklist. Previously
    // `USAGE_SKIP_DIRS` had only the performance skips, so a
    // `.env` file would be searched. After the dedup,
    // `WalkFilter::default()` includes the security blocklist.
    //
    // We place a unique marker in `.env` and search for it; the
    // dedup must prune `.env` before the file is read.
    let tmp = tempdir("find-skip-sec");
    std::fs::create_dir_all(tmp.join("src")).unwrap();
    std::fs::write(
        tmp.join("src").join("lib.rs"),
        "pub fn legit_search_token() {}\n",
    )
    .unwrap();
    std::fs::write(
        tmp.join(".env"),
        "SECRET=this_file_would_match_legit_search_token_if_walked\n",
    )
    .unwrap();

    let svc = AnalysisService::new();
    let params = UsageSearchParams {
        project_dir: tmp.clone(),
        symbol_name: "legit_search_token".to_string(),
        include_declaration: true,
        context_lines: Some(0),
        first_only_definition: false,
    };
    let usages = svc.find_symbol_usages(params).expect("ok");
    let paths: Vec<String> = usages.iter().map(|u| u.file.clone()).collect();
    assert!(
        paths.iter().any(|p| p.contains("lib.rs")),
        "the kept file must still be found, got {paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p.ends_with(".env")),
        "the security blocklist must prune .env; got {paths:?}"
    );
}
