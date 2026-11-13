//! The files the `analyze` report samples for its complexity section.
//!
//! `cognicode analyze <dir>` ends with a `=== Complexity Analysis ===` block
//! that measures a handful of the `.rs` files directly under `<dir>`. The
//! handful used to be the first five names the operating system listed, so
//! the same project printed a different block on another filesystem, and
//! after the files were copied. A report that changes with the directory
//! order is not a report of the project.
//!
//! The sample now comes from the source inventory, which walks in file-name
//! order: the same tree names the same five files everywhere.

use std::path::PathBuf;
use std::process::{Command, Output};

use crate::common;

fn run(args: &[&str]) -> Output {
    Command::new(common::binary_path("cognicode"))
        .args(args)
        .output()
        .expect("spawn cognicode binary")
}

/// The lines of the complexity block, without the heading.
fn complexity_block(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .skip_while(|line| !line.contains("=== Complexity Analysis ==="))
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect()
}

fn workspace_with(names: &[&str]) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    // Created in name order: a filesystem that lists the newest entry first
    // hands the old sample `g, f, e, d, c`.
    for name in names {
        std::fs::write(
            root.join(name),
            "pub fn pick(x: u8) -> u8 { if x > 0 { x } else { 0 } }\n",
        )
        .expect("write");
    }
    (tmp, root)
}

/// Seven files, five slots: the five that sort first are the ones measured.
#[test]
fn the_sample_is_the_first_five_files_by_name() {
    let (_tmp, root) = workspace_with(&["a.rs", "b.rs", "c.rs", "d.rs", "e.rs", "f.rs", "g.rs"]);

    let out = run(&["analyze", root.to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();

    let names: Vec<String> = complexity_block(&stdout)
        .iter()
        .filter_map(|line| line.split(':').next().map(str::to_string))
        .collect();
    assert_eq!(
        names,
        vec!["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"],
        "the sample must not depend on the directory order: {stdout}"
    );
}

/// Only the files directly under the directory are sampled, and only `.rs`.
#[test]
fn a_nested_file_and_a_non_source_file_are_not_in_the_sample() {
    let (_tmp, root) = workspace_with(&["a.rs"]);
    std::fs::write(root.join("notes.md"), "# notes\n").expect("write");
    std::fs::create_dir_all(root.join("inner")).expect("dir");
    std::fs::write(root.join("inner/deep.rs"), "pub fn deep() {}\n").expect("write");

    let out = run(&["analyze", root.to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();

    let block = complexity_block(&stdout).join("\n");
    assert!(
        block.contains("a.rs:"),
        "the top-level source is measured: {stdout}"
    );
    assert!(
        !block.contains("deep.rs") && !block.contains("notes.md"),
        "a nested or non-source file is not in the sample: {stdout}"
    );
}
