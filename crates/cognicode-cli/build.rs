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
//!   - `cargo:rerun-if-changed=.git/HEAD` so a fresh commit triggers
//!     a rebuild. We do NOT rerun-if-changed on the working tree
//!     (it would rebuild on every edit), only on HEAD movement.
//!
//! Does NOT affect the byte stream of `--version` (T1-5.1 snapshot
//! pins `"cogh {CARGO_PKG_VERSION}"`); that flag stays byte-identical.
//! Identity is exposed via `cognicode identity` (B1.W2 new variant).

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