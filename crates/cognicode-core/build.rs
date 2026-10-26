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
//! `cargo:rerun-if-changed=.git/HEAD` so a fresh commit triggers a rebuild.
//! The working tree is NOT a rerun trigger: it would rebuild on every edit.

use std::process::Command;

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
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads/");
}
