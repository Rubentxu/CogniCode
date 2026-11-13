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
//! ## The file operations (TASK-247)
//!
//! `search_content` and `list_files` walk the tree with a walker of their own
//! and used to answer an entry they could not read with a bare `continue`: a
//! search over a tree with a file nobody could open read as a search over the
//! whole tree. They publish the same `coverage` the usage search does. What
//! each one can fail on is not the same, and the tests say which is which:
//!
//! | what the tree has            | `search_content`          | `list_files`                     |
//! |------------------------------|---------------------------|----------------------------------|
//! | a file with mode 000         | hole (it opens the file)  | listed (it only measures it)     |
//! | a directory with mode 000    | hole (the walk fails)     | hole (the walk fails)            |
//! | a symlink to nothing         | not a file, not searched  | hole (the measurement fails)     |
//!
//! These answers are read as JSON, the way a client reads them. The set of
//! files each operation reads is the one it always read: the characterization
//! in `each_reader_of_the_tree_says_which_files_it_reads` stays as it was.
//!
//! The permission is restored when the test ends, so the temporary directory
//! can be removed.

#![cfg(unix)]

use cognicode_core::application::services::analysis_service::{AnalysisService, UsageSearchParams};
use cognicode_core::domain::value_objects::SymbolSearchQuery;
use cognicode_core::infrastructure::graph::LightweightIndex;
use cognicode_core::infrastructure::graph::per_file_graph::{BuildStatus, SkipReason, SkippedFile};
use cognicode_core::infrastructure::semantic::semantic_search::SemanticSearchService;
use cognicode_core::interface::mcp::file_ops_handlers::{handle_list_files, handle_search_content};
use cognicode_core::interface::mcp::handlers::{HandlerContext, handle_find_usages};
use cognicode_core::interface::mcp::schemas::{
    FindUsagesInput, ListFilesInput, ListFilesOutput, SearchContentInput, SearchContentOutput,
};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const READABLE: &str = "readable_symbol";
const LOCKED: &str = "locked_symbol";

/// Gives the path its permissions back when the test is over, however it ends.
struct Unlock(PathBuf, u32);

impl Drop for Unlock {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(self.1));
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
    let unlock = Unlock(locked.clone(), 0o644);
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

// ---------------------------------------------------------------------------
// The file operations: `search_content` and `list_files`
// ---------------------------------------------------------------------------

/// A tree with a directory nobody can list. A symlink to nothing sits beside
/// it, for the measurement the listing makes of every entry.
fn a_tree_with_one_sealed_directory() -> (tempfile::TempDir, PathBuf, Unlock) {
    let (tmp, root) = a_readable_tree();
    let sealed = root.join("sealed");
    std::fs::create_dir_all(&sealed).expect("the directory is created");
    std::fs::write(
        sealed.join("inside.rs"),
        format!("pub fn {LOCKED}() -> u32 {{ 3 }}\n"),
    )
    .expect("the hidden source is written");
    let unlock = Unlock(sealed.clone(), 0o755);
    std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o000))
        .expect("the directory is sealed");
    std::fs::read_dir(&sealed).expect_err("the premise: the directory cannot be listed");
    (tmp, root, unlock)
}

/// A tree with one entry the walk lists and the measurement cannot follow.
fn a_tree_with_a_symlink_to_nothing() -> (tempfile::TempDir, PathBuf) {
    let (tmp, root) = a_readable_tree();
    std::os::unix::fs::symlink(root.join("missing.rs"), root.join("dangling.rs"))
        .expect("the symlink is created");
    std::fs::metadata(root.join("dangling.rs"))
        .expect_err("the premise: the symlink points at nothing");
    (tmp, root)
}

fn search_for(pattern: &str) -> SearchContentInput {
    SearchContentInput {
        pattern: pattern.to_string(),
        path: None,
        file_glob: None,
        regex: Some(false),
        case_insensitive: None,
        max_results: None,
        context_lines: Some(0),
    }
}

fn list_everything() -> ListFilesInput {
    ListFilesInput {
        path: None,
        glob: None,
        offset: None,
        limit: Some(1000),
        recursive: None,
        max_depth: None,
    }
}

async fn search_answer(root: &Path, pattern: &str) -> serde_json::Value {
    let ctx = HandlerContext::builder()
        .with_working_dir(root.to_path_buf())
        .build();
    let out = handle_search_content(&ctx, search_for(pattern))
        .await
        .expect("the search runs");
    serde_json::to_value(&out).expect("the answer serializes")
}

async fn listing_answer(root: &Path) -> serde_json::Value {
    let ctx = HandlerContext::builder()
        .with_working_dir(root.to_path_buf())
        .build();
    let out = handle_list_files(&ctx, list_everything())
        .await
        .expect("the listing runs");
    serde_json::to_value(&out).expect("the answer serializes")
}

/// The word of the coverage an answer carries, as a client reads it.
fn status_of(answer: &serde_json::Value) -> &str {
    answer["coverage"]["status"]
        .as_str()
        .unwrap_or("<no coverage in the answer>")
}

/// The holes an answer names, as `(path, reason)`.
fn holes_of(answer: &serde_json::Value) -> Vec<(String, String)> {
    answer["coverage"]["skipped_files"]
        .as_array()
        .map(|holes| {
            holes
                .iter()
                .map(|hole| {
                    (
                        hole["path"].as_str().unwrap_or_default().to_string(),
                        hole["reason"].as_str().unwrap_or_default().to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn holes_ending_with<'a>(holes: &'a [(String, String)], suffix: &str) -> Vec<&'a (String, String)> {
    holes
        .iter()
        .filter(|(path, _)| path.ends_with(suffix))
        .collect()
}

fn matched_files(answer: &serde_json::Value) -> Vec<String> {
    answer["matches"]
        .as_array()
        .expect("the answer carries its matches")
        .iter()
        .map(|m| m["file"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio::test]
async fn the_search_publishes_the_file_it_could_not_read() {
    let (_tmp, root, _unlock) = a_tree_with_one_unreadable_file();
    let answer = search_answer(&root, "pub fn").await;

    let files = matched_files(&answer);
    assert!(
        files.iter().any(|f| f.ends_with("src/lib.rs")),
        "the readable file still answers: {files:?}"
    );
    assert_eq!(
        status_of(&answer),
        "partial",
        "a file could not be opened and the search says it covered the tree"
    );
    let holes = holes_of(&answer);
    let locked = holes_ending_with(&holes, "src/locked.rs");
    assert_eq!(
        locked.len(),
        1,
        "the file could not be opened and the search does not name it: {holes:?}"
    );
    assert!(
        locked[0].1.starts_with("read error"),
        "the hole carries its cause: {}",
        locked[0].1
    );
    assert_eq!(
        answer["files_scanned"], 2,
        "the set of files the search opens is the one it always opened: the locked file counts"
    );
}

#[tokio::test]
async fn the_search_publishes_the_directory_it_could_not_list() {
    let (_tmp, root, _unlock) = a_tree_with_one_sealed_directory();
    let answer = search_answer(&root, "pub fn").await;

    assert_eq!(
        status_of(&answer),
        "partial",
        "a directory could not be listed and the search says it covered the tree"
    );
    let holes = holes_of(&answer);
    let sealed = holes_ending_with(&holes, "sealed");
    assert_eq!(
        sealed.len(),
        1,
        "the directory could not be listed and the search does not name it: {holes:?}"
    );
    assert!(
        sealed[0].1.starts_with("walk error"),
        "the hole carries its cause: {}",
        sealed[0].1
    );
    assert!(
        matched_files(&answer)
            .iter()
            .any(|f| f.ends_with("src/lib.rs")),
        "the readable file still answers"
    );
}

#[tokio::test]
async fn the_search_of_a_tree_it_read_whole_is_complete() {
    let (_tmp, root) = a_readable_tree();
    let answer = search_answer(&root, "pub fn").await;

    assert_eq!(
        status_of(&answer),
        "complete",
        "nothing was left out and the search reports a hole: {:?}",
        holes_of(&answer)
    );
    assert!(
        holes_of(&answer).is_empty(),
        "a complete answer names no file"
    );
    assert_eq!(answer["total"], 1);
}

/// A file that opens and is not text has nothing a text search could match: it
/// is the same case as a file with a binary extension, which the search skips
/// without a word. Calling it a hole would degrade to `partial` every search
/// over a tree that keeps one image or one compiled object under a source
/// extension, and the caller could do nothing about it.
#[tokio::test]
async fn the_search_does_not_call_a_file_that_is_not_text_a_hole() {
    let (_tmp, root) = a_readable_tree();
    let blob = root.join("src").join("blob.rs");
    std::fs::write(&blob, [0xff_u8, 0xfe, 0x00, 0xc3, 0x28, 0x70, 0x75, 0x62])
        .expect("the non-text file is written");
    std::fs::read_to_string(&blob).expect_err("the premise: the file opens and is not valid text");

    let answer = search_answer(&root, "pub fn").await;

    assert_eq!(
        status_of(&answer),
        "complete",
        "a file that is not text is not a file the search could not read: {:?}",
        holes_of(&answer)
    );
    assert!(
        holes_of(&answer).is_empty(),
        "a complete answer names no file"
    );
    assert_eq!(answer["total"], 1, "the readable file still answers");
}

#[tokio::test]
async fn the_listing_publishes_the_directory_it_could_not_list() {
    let (_tmp, root, _unlock) = a_tree_with_one_sealed_directory();
    let answer = listing_answer(&root).await;

    assert_eq!(
        status_of(&answer),
        "partial",
        "a directory could not be listed and the listing says it covered the tree"
    );
    let holes = holes_of(&answer);
    let sealed = holes_ending_with(&holes, "sealed");
    assert_eq!(
        sealed.len(),
        1,
        "the directory could not be listed and the listing does not name it: {holes:?}"
    );
    assert!(
        sealed[0].1.starts_with("walk error"),
        "the hole carries its cause: {}",
        sealed[0].1
    );
}

/// The listing measures a file and never opens it: a file nobody can read is
/// listed, and the answer is not degraded for it.
#[tokio::test]
async fn the_listing_does_not_call_a_file_it_only_measures_a_hole() {
    let (_tmp, root, _unlock) = a_tree_with_one_unreadable_file();
    let answer = listing_answer(&root).await;

    let listed: Vec<&str> = answer["files"]
        .as_array()
        .expect("the answer carries its files")
        .iter()
        .filter_map(|f| f["path"].as_str())
        .collect();
    assert!(
        listed.iter().any(|p| p.ends_with("src/locked.rs")),
        "the locked file is listed, as it always was: {listed:?}"
    );
    assert_eq!(status_of(&answer), "complete", "{:?}", holes_of(&answer));
}

#[tokio::test]
async fn the_listing_publishes_the_entry_it_could_not_measure() {
    let (_tmp, root) = a_tree_with_a_symlink_to_nothing();
    let answer = listing_answer(&root).await;

    let listed: Vec<&str> = answer["files"]
        .as_array()
        .expect("the answer carries its files")
        .iter()
        .filter_map(|f| f["path"].as_str())
        .collect();
    assert!(
        !listed.iter().any(|p| p.ends_with("dangling.rs")),
        "the entry is not listed, as it never was: {listed:?}"
    );
    assert_eq!(
        status_of(&answer),
        "partial",
        "an entry of the directory is missing from the listing and the listing says it is whole"
    );
    let holes = holes_of(&answer);
    let dangling = holes_ending_with(&holes, "dangling.rs");
    assert_eq!(
        dangling.len(),
        1,
        "the entry could not be measured and the listing does not name it: {holes:?}"
    );
    assert!(
        dangling[0].1.starts_with("read error"),
        "the hole carries its cause: {}",
        dangling[0].1
    );
}

#[tokio::test]
async fn the_listing_of_a_tree_it_read_whole_is_complete() {
    let (_tmp, root) = a_readable_tree();
    let answer = listing_answer(&root).await;

    assert_eq!(
        status_of(&answer),
        "complete",
        "nothing was left out and the listing reports a hole: {:?}",
        holes_of(&answer)
    );
    assert!(
        holes_of(&answer).is_empty(),
        "a complete answer names no file"
    );
}

/// An answer written before the coverage existed carries none. The consumer
/// that reads it gets "unknown": a missing field is not a claim that the tree
/// was covered. The fields the old answer had are read as they always were.
#[tokio::test]
async fn an_answer_of_the_file_operations_written_before_the_coverage_reads_as_unknown() {
    let search_before = serde_json::json!({
        "matches": [{ "file": "a.rs", "line": 1, "col": 1, "text": "x", "context": [] }],
        "total": 1,
        "files_scanned": 3,
    });
    let read: SearchContentOutput =
        serde_json::from_value(search_before).expect("an old search payload still deserializes");
    let read = serde_json::to_value(&read).expect("it serializes");
    assert_eq!(status_of(&read), "unknown");
    assert!(holes_of(&read).is_empty());
    assert_eq!(read["total"], 1);
    assert_eq!(read["files_scanned"], 3);

    let listing_before = serde_json::json!({
        "files": [],
        "total": 0,
        "depth_traversed": 1,
    });
    let read: ListFilesOutput =
        serde_json::from_value(listing_before).expect("an old listing payload still deserializes");
    let read = serde_json::to_value(&read).expect("it serializes");
    assert_eq!(status_of(&read), "unknown");
    assert!(holes_of(&read).is_empty());
    assert_eq!(read["depth_traversed"], 1);
}

/// The holes of the application's answer are additive too: a result that was
/// read whole serializes as it always did, and an old one still reads.
#[test]
fn the_application_answers_carry_their_holes_only_when_they_have_some() {
    use cognicode_core::application::dto::{ListFilesResult, SearchContentResult};

    let search_before = r#"{"matches":[],"total":0,"files_scanned":0}"#;
    let search: SearchContentResult =
        serde_json::from_str(search_before).expect("an old search result still deserializes");
    let json = serde_json::to_value(&search).expect("it serializes");
    assert!(
        json.get("unread").is_none(),
        "a result with nothing unread is the result it always was: {json}"
    );

    let listing_before = r#"{"files":[],"total":0}"#;
    let listing: ListFilesResult =
        serde_json::from_str(listing_before).expect("an old listing result still deserializes");
    let json = serde_json::to_value(&listing).expect("it serializes");
    assert!(
        json.get("unread").is_none(),
        "a result with nothing unread is the result it always was: {json}"
    );
}
