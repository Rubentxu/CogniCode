//! `cognicode find-usages` says which files it could not search.
//!
//! "No usages found" over a file nobody could open is not a finding, it is a
//! hole published as a zero. The search reads every source file of the tree;
//! when one cannot be read, the answer has to name it, in the terminal a
//! person reads and in the document a script parses.
//!
//! The binary is run as a sub-process, over a tree built in a temporary
//! directory, with one file whose permissions forbid reading it. The core
//! suite measures the same fact at the service and at the MCP handler; this
//! file measures what the CLI's two renderers do with it.

use crate::common;

use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const READABLE: &str = "readable_symbol";
const LOCKED: &str = "locked_symbol";

/// Gives the file its permissions back when the test is over, however it ends.
struct Unlock(PathBuf);

impl Drop for Unlock {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o644));
    }
}

/// A tree whose every file can be read: one declaration and one call.
fn a_readable_tree() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("a temporary directory");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join("src")).expect("the directory is created");
    std::fs::write(
        root.join("src/lib.rs"),
        format!("pub fn {READABLE}() -> u32 {{ 1 }}\npub fn caller() -> u32 {{ {READABLE}() }}\n"),
    )
    .expect("the readable source is written");
    (tmp, root)
}

/// The same tree, plus a file the search can list but not read.
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

/// What the CLI wrote to the standard output.
fn find_usages(symbol: &str, root: &Path, format: &str) -> String {
    let binary = common::binary_path("cognicode");
    let cache = tempfile::tempdir().expect("a cache of its own");
    let output = Command::new(&binary)
        .args(["find-usages", symbol, "-C"])
        .arg(root)
        .args(["-f", format])
        .env("XDG_CACHE_HOME", cache.path())
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", binary.display()));
    assert!(
        output.status.success(),
        "the CLI exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("the answer is text")
}

fn find_usages_document(symbol: &str, root: &Path) -> Value {
    let text = find_usages(symbol, root, "json");
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("the CLI did not answer with one JSON document: {e}: {text}"))
}

#[test]
fn the_terminal_names_the_file_it_could_not_read() {
    let (_tmp, root, _unlock) = a_tree_with_one_unreadable_file();

    let text = find_usages(READABLE, &root, "text");

    assert!(
        text.contains("src/lib.rs"),
        "the usages of the readable file are listed: {text}"
    );
    assert!(
        text.lines().any(|line| line == "coverage: partial"),
        "the answer does not say it is partial: {text}"
    );
    let holes: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("  not searched: "))
        .collect();
    assert_eq!(holes.len(), 1, "one file was not searched: {text}");
    assert!(
        holes[0].contains("src/locked.rs"),
        "the hole names the file: {}",
        holes[0]
    );
}

#[test]
fn an_empty_answer_over_a_hole_does_not_claim_the_tree_is_clean() {
    let (_tmp, root, _unlock) = a_tree_with_one_unreadable_file();

    let text = find_usages(LOCKED, &root, "text");

    assert!(
        text.contains("(no usages found in what could be searched)"),
        "an empty answer over a partial search says what it searched: {text}"
    );
    assert!(
        !text.contains("(no usages found)\n"),
        "an empty answer over a partial search must not read as a clean tree: {text}"
    );
    assert!(text.contains("coverage: partial"), "{text}");
}

#[test]
fn the_document_publishes_the_file_it_could_not_read() {
    let (_tmp, root, _unlock) = a_tree_with_one_unreadable_file();

    let document = find_usages_document(READABLE, &root);

    assert_eq!(document["coverage"]["status"], "partial", "{document}");
    let skipped = document["coverage"]["skipped_files"]
        .as_array()
        .unwrap_or_else(|| panic!("the coverage lists its skipped files: {document}"));
    assert_eq!(skipped.len(), 1, "{document}");
    assert!(
        skipped[0]["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("src/locked.rs")),
        "the hole names the file: {document}"
    );
    assert!(
        skipped[0]["reason"]
            .as_str()
            .is_some_and(|reason| !reason.is_empty()),
        "the hole says why: {document}"
    );
}

#[test]
fn a_tree_that_was_read_whole_says_nothing_extra() {
    let (_tmp, root) = a_readable_tree();

    let text = find_usages(READABLE, &root, "text");
    assert!(
        !text.contains("coverage:") && !text.contains("not searched:"),
        "a complete search adds no coverage lines to the terminal: {text}"
    );

    let document = find_usages_document(READABLE, &root);
    assert_eq!(document["coverage"]["status"], "complete", "{document}");
    assert_eq!(
        document["coverage"]["skipped_files"]
            .as_array()
            .map(Vec::len),
        Some(0),
        "{document}"
    );
}
