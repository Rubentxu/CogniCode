//! Build script: bake the source identity into the analyzer.
//!
//! Emits `cargo:rustc-env=GIT_SHA=<short SHA>` so the analyzer can say
//! which commit produced it. A snapshot that does not say who wrote it can
//! be served by a build that did not write it, and then the consumer gets a
//! graph calculated by another analyzer with no sign of it.
//!
//! The mechanism is the same one the CLI uses, and it answers the same
//! question: `git rev-parse --short HEAD` at build time. When the workspace
//! is not a git checkout (a release tarball, for instance) the variable is
//! empty and the runtime says so instead of inventing a commit.
//!
//! `cargo:rerun-if-changed` on git's own HEAD and branch refs, so a fresh
//! commit triggers a rebuild.
//! The working tree is NOT a rerun trigger: it would rebuild on every edit.

use std::process::Command;

/// Asks git where a path lives. In a linked worktree `.git` is a file and
/// HEAD sits under the common directory, so a hardcoded `../../.git/HEAD`
/// points at nothing, and Cargo reruns the script (and rebuilds the crate)
/// on every invocation when a watched path is missing.
fn git_path(args: &[&str]) -> Option<std::path::PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--path-format=absolute"])
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let path = std::path::PathBuf::from(String::from_utf8(out.stdout).ok()?.trim());
    path.exists().then_some(path)
}

fn main() {
    let short_sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    println!("cargo:rustc-env=GIT_SHA={}", short_sha);
    let common = git_path(&["--git-common-dir"]);
    let watched = [
        git_path(&["--git-path", "HEAD"]),
        common
            .as_ref()
            .and_then(|c| Some(c.join("refs/heads")).filter(|p| p.exists())),
        common
            .as_ref()
            .and_then(|c| Some(c.join("packed-refs")).filter(|p| p.exists())),
    ];
    for path in watched.into_iter().flatten() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
