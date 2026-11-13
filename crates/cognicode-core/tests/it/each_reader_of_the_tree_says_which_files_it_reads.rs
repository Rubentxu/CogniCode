//! Which files each reader of a project tree reads, over ONE tree.
//!
//! The graph, the light index, the usage search and the manifest take their
//! files from `source_files::inventory`. Four readers keep a walk of their own
//! (`REGISTERED_TRAVERSALS` in `scripts/ci/test_ratchets.py`), and a walk of
//! its own is a set of files of its own: the same question gets another answer
//! and neither says so. This file measures the set each of them reads over a
//! tree with the awkward cases in it, and pins the DIFFERENCES, so that
//! aligning a reader is a decision with its published consequence in view and
//! not an accident of a refactor.
//!
//! ## The tree
//!
//! ```text
//! main.rs                  a source at the root
//! src/lib.rs               a source
//! src/inner/mod.rs         a source, nested
//! src/locked.rs            a source nobody can read (mode 000)
//! src/notes.md             not a source
//! build/generated.rs       build output named like one
//! vendor/dep.rs            vendored code
//! target/debug/out.rs      cargo output
//! .hidden/h.rs             a hidden directory
//! linked -> src            a symlink to a directory
//! ```
//!
//! ## What was measured (the `.rs` files each reader reads)
//!
//! | reader                               | root | src | locked | build | vendor | target | hidden | linked |
//! |--------------------------------------|------|-----|--------|-------|--------|--------|--------|--------|
//! | `inventory` (graph, index, usages)   | yes  | yes | listed | no    | no     | no     | no     | declined |
//! | `control_query::read_from_source_root` | yes | yes | DROPPED | yes  | yes    | no     | yes    | followed (twice) |
//! | `workspace_extractor` (export)       | yes  | yes | listed | no    | yes    | no     | yes    | followed (twice) |
//! | `file_operations::list_files`        | yes  | yes | listed | yes   | yes    | yes    | no     | not followed |
//!
//! The fifth reader, the complexity section of `cognicode complexity`
//! (`cli/commands.rs`), reads ONE level of the path it is given and takes five
//! names: it is a sample for the person, not a coverage claim, and it is
//! measured in the CLI suite.
//!
//! ## The decisions (TASK-243)
//!
//! * `file_operations` is not a reader of sources: it searches and lists ANY
//!   file (a README, a lockfile, a log) inside the workspace, and a project
//!   that asks "list `build/`" must be answered. It answers a different
//!   question and stays on its own walk.
//! * `workspace_extractor` publishes a digest over the entities it lists.
//!   Moving it to the inventory drops `vendor/`, `.hidden/` and the symlink
//!   from every export that has them, and changes the digest of each one: a
//!   published change that belongs to the maintainer, with the old consumer
//!   in view. It stays; the pin below is what changes with the decision.
//! * `control_query` has no consumer in THIS repository (no route, no CLI, no
//!   handler calls it), but it does have one outside it: the `cognicode-explorer`
//!   repository calls `read_from_source_root` from its control-plane endpoint
//!   (pinned to the `v0.102.1` tag). Removing it is a breaking change for that
//!   consumer (TASK-248 measured this and kept it), and the permanent test of
//!   its identity (`SPEC-CU-010`) lives with it. Its walk reads `build/`,
//!   `vendor/` and hidden directories and follows symlinks, and it dropped an
//!   unreadable file without a word. The silence is fixed here (the last
//!   tests); moving the walk to the inventory would make `application` import
//!   `infrastructure` from a new file, and the CR-06 allowlist only shrinks
//!   without a named approval.
//!
//! The permission is restored when the test ends, so the temporary directory
//! can be removed.
//!
//! ## The defect the measurement found, and its fix
//!
//! `control_query` skipped an unreadable file, and a directory it could not
//! list, with a bare `continue`: the read was smaller than the tree and
//! nothing in `SourceRead` said so. It now publishes them in `unreadable`
//! (relative path and reason), the same fact the inventory publishes as a
//! hole. The last two tests are the ones that failed before.

#![cfg(unix)]

use cognicode_core::application::architecture::control_query::read_from_source_root;
use cognicode_core::assurance_export::workspace_extractor::CAP_ENTITIES;
use cognicode_core::assurance_export::{
    ExtractionCapabilities, ProducerInfo, WorkspaceInfo, extract_from_workspace,
};
use cognicode_core::infrastructure::graph::source_files::inventory;
use cognicode_core::interface::mcp::file_ops_handlers::handle_list_files;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::schemas::ListFilesInput;
use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Gives the path its permissions back when the test is over, however it ends.
struct Unlock(PathBuf, u32);

impl Drop for Unlock {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(self.1));
    }
}

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("directory");
    std::fs::write(path, contents).expect("file");
}

/// The tree of the module documentation.
fn the_tree() -> (tempfile::TempDir, PathBuf, Unlock) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    write(&root.join("main.rs"), "fn main() {}\n");
    write(&root.join("src/lib.rs"), "pub fn kept() {}\n");
    write(&root.join("src/inner/mod.rs"), "pub fn inner() {}\n");
    write(&root.join("src/locked.rs"), "pub fn locked() {}\n");
    write(&root.join("src/notes.md"), "# notes\n");
    write(&root.join("build/generated.rs"), "pub fn generated() {}\n");
    write(&root.join("vendor/dep.rs"), "pub fn dep() {}\n");
    write(&root.join("target/debug/out.rs"), "pub fn out() {}\n");
    write(&root.join(".hidden/h.rs"), "pub fn h() {}\n");
    std::os::unix::fs::symlink(root.join("src"), root.join("linked")).expect("symlink");

    let locked = root.join("src/locked.rs");
    let unlock = Unlock(locked.clone(), 0o644);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("lock");
    let error = std::fs::read_to_string(&locked).expect_err("the premise: it cannot be read");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    (tmp, root, unlock)
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

fn rust_only(names: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    names.into_iter().filter(|n| n.ends_with(".rs")).collect()
}

#[test]
fn the_inventory_reads_the_sources_and_names_what_it_declined() {
    let (_tmp, root, _unlock) = the_tree();
    let found = inventory(&root);

    assert_eq!(
        rust_only(found.files.iter().map(|f| relative(&root, &f.path))),
        set(&["main.rs", "src/inner/mod.rs", "src/lib.rs", "src/locked.rs"]),
        "build/, vendor/, target/ and the hidden directory are pruned; the file nobody can \
         read is LISTED (it can be measured), and reading it is the route's business"
    );
    let declined: Vec<String> = found
        .declined
        .iter()
        .map(|d| relative(&root, Path::new(&d.path)))
        .collect();
    assert_eq!(
        declined,
        vec!["linked".to_string()],
        "the symlink is declined"
    );
}

#[test]
fn control_query_reads_what_the_inventory_prunes_and_follows_the_symlink() {
    let (_tmp, root, _unlock) = the_tree();
    let read = read_from_source_root(&root);

    let files: BTreeSet<String> = read
        .source
        .files
        .iter()
        .map(|f| relative(&root, Path::new(&f.file_path)))
        .collect();
    assert_eq!(
        files,
        set(&[
            ".hidden/h.rs",
            "build/generated.rs",
            "linked/inner/mod.rs",
            "linked/lib.rs",
            "main.rs",
            "src/inner/mod.rs",
            "src/lib.rs",
            "vendor/dep.rs",
        ]),
        "it prunes only `target` and `node_modules`: build output, vendored code and hidden \
         directories are read, the symlinked directory is read a second time under its own \
         name, and the file nobody can read is not there"
    );
    assert_eq!(
        read.basis.files, 8,
        "the identity covers exactly those files"
    );
}

#[test]
fn the_export_lists_what_the_inventory_prunes_and_follows_the_symlink() {
    let (_tmp, root, _unlock) = the_tree();
    let producer = ProducerInfo {
        id: "cognicode".into(),
        version: "task-243".into(),
        schema_version: "assurance-evidence/v1".into(),
    };
    let export = extract_from_workspace(
        &WorkspaceInfo::detect(root.clone()),
        &ExtractionCapabilities::from_flags(&[CAP_ENTITIES.to_string()]),
        &producer,
    )
    .expect("the export is produced");

    let entities: BTreeSet<String> = export
        .entities
        .iter()
        .map(|e| relative(&root, Path::new(&e.name)))
        .collect();
    assert_eq!(
        entities,
        set(&[
            ".hidden/h.rs",
            "linked/inner/mod.rs",
            "linked/lib.rs",
            "linked/locked.rs",
            "main.rs",
            "src/inner/mod.rs",
            "src/lib.rs",
            "src/locked.rs",
            "vendor/dep.rs",
        ]),
        "it prunes `.git`, `target`, `node_modules`, `.venv`, `dist` and `build`: vendored code \
         and hidden directories are listed, the symlinked directory is listed twice, and the \
         file nobody can read is listed (the export never opens it)"
    );
}

#[tokio::test]
async fn the_listing_answers_a_different_question_and_follows_no_symlink() {
    let (_tmp, root, _unlock) = the_tree();
    let ctx = HandlerContext::builder()
        .with_working_dir(root.clone())
        .build();
    let out = handle_list_files(
        &ctx,
        ListFilesInput {
            path: None,
            glob: None,
            offset: None,
            limit: Some(1000),
            recursive: None,
            max_depth: None,
        },
    )
    .await
    .expect("the listing runs");

    let listed: BTreeSet<String> = out
        .files
        .iter()
        .map(|f| relative(&root, Path::new(&f.path)))
        .collect();
    assert_eq!(
        rust_only(listed.iter().cloned()),
        set(&[
            "build/generated.rs",
            "main.rs",
            "src/inner/mod.rs",
            "src/lib.rs",
            "src/locked.rs",
            "target/debug/out.rs",
            "vendor/dep.rs",
        ]),
        "the listing prunes nothing by name (it honours .gitignore and skips hidden entries) \
         and does not follow the symlink"
    );
    assert!(
        listed.contains("src/notes.md"),
        "a file that is not a source is listed: that is what the tool is for ({listed:?})"
    );
}

/// The read says what it could not read: a file nobody can open.
#[test]
fn control_query_publishes_the_file_it_could_not_read() {
    let (_tmp, root, _unlock) = the_tree();
    let read = read_from_source_root(&root);

    let unreadable: Vec<&str> = read.unreadable.iter().map(|u| u.path.as_str()).collect();
    assert_eq!(
        unreadable,
        vec!["linked/locked.rs", "src/locked.rs"],
        "the file that cannot be read is named, once per path it was reached by"
    );
    assert!(
        read.unreadable.iter().all(|u| !u.reason.is_empty()),
        "every hole carries the reason the operating system gave"
    );
}

/// ... and a directory it could not list.
#[test]
fn control_query_publishes_the_directory_it_could_not_list() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    write(&root.join("src/lib.rs"), "pub fn kept() {}\n");
    write(
        &root.join("sealed/hidden_inside.rs"),
        "pub fn inside() {}\n",
    );
    let sealed = root.join("sealed");
    let _unlock = Unlock(sealed.clone(), 0o755);
    std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o000)).expect("seal");
    std::fs::read_dir(&sealed).expect_err("the premise: the directory cannot be listed");

    let read = read_from_source_root(&root);

    assert_eq!(
        read.basis.files, 1,
        "only the readable source is in the identity"
    );
    let unreadable: Vec<&str> = read.unreadable.iter().map(|u| u.path.as_str()).collect();
    assert_eq!(unreadable, vec!["sealed"], "the directory is named");
}

/// A tree that was read whole publishes no hole: the field is not noise.
#[test]
fn control_query_publishes_no_hole_over_a_tree_it_read_whole() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    write(&root.join("src/lib.rs"), "pub fn kept() {}\n");

    let read = read_from_source_root(&root);

    assert_eq!(read.basis.files, 1);
    assert!(read.unreadable.is_empty(), "{:?}", read.unreadable);
}
