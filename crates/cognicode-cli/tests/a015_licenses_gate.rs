//! A-015 / PR-SEC — `cargo deny check licenses` gate.
//!
//! Locks down the supply-chain licences gate as a hermetic
//! black-box test. The `release-validate.yml` workflow runs the
//! same command in CI; this test is the local equivalent that
//! runs on every `cargo test -p cognicode-cli`.
//!
//! Why hermetic: the gate is the source of truth for "every
//! dependency licence is allowed by `deny.toml`". Without an
//! automated check, a contributor adding a new dependency with
//! an unlisted licence would slip past `merge-gate` until the
//! next release audit.
//!
//! Failure modes the test pins:
//!   1. `cargo-deny` not installed on the host → SKIP (the test is
//!      best-effort, the CI gate is authoritative).
//!   2. The gate itself reports a failure (exit code != 0 or no
//!      "licenses ok" in stdout) → FAIL with the captured output.
//!   3. The `deny.toml` is missing the `[licenses]` section → FAIL
//!      (the gate would have nothing to check).
//!
//! Run: `cargo test -p cognicode-cli --test a015_licenses_gate`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn deny_toml_path() -> PathBuf {
    workspace_root().join("deny.toml")
}

fn has_licenses_section(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    // Anchor on a section header line. Anchoring on the bracket-only
    // form avoids false positives if `[licenses]` appears as a
    // comment (e.g. `# see [licenses] in the docs`).
    text.lines()
        .any(|l| l.trim_start().starts_with("[licenses]"))
}

fn locate_cargo_deny() -> Option<PathBuf> {
    // Prefer `cargo deny` on PATH; fall back to a well-known cargo
    // bin location. Skip the test if neither exists — the CI gate
    // is authoritative and the test is best-effort locally.
    let on_path = Command::new("cargo-deny")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| PathBuf::from("cargo-deny"));
    if let Some(p) = on_path {
        return Some(p);
    }
    let cargo_home = std::env::var("CARGO_HOME")
        .ok()
        .map(|h| PathBuf::from(h).join("bin").join("cargo-deny"));
    let home_bin = std::env::var("HOME").ok().map(|h| {
        PathBuf::from(h)
            .join(".cargo")
            .join("bin")
            .join("cargo-deny")
    });
    [cargo_home, home_bin]
        .into_iter()
        .flatten()
        .find(|p| p.is_file())
}

#[test]
fn a015_deny_toml_declares_a_licenses_section() {
    // The gate is only meaningful if the policy exists. A missing
    // `[licenses]` section means `cargo deny check licenses` exits
    // 0 silently and reports nothing — a silent green that hides
    // every future violation.
    let path = deny_toml_path();
    assert!(
        path.exists(),
        "deny.toml must exist at the workspace root: {}",
        path.display()
    );
    assert!(
        has_licenses_section(&path),
        "deny.toml must contain a `[licenses]` section; without it \
         `cargo deny check licenses` would pass silently"
    );
}

#[test]
fn a015_cargo_deny_check_licenses_passes_on_the_real_workspace() {
    // The canonical gate. Runs `cargo deny check licenses` from the
    // workspace root and asserts (a) exit 0 and (b) "licenses ok"
    // in stdout. Either condition failing means a dependency
    // licence is not on the allow-list in `deny.toml` — the release
    // gate would (and now does) fail.
    let Some(cargo_deny) = locate_cargo_deny() else {
        eprintln!(
            "skip: cargo-deny not on PATH; install via `cargo install \
             cargo-deny --locked` to enable this test"
        );
        return;
    };

    let workspace = workspace_root();
    let output = Command::new(&cargo_deny)
        .arg("check")
        .arg("licenses")
        .current_dir(&workspace)
        .output()
        .expect("spawn cargo deny check licenses");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "cargo deny check licenses failed under {} (exit {:?})\n--- stdout ---\n{}\n--- stderr ---\n{}",
        workspace.display(),
        output.status.code(),
        stdout,
        stderr
    );
    assert!(
        stdout.contains("licenses ok"),
        "cargo deny check licenses must report `licenses ok`; got: {stdout}"
    );
}

#[test]
fn a015_cargo_deny_version_reports_a_recognised_build() {
    // Sanity check: the binary we found responds to `--version`.
    // If this fails, the install on the host is broken and the
    // other two tests in this file would silently skip — the test
    // suite must surface the broken state instead of going green.
    let Some(cargo_deny) = locate_cargo_deny() else {
        eprintln!("skip: cargo-deny not on PATH");
        return;
    };
    let output = Command::new(&cargo_deny)
        .arg("--version")
        .output()
        .expect("spawn cargo-deny --version");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "cargo-deny --version failed; stdout: {stdout}"
    );
    assert!(
        stdout.contains("cargo-deny"),
        "cargo-deny --version output should mention `cargo-deny`; got: {stdout}"
    );
}
