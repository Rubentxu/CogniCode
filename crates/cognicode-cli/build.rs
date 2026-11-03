//! B1.W2 — Build script: bake source identity into the binary.
//!
//! Emits `cargo:rustc-env=GIT_SHA=<short SHA>` so the runtime can
//! answer `cognicode identity` (and `--version --verbose`) with the
//! exact commit the binary was built from. The contract `B1.W2`
//! requires this: a binary claiming v0.101.11 but built from a
//! different commit is a forged identity, not a release artifact.
//!
//! Behaviour:
//!   - `git rev-parse --short HEAD` at build time. If the workspace
//!     is not a git checkout (e.g. release tarball), the env var
//!     is empty and the runtime reports `"unknown"` instead of
//!     panicking — that case is itself a signal (a release tarball
//!     SHOULD carry the SHA baked at archive time, not a placeholder).
//!   - `cargo:rerun-if-changed` on git's own HEAD and branch refs so a fresh commit triggers
//!     a rebuild. We do NOT rerun-if-changed on the working tree
//!     (it would rebuild on every edit), only on HEAD movement.
//!
//! Does NOT affect the byte stream of `--version` (T1-5.1 snapshot
//! pins `"cogh {CARGO_PKG_VERSION}"`); that flag stays byte-identical.
//! Identity is exposed via `cognicode identity` (B1.W2 new variant).

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
