//! An unreadable file is a hole on every route that reads the tree.
//!
//! ## The gap
//!
//! The inventory lists a file it can stat. Whether the file can be READ is
//! only known to whoever opens it, and two of the routes that open it dropped
//! the file without saying so: the light index skipped a failed read and went
//! on, and the semantic index did the same. A tree with one file nobody could
//! read looked, through those two, like a tree that was fully indexed.
//!
//! ## What this file pins
//!
//! The same fixture on the routes that publish a channel for it: the light
//! index (`holes`), the semantic index (`populate_from_directory_with_holes`),
//! the per-file graph (the `Partial` status) and the usage search (the
//! coverage of `find_usages_with_coverage`, and the `coverage` the MCP
//! handler publishes with it). The premise is observed before the claim: the
//! file really is unreadable, and a tree without such a file reports no hole,
//! so the channel is not simply always full.
//!
//! The permission is restored when the test ends, so the temporary directory
//! can be removed.

#![cfg(unix)]

use cognicode_core::application::services::analysis_service::{AnalysisService, UsageSearchParams};
use cognicode_core::domain::value_objects::SymbolSearchQuery;
use cognicode_core::infrastructure::graph::LightweightIndex;
use cognicode_core::infrastructure::graph::per_file_graph::{BuildStatus, SkipReason, SkippedFile};
use cognicode_core::infrastructure::semantic::semantic_search::SemanticSearchService;
use cognicode_core::interface::mcp::handlers::{HandlerContext, handle_find_usages};
use cognicode_core::interface::mcp::schemas::FindUsagesInput;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const READABLE: &str = "readable_symbol";
const LOCKED: &str = "locked_symbol";

/// Gives the file its permissions back when the test is over, however it ends.
struct Unlock(PathBuf);

impl Drop for Unlock {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o644));
    }
}

fn write_sources(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("the directory is created");
    std::fs::write(
        root.join("src/lib.rs"),
        format!("pub fn {READABLE}() -> u32 {{ 1 }}\n"),
    )
    .expect("the readable source is written");
}

/// A tree whose every file can be read.
fn a_readable_tree() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("a temporary directory");
    let root = tmp.path().join("project");
    write_sources(&root);
    (tmp, root)
}

/// A tree with one file that can be listed and measured but not read.
fn a_tree_with_one_unreadable_file() -> (tempfile::TempDir, PathBuf, Unlock) {
    let (tmp, root) = a_readable_tree();
    let locked = root.join("src/locked.rs");
    std::fs::write(&locked, format!("pub fn {LOCKED}() -> u32 {{ 2 }}\n"))
        .expect("the locked source is written");
    let unlock = Unlock(locked.clone());
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000))
        .expect("the file is locked");
    let error = std::fs::read_to_string(&locked).expect_err("the premise: the file cannot be read");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    (tmp, root, unlock)
}

/// The holes that name the locked file, as a read failure.
fn read_failures_of_the_locked_file(holes: &[SkippedFile]) -> Vec<&SkippedFile> {
    holes
        .iter()
        .filter(|hole| hole.path.ends_with("src/locked.rs"))
        .filter(|hole| matches!(hole.reason, SkipReason::Read(_)))
        .collect()
}

#[test]
fn the_light_index_publishes_the_file_it_could_not_read() {
    let (_tmp, root, _unlock) = a_tree_with_one_unreadable_file();
    let mut index = LightweightIndex::new();
    index.build_index(&root).expect("the index builds");

    assert_eq!(
        read_failures_of_the_locked_file(index.holes()).len(),
        1,
        "the file could not be read and the index does not say so: {:?}",
        index.holes()
    );
    let names: Vec<&str> = index.symbols().collect();
    assert!(names.contains(&READABLE), "the readable file is indexed");
    assert!(
        !names.contains(&LOCKED),
        "the unreadable file cannot have contributed a symbol"
    );
}

#[test]
fn the_light_index_of_a_readable_tree_has_no_holes() {
    let (_tmp, root) = a_readable_tree();
    let mut index = LightweightIndex::new();
    index.build_index(&root).expect("the index builds");

    assert!(
        index.holes().is_empty(),
        "nothing was left out and the index reports holes: {:?}",
        index.holes()
    );
}

#[test]
fn the_semantic_index_publishes_the_file_it_could_not_read() {
    let (_tmp, root, _unlock) = a_tree_with_one_unreadable_file();
    let service = SemanticSearchService::new();
    let holes = service
        .populate_from_directory_with_holes(&root)
        .expect("the semantic index is populated");

    assert_eq!(
        read_failures_of_the_locked_file(&holes).len(),
        1,
        "the file could not be read and the index does not say so: {holes:?}"
    );
    let found = |name: &str| {
        service
            .search(SymbolSearchQuery {
                query: name.to_string(),
                kinds: Vec::new(),
                max_results: 20,
            })
            .iter()
            .any(|hit| hit.symbol.name() == name)
    };
    assert!(found(READABLE), "the readable file is indexed");
    assert!(
        !found(LOCKED),
        "the unreadable file cannot have contributed a symbol"
    );
}

#[test]
fn the_semantic_index_of_a_readable_tree_has_no_holes() {
    let (_tmp, root) = a_readable_tree();
    let service = SemanticSearchService::new();
    let holes = service
        .populate_from_directory_with_holes(&root)
        .expect("the semantic index is populated");

    assert!(
        holes.is_empty(),
        "nothing was left out and the index reports holes: {holes:?}"
    );
}

#[test]
fn the_per_file_graph_is_partial_over_the_file_it_could_not_read() {
    let (_tmp, root, _unlock) = a_tree_with_one_unreadable_file();
    let service = AnalysisService::new();
    let report = service
        .build_graph_per_file_with_report(&root)
        .expect("the per-file graph builds");

    match report.status {
        BuildStatus::Partial { skipped, .. } => assert_eq!(
            read_failures_of_the_locked_file(&skipped).len(),
            1,
            "the build is partial but not over the file that could not be read: {skipped:?}"
        ),
        BuildStatus::Complete => {
            panic!("a file could not be read and the build says it is complete")
        }
    }
}

fn usage_search_of(root: &Path, symbol: &str) -> UsageSearchParams {
    UsageSearchParams {
        project_dir: root.to_path_buf(),
        symbol_name: symbol.to_string(),
        include_declaration: true,
        context_lines: None,
        first_only_definition: true,
    }
}

#[test]
fn the_usage_search_is_partial_over_the_file_it_could_not_read() {
    let (_tmp, root, _unlock) = a_tree_with_one_unreadable_file();
    let search = AnalysisService::new()
        .find_symbol_usages_with_coverage(usage_search_of(&root, READABLE))
        .expect("the usage search runs");

    assert!(
        search
            .usages
            .iter()
            .any(|usage| usage.file.ends_with("src/lib.rs")),
        "the readable file still answers: {:?}",
        search.usages
    );
    assert_eq!(
        search.coverage.published_state().published_label(),
        "partial",
        "a file could not be read and the search says it covered the tree"
    );
    let facts = search.coverage.facts();
    assert_eq!(
        read_failures_of_the_locked_file(&facts.skipped).len(),
        1,
        "the file could not be read and the search does not name it: {:?}",
        facts.skipped
    );
}

#[test]
fn the_usage_search_of_a_readable_tree_is_complete() {
    let (_tmp, root) = a_readable_tree();
    let search = AnalysisService::new()
        .find_symbol_usages_with_coverage(usage_search_of(&root, READABLE))
        .expect("the usage search runs");

    assert_eq!(
        search.coverage.published_state().published_label(),
        "complete",
        "nothing was left out and the search reports a hole: {:?}",
        search.coverage.facts().skipped
    );
    assert!(
        search.coverage.facts().skipped.is_empty(),
        "a complete coverage names no file"
    );
}

#[test]
fn the_usage_search_of_a_tree_with_nothing_to_read_is_not_complete() {
    let tmp = tempfile::tempdir().expect("a temporary directory");
    let search = AnalysisService::new()
        .find_symbol_usages_with_coverage(usage_search_of(tmp.path(), READABLE))
        .expect("the usage search runs");

    assert!(search.usages.is_empty());
    assert_eq!(
        search.coverage.published_state().published_label(),
        "unsupported",
        "no usages in a tree with nothing this product can read is not `no usages`"
    );
}

#[tokio::test]
async fn find_usages_publishes_the_file_it_could_not_read() {
    let (_tmp, root, _unlock) = a_tree_with_one_unreadable_file();
    let ctx = HandlerContext::builder().with_working_dir(root).build();

    let out = handle_find_usages(
        &ctx,
        FindUsagesInput {
            symbol_name: READABLE.to_string(),
            include_declaration: true,
            context_lines: None,
        },
    )
    .await
    .expect("the handler runs");

    assert_eq!(
        out.coverage.status, "partial",
        "a file could not be read and the answer says it covered the tree"
    );
    let locked: Vec<_> = out
        .coverage
        .skipped_files
        .iter()
        .filter(|file| file.path.ends_with("src/locked.rs"))
        .collect();
    assert_eq!(
        locked.len(),
        1,
        "the file could not be read and the answer does not name it: {:?}",
        out.coverage.skipped_files
    );
    assert!(
        locked[0].reason.starts_with("read error"),
        "the hole carries its cause: {}",
        locked[0].reason
    );
    assert!(out.total >= 1, "the readable file still answers");
}

#[tokio::test]
async fn find_usages_of_a_readable_tree_publishes_a_complete_coverage() {
    let (_tmp, root) = a_readable_tree();
    let ctx = HandlerContext::builder().with_working_dir(root).build();

    let out = handle_find_usages(
        &ctx,
        FindUsagesInput {
            symbol_name: READABLE.to_string(),
            include_declaration: true,
            context_lines: None,
        },
    )
    .await
    .expect("the handler runs");

    assert_eq!(out.coverage.status, "complete");
    assert!(out.coverage.skipped_files.is_empty());
}

/// An answer written before the coverage existed carries no `coverage`. The
/// consumer that reads it must get the word "unknown": a missing field is not
/// a claim that the tree was covered.
#[tokio::test]
async fn an_answer_written_before_the_coverage_existed_reads_as_unknown() {
    let (_tmp, root) = a_readable_tree();
    let ctx = HandlerContext::builder().with_working_dir(root).build();
    let out = handle_find_usages(
        &ctx,
        FindUsagesInput {
            symbol_name: READABLE.to_string(),
            include_declaration: true,
            context_lines: None,
        },
    )
    .await
    .expect("the handler runs");

    let mut old_payload = serde_json::to_value(&out).expect("the answer serializes");
    let removed = old_payload
        .as_object_mut()
        .expect("the answer is an object")
        .remove("coverage");
    assert!(
        removed.is_some(),
        "the premise: the answer carries a coverage"
    );

    let read: cognicode_core::interface::mcp::schemas::FindUsagesOutput =
        serde_json::from_value(old_payload).expect("an old payload still deserializes");

    assert_eq!(read.coverage.status, "unknown");
    assert!(read.coverage.skipped_files.is_empty());
    assert_eq!(read.total, out.total, "the rest of the answer is untouched");
}
