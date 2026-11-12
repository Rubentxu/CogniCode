//! Build script: bake the analyzer's identity into the crate.
//!
//! A snapshot that does not say who wrote it can be served by a build that
//! did not write it, and then the consumer gets a graph calculated by another
//! analyzer with no sign of it. The identity has two parts:
//!
//! - `COGNICODE_ANALYZER_SOURCE_DIGEST`, always: the sha256 of the analyzer's
//!   inputs (see `analyzer_source.rs`). Two builds of different code are two
//!   analyzers even when they sit on the same commit.
//! - `COGNICODE_ANALYZER_COMMIT`, only in the `release` profile: the commit a
//!   published binary was built from, so the artifact carries its provenance.
//!   A development build does not ask git, and does not watch git's refs: a
//!   commit changes no byte of the analyzer, and watching the refs recompiled
//!   the core and everything above it on every commit, in every worktree that
//!   shares the target directory.
//!
//! The script watches the inputs it hashes and nothing else.

#[path = "analyzer_source.rs"]
mod analyzer_source;

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let package_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
    );

    let tree = analyzer_source::collect(&package_dir)
        .unwrap_or_else(|error| panic!("the analyzer's source cannot be read: {error}"));
    println!(
        "cargo:rustc-env=COGNICODE_ANALYZER_SOURCE_DIGEST={}",
        analyzer_source::digest(tree)
    );
    for input in analyzer_source::INPUTS {
        let path = package_dir.join(input);
        // Cargo reruns the script on every build when a watched path does not
        // exist, so an input that is not there (a packaged crate has no
        // siblings) is hashed as absent and not watched.
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    let release = std::env::var("PROFILE").as_deref() == Ok("release");
    let commit = if release { release_commit() } else { None };
    // Emitted even when empty, so a variable of the same name in the
    // environment cannot become the analyzer's commit.
    println!(
        "cargo:rustc-env=COGNICODE_ANALYZER_COMMIT={}",
        commit.as_deref().unwrap_or("")
    );
}

/// The commit a release build comes from, and the git refs that move it.
///
/// `None` outside a git checkout (a release tarball, for instance): the
/// runtime says so instead of inventing a commit.
fn release_commit() -> Option<String> {
    let common = git_path(&["--git-common-dir"]);
    let watched = [
        git_path(&["--git-path", "HEAD"]),
        common.as_ref().map(|c| c.join("refs/heads")),
        common.as_ref().map(|c| c.join("packed-refs")),
    ];
    for path in watched.into_iter().flatten().filter(|p| p.exists()) {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    git(&["rev-parse", "--short", "HEAD"]).filter(|sha| !sha.is_empty())
}

/// Asks git where a path lives. In a linked worktree `.git` is a file and
/// HEAD sits under the common directory, so a hardcoded `../../.git/HEAD`
/// points at nothing.
fn git_path(args: &[&str]) -> Option<PathBuf> {
    let mut full = vec!["rev-parse", "--path-format=absolute"];
    full.extend_from_slice(args);
    git(&full)
        .map(PathBuf::from)
        .filter(|path| Path::new(path).exists())
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    Some(String::from_utf8(out.stdout).ok()?.trim().to_string())
}
