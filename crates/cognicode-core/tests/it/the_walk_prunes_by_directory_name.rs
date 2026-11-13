//! Which directory names the walk prunes, and at which depth.
//!
//! The walk every route over the tree uses (`source_files::walk`) asks
//! `WalkFilter` about every entry, and the filter compares the NAME of each
//! component below the project root with a fixed list: `target`,
//! `node_modules`, `build`, `dist`, `vendor`, `env`, `coverage`, `secrets`,
//! `credentials`, ... A directory with one of those names is not read, at the
//! root of the project and at any depth below it.
//!
//! ## What this file pins
//!
//! The behaviour as it is, so that changing it is a decision and not an
//! accident:
//!
//!   - the names are pruned at the root and nested at any depth;
//!   - the name is compared whole and case-sensitively (`builder`, `Build`,
//!     `environment` are walked);
//!   - only the components BELOW the root count: a project that lives under a
//!     directory called `build` is still walked;
//!   - the pruning is a decision by NAME, not by content: a source module that
//!     happens to be called `env` is pruned like a virtual environment, and
//!     the inventory says nothing about it (no declined entry, no error).
//!
//! ## The decision (TASK-244)
//!
//! KEEP. The list stays as it is. Measured over the corpora of `sandbox/`,
//! the cost of the by-name rule is real but small: `clap_complete/src/env`
//! (2 Rust files of 330 in the repository) and `chalk/source/vendor` (6 files)
//! are source that the graph never sees, and the answer for those trees is
//! `complete`. Narrowing the list changes the set of files of every project
//! that has one of those names, and with it the digest of its manifest and
//! the identity of its snapshot: that is a published change and it belongs to
//! the maintainer, with an old-consumer test, not to a characterization.
//! What this file does not accept is the silence being invisible: the last
//! test says so, and the follow-up is a task of its own.

use cognicode_core::infrastructure::graph::source_files::inventory;
use std::path::Path;

/// The names of the performance skips and the security blocklist that are
/// ordinary directory names (the dot-names are hidden entries and the walk
/// declines those on its own).
const PRUNED_NAMES: [&str; 11] = [
    "target",
    "node_modules",
    "venv",
    "env",
    "dist",
    "build",
    "vendor",
    "coverage",
    "__pycache__",
    "secrets",
    "credentials",
];

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("directory");
    std::fs::write(path, contents).expect("file");
}

/// The files of the inventory, relative to `root`, in the walk's order.
fn relative_files(root: &Path) -> Vec<String> {
    inventory(root)
        .files
        .iter()
        .map(|file| {
            file.path
                .strip_prefix(root)
                .expect("the walk stays under the root")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

#[test]
fn a_skipped_name_is_pruned_at_the_root_and_at_any_depth() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    write(&root.join("src/lib.rs"), "pub fn kept() {}\n");
    for name in PRUNED_NAMES {
        write(&root.join(name).join("at_root.rs"), "pub fn x() {}\n");
        write(
            &root.join("src").join(name).join("nested.rs"),
            "pub fn x() {}\n",
        );
        write(
            &root.join("src/deep/er").join(name).join("deeper.rs"),
            "pub fn x() {}\n",
        );
    }

    assert_eq!(
        relative_files(root),
        vec!["src/lib.rs".to_string()],
        "every directory with a skipped name is pruned, whatever its depth"
    );
}

#[test]
fn a_pruned_directory_is_pruned_with_everything_below_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    write(&root.join("src/lib.rs"), "pub fn kept() {}\n");
    write(
        &root.join("src/vendor/a/b/c/inside.rs"),
        "pub fn inside() {}\n",
    );

    assert_eq!(
        relative_files(root),
        vec!["src/lib.rs".to_string()],
        "a name below a pruned directory is not looked at, whatever it is called"
    );
}

#[test]
fn the_name_is_compared_whole_and_case_sensitively() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    for name in [
        "builder",
        "rebuild",
        "environment",
        "vendors",
        "targets",
        "Build",
        "ENV",
        "distinct",
        "coverages",
    ] {
        write(
            &root.join("src").join(name).join("mod.rs"),
            "pub fn x() {}\n",
        );
    }

    let found = relative_files(root);
    assert_eq!(
        found.len(),
        9,
        "a name that merely contains a skipped name is walked: {found:?}"
    );
}

#[test]
fn a_project_that_lives_under_a_skipped_name_is_still_walked() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("build").join("vendor").join("project");
    write(&project.join("src/lib.rs"), "pub fn kept() {}\n");
    write(&project.join("vendor/lost.rs"), "pub fn lost() {}\n");

    assert_eq!(
        relative_files(&project),
        vec!["src/lib.rs".to_string()],
        "only the names below the root say what the project does not contain"
    );
}

/// The silence, pinned. A source module called like a build directory is not
/// part of the graph, and nothing in the inventory says so: there is no
/// declined entry and no error, so a route that publishes coverage from the
/// inventory publishes `complete` over a tree it did not read whole.
///
/// This is a documented limitation (see the README), kept on purpose: naming
/// each pruned directory as declined would make the coverage `partial` for a
/// fifth of the sandbox corpus on build output alone. If the walk ever says
/// what it pruned, this test changes with it.
#[test]
fn a_source_module_called_like_build_output_is_dropped_without_a_word() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    write(&root.join("src/lib.rs"), "pub mod env;\n");
    write(&root.join("src/env/mod.rs"), "pub fn load() {}\n");

    let found = inventory(root);

    let files: Vec<_> = found
        .files
        .iter()
        .map(|f| {
            f.path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(files, vec!["src/lib.rs".to_string()], "env/ is pruned");
    assert!(
        found.declined.is_empty() && found.errors.is_empty(),
        "the inventory does not name the pruned directory: declined {:?}, errors {:?}",
        found.declined,
        found.errors
    );
}
