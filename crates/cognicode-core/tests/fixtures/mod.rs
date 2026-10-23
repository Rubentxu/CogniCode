//! The repository's real fixtures, in a scratch directory.
//!
//! A contract that answers a question about a project should ask it about
//! the project the repository really holds, not about a toy written inside
//! the test. The versioned fixtures are that project, and copying one into
//! a temporary directory is how a test gets to read it without writing
//! into the tree.
//!
//! Doing it here once keeps four contracts from growing their own copy of
//! the same twenty lines — and keeps them from drifting apart, which is
//! how two of them end up testing different fixtures under the same name.
//!
//! Not every contract needs every helper here, so the unused ones are not
//! a finding.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};

/// The directory the versioned fixtures live in.
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures")
        .canonicalize()
        .expect("the versioned fixtures are where the repository says they are")
}

/// The fixture named `name`, copied into `dir`.
///
/// The copy holds the versioned content: a contract that runs against it
/// runs against what the repository really holds, and the repository is
/// left as it was.
pub fn copy_fixture(dir: &Path, name: &str) -> PathBuf {
    let target = dir.join(name);
    copy_tree(&fixtures_root().join(name), &target);
    target
}

/// Copies a directory tree, creating the directories it needs.
pub fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).expect("mkdir");
    for entry in std::fs::read_dir(source).expect("read fixture") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &destination);
        } else {
            std::fs::copy(&path, &destination).expect("copy fixture");
        }
    }
}

/// The graph of a scratch directory that holds the given fixture.
///
/// The graph is built the way the product builds it, over the fixture the
/// repository holds, so a contract that reads this graph reads what a
/// consumer would read.
pub async fn graph_of(dir: &Path, fixture: &str) -> HandlerContext {
    copy_fixture(dir, fixture);
    graph_of_dir(dir).await
}

/// The graph of a scratch directory whose files are already in it.
pub async fn graph_of_dir(dir: &Path) -> HandlerContext {
    let ctx = HandlerContext::builder().with_working_dir(dir).build();
    handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");
    ctx
}
