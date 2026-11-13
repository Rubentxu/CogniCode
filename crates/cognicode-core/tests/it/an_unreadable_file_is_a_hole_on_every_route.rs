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
//! The same fixture on the three routes that publish a channel for it: the
//! light index (`holes`), the semantic index
//! (`populate_from_directory_with_holes`) and the per-file graph (the
//! `Partial` status). The premise is observed before the claim: the file
//! really is unreadable, and a tree without such a file reports no hole, so
//! the channel is not simply always full.
//!
//! The permission is restored when the test ends, so the temporary directory
//! can be removed.

#![cfg(unix)]

use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::value_objects::SymbolSearchQuery;
use cognicode_core::infrastructure::graph::LightweightIndex;
use cognicode_core::infrastructure::graph::per_file_graph::{BuildStatus, SkipReason, SkippedFile};
use cognicode_core::infrastructure::semantic::semantic_search::SemanticSearchService;
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
