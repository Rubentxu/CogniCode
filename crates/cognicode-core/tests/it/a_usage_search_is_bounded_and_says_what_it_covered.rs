//! The usage search has a budget, and it says what it did not cover.
//!
//! ## The defect
//!
//! `find-usages` walked the whole tree with no bound: its cost depended
//! on the tree and not on the question. The published 0.103.1 walked it
//! with `walkdir`, which honours no `.gitignore`, so the search
//! descended into `sandbox/` (4,1 GB of third-party repositories) and
//! did not finish in 100 s. Through the MCP the same search answered
//! nothing at all: the `search` category has a 500 ms dispatch deadline
//! and the search is a blocking call, so the consumer got a timeout
//! instead of an answer.
//!
//! ## What is measured
//!
//! - With the budget exhausted the answer is `partial`, it names the
//!   files it left out and it says why.
//! - A file the walk does not cover is neither searched nor read, so it
//!   leaves no hole in the coverage.
//!
//! ## What has to stay true
//!
//! - a search that fits its budget is `complete`;
//! - the symbol is found in every file that names it.

#![cfg(unix)]

use cognicode_core::application::analysis_basis::Budget;
use cognicode_core::application::services::analysis_service::{
    AnalysisService, UsageSearch, UsageSearchParams,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Three files that name the same symbol.
fn tree_of_three() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("the directory is created");
    std::fs::write(src.join("a.rs"), "pub fn alpha() -> u32 { beta() }\n").expect("written");
    std::fs::write(src.join("b.rs"), "pub fn beta() -> u32 { 1 }\n").expect("written");
    std::fs::write(src.join("c.rs"), "pub fn gamma() -> u32 { beta() }\n").expect("written");
    (dir, root)
}

/// The search, with a deadline: a search that cannot be bounded would
/// hold the suite instead of failing it.
fn search_within(root: &Path, symbol: &str, budget: Budget) -> UsageSearch {
    let root = root.to_path_buf();
    let symbol = symbol.to_string();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let answer = AnalysisService::new()
            .find_symbol_usages_with_coverage(UsageSearchParams {
                project_dir: root,
                symbol_name: symbol,
                include_declaration: true,
                context_lines: None,
                first_only_definition: true,
                budget,
            })
            .expect("the search answers");
        let _ = sender.send(answer);
    });
    receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("the search answers within its deadline")
}

/// A search that ran out of budget says so, says why, and says how much
/// it left out.
#[test]
fn a_search_that_exhausts_its_budget_answers_partial_and_says_why() {
    let (_tmp, root) = tree_of_three();

    let search = search_within(
        &root,
        "beta",
        Budget {
            max_elapsed: None,
            max_entities: Some(1),
        },
    );

    assert_eq!(
        search.coverage.published_state().published_label(),
        "partial",
        "a search that stopped early does not say it covered the tree"
    );
    assert_eq!(search.omitted, Some(2), "two files were not read");
    assert!(
        search
            .truncation_reason
            .as_deref()
            .unwrap_or_default()
            .contains("budget"),
        "the answer says why it stopped: {:?}",
        search.truncation_reason
    );
    assert_eq!(
        search.usages.len(),
        1,
        "only the first file was read: {:?}",
        search.usages
    );
}

/// The truncation survives the trip to the wire: the consumer reads the
/// answer, not the search that produced it.
#[test]
fn the_truncated_search_reaches_the_wire() {
    let (_tmp, root) = tree_of_three();

    let search = search_within(
        &root,
        "beta",
        Budget {
            max_elapsed: None,
            max_entities: Some(1),
        },
    );
    let out = cognicode_core::interface::mcp::schemas::FindUsagesOutput::of_search(
        "beta".to_string(),
        search,
    );

    assert_eq!(out.omitted, Some(2), "the wire carries the count");
    assert!(
        out.truncation_reason.is_some(),
        "the wire carries the reason: {:?}",
        out.truncation_reason
    );
    assert_eq!(
        out.coverage.status, "partial",
        "the wire carries the coverage"
    );
}

/// The control: the same tree, with a budget that fits it.
#[test]
fn a_search_that_fits_its_budget_answers_complete() {
    let (_tmp, root) = tree_of_three();

    let search = search_within(&root, "beta", Budget::default());

    assert_eq!(
        search.coverage.published_state().published_label(),
        "complete",
        "a search that read the whole tree is complete"
    );
    assert_eq!(search.omitted, None, "nothing was left out");
    assert_eq!(search.truncation_reason, None, "nothing was truncated");
    assert_eq!(
        search.usages.len(),
        3,
        "every file that names the symbol answers: {:?}",
        search.usages
    );
}

/// A file the `.gitignore` of the project names is neither searched nor
/// read — not even when the project is not a git repository.
#[test]
fn a_file_the_gitignore_names_is_neither_searched_nor_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("the directory is created");
    // The tree is NOT a git repository: the `.gitignore` of a project is
    // the project's, whether or not the project is a repository.
    std::fs::write(root.join(".gitignore"), "ignored.rs\nunreadable.rs\n").expect("written");
    std::fs::write(src.join("lib.rs"), "pub fn beta() -> u32 { 1 }\n").expect("written");
    std::fs::write(src.join("ignored.rs"), "pub fn beta() -> u32 { 2 }\n").expect("written");
    // A file that cannot be read: if the walk read it, the answer would
    // carry a hole for it.
    std::fs::write(src.join("unreadable.rs"), [0xff, 0xfe, 0xfd]).expect("written");

    let search = search_within(&root, "beta", Budget::default());

    assert!(
        !search
            .usages
            .iter()
            .any(|usage| usage.file.ends_with("ignored.rs")),
        "the file the .gitignore names is not searched: {:?}",
        search.usages
    );
    assert_eq!(
        search.coverage.published_state().published_label(),
        "complete",
        "a file the walk never read leaves no hole: {:?}",
        search.coverage.facts().skipped
    );
}

/// The symbol is found in every file that names it, and a file that only
/// mentions it in a comment is not a usage.
#[test]
fn the_symbol_is_found_in_every_file_that_names_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("the directory is created");
    std::fs::write(src.join("declares.rs"), "pub fn beta() -> u32 { 1 }\n").expect("written");
    std::fs::write(src.join("calls.rs"), "pub fn alpha() -> u32 { beta() }\n").expect("written");
    std::fs::write(src.join("mentions.rs"), "// beta is only mentioned here\n").expect("written");

    let search = search_within(&root, "beta", Budget::default());

    let files: Vec<&str> = search
        .usages
        .iter()
        .map(|usage| usage.file.as_str())
        .collect();
    assert!(
        files.iter().any(|file| file.ends_with("declares.rs")),
        "the file that declares the name answers: {files:?}"
    );
    assert!(
        files.iter().any(|file| file.ends_with("calls.rs")),
        "the file that calls the name answers: {files:?}"
    );
    assert!(
        !files.iter().any(|file| file.ends_with("mentions.rs")),
        "a mention in a comment is not a usage: {files:?}"
    );
}
