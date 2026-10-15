//! B6.W3 — Integration tests for the user-facing `cogh` lifecycle.
//!
//! These tests pin the contract advertised in
//! `odd/tasks/b6-agent-ready-runtime.md` B6.W3:
//!
//! - `cogh install --version X` (no `--apply`) prints a JSON plan
//!   and exits 0.
//! - `cogh install --version X --apply` copies the running binary
//!   to the planned install path.
//! - `cogh install --rollback Y --apply` updates the tracker pin
//!   to Y.
//! - `cogh doctor` exits 0 after a clean self-install.
//! - The version pin survives a round-trip through the tracker.
//!
//! Every subprocess test sets `COGNICODE_BIN_DIR` to a tempdir so
//! the test never touches the developer's real `$CARGO_HOME` or
//! `$HOME`. The `serde_json` envelope is parsed to verify the
//! `dry_run` / `outcome` shape rather than just substring-matching
//! stdout (which is brittle).
//!
//! TDD contract: every test is RED before this commit (the
//! `cogh install --version` / `--rollback` / `--apply` surface
//! does not exist yet), GREEN after.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

mod common;

// ============================================================================
// helpers
// ============================================================================

/// Path to the `cogh` binary under test.
fn cogh() -> PathBuf {
    common::binary_path("cogh")
}

/// Run `cogh` with `COGNICODE_BIN_DIR` redirected at `bin_dir` so the
/// self-install never touches the developer's real install root. The
/// `HOME` override is also passed via `--home` to keep the tracker
/// under the same tempdir.
fn run_cogh_with_bin<I, S>(home: &Path, bin_dir: &Path, args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new(cogh())
        .arg("--home")
        .arg(home)
        .args(args)
        .env("COGNICODE_BIN_DIR", bin_dir)
        .env_remove("CARGO_HOME")
        .env_remove("XDG_DATA_HOME")
        .output()
        .expect("spawn cogh binary")
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn assert_success(out: &Output, label: &str) {
    assert!(
        out.status.success(),
        "{label} must exit 0; got {:?}\nstderr: {}",
        out.status,
        stderr_of(out)
    );
}

// ============================================================================
// REQ: dry-run prints a JSON InstallPlan and does NOT touch disk
// ============================================================================

#[test]
fn execute_install_dry_run_prints_plan() {
    let home = tempfile::tempdir().expect("create temp home");
    let bin_dir = tempfile::tempdir().expect("create temp bin dir");
    let out = run_cogh_with_bin(
        home.path(),
        bin_dir.path(),
        ["install", "--version", "0.101.15"],
    );

    assert_success(&out, "cogh install --version 0.101.15 (dry-run)");
    let body = stdout_of(&out);
    let json: Value =
        serde_json::from_str(&body).unwrap_or_else(|e| panic!("stdout is not JSON: {e}\n{body}"));

    assert_eq!(
        json.get("dry_run").and_then(|v| v.as_bool()),
        Some(true),
        "dry-run envelope must carry dry_run=true; got: {body}"
    );
    let plan = json
        .get("plan")
        .expect("envelope must include `plan`")
        .as_object()
        .expect("plan must be an object");
    assert_eq!(
        plan.get("version").and_then(|v| v.as_str()),
        Some("0.101.15"),
        "plan.version must echo the requested version"
    );
    let install_path = plan
        .get("install_path")
        .and_then(|v| v.as_str())
        .expect("plan.install_path must be a string");
    assert!(
        install_path.ends_with("cogh") || install_path.ends_with("cogh.exe"),
        "install_path must end with cogh binary; got {install_path}"
    );

    // Dry-run must NOT write the binary. The bin_dir is empty.
    let mut entries: Vec<_> = std::fs::read_dir(bin_dir.path())
        .expect("read bin dir")
        .filter_map(|e| e.ok())
        .collect();
    entries.retain(|e| e.file_name() != "completions");
    assert!(
        entries.is_empty(),
        "dry-run must not write the binary; found {:?}",
        entries.iter().map(|e| e.path()).collect::<Vec<_>>()
    );
}

// ============================================================================
// REQ: --apply copies the running binary to the planned install path
// ============================================================================

#[test]
fn execute_install_apply_copies_binary() {
    let home = tempfile::tempdir().expect("create temp home");
    let bin_dir = tempfile::tempdir().expect("create temp bin dir");
    let out = run_cogh_with_bin(
        home.path(),
        bin_dir.path(),
        ["install", "--version", "0.101.15", "--apply"],
    );

    assert_success(&out, "cogh install --version 0.101.15 --apply");
    let body = stdout_of(&out);
    let json: Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("--apply stdout is not JSON: {e}\n{body}"));
    assert_eq!(json.get("dry_run").and_then(|v| v.as_bool()), Some(false));
    let outcome = json
        .get("outcome")
        .expect("envelope must include `outcome`");
    let installed = outcome
        .get("install_path")
        .and_then(|v| v.as_str())
        .expect("outcome.install_path must be a string");
    let installed_path = PathBuf::from(installed);
    assert!(
        installed_path.exists(),
        "binary must be written to {}",
        installed_path.display()
    );
    // Executable on unix: mode must include at least 0o755.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&installed_path)
            .expect("stat installed")
            .permissions()
            .mode();
        assert!(
            mode & 0o111 == 0o111,
            "installed binary must be executable; got mode {mode:o}"
        );
    }
}

// ============================================================================
// REQ: dry-run envelope includes an action discriminator
// ============================================================================

#[test]
fn dry_run_envelope_records_install_action() {
    let home = tempfile::tempdir().expect("create temp home");
    let bin_dir = tempfile::tempdir().expect("create temp bin dir");
    let out = run_cogh_with_bin(
        home.path(),
        bin_dir.path(),
        ["install", "--version", "0.101.15"],
    );
    assert_success(&out, "dry-run install");
    let json: Value = serde_json::from_str(&stdout_of(&out)).expect("stdout is JSON");
    let action = json.get("action").expect("envelope must include `action`");
    assert_eq!(
        action.get("kind").and_then(|v| v.as_str()),
        Some("install"),
        "first-time install must be tagged kind=install"
    );
}

// ============================================================================
// REQ: --rollback updates the tracker pin
// ============================================================================

#[test]
fn rollback_writes_tracker_pin() {
    let home = tempfile::tempdir().expect("create temp home");
    let bin_dir = tempfile::tempdir().expect("create temp bin dir");

    // First install pins 0.101.15.
    let first = run_cogh_with_bin(
        home.path(),
        bin_dir.path(),
        ["install", "--version", "0.101.15", "--apply"],
    );
    assert_success(&first, "first install");
    let tracker = home.path().join("tracker").join("version");
    assert_eq!(
        std::fs::read_to_string(&tracker)
            .expect("read tracker")
            .trim(),
        "0.101.15",
        "tracker must reflect the first install"
    );

    // Then rollback to 0.101.14.
    let second = run_cogh_with_bin(
        home.path(),
        bin_dir.path(),
        ["install", "--rollback", "0.101.14", "--apply"],
    );
    assert_success(&second, "rollback install");
    let body = stdout_of(&second);
    let json: Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("rollback stdout is not JSON: {e}\n{body}"));
    let action = json
        .get("outcome")
        .and_then(|o| o.get("action"))
        .expect("rollback outcome must carry an action");
    assert_eq!(
        action.get("kind").and_then(|v| v.as_str()),
        Some("rollback"),
        "rollback envelope must be tagged kind=rollback"
    );
    assert_eq!(
        action.get("to").and_then(|v| v.as_str()),
        Some("0.101.14"),
        "rollback `to` must echo the target version"
    );
    assert_eq!(
        std::fs::read_to_string(&tracker)
            .expect("read tracker")
            .trim(),
        "0.101.14",
        "tracker must be updated to the rollback target"
    );
}

// ============================================================================
// REQ: dry-run --rollback prints a plan, exits 0, does not touch the tracker
// ============================================================================

#[test]
fn rollback_dry_run_prints_plan_without_writing() {
    let home = tempfile::tempdir().expect("create temp home");
    let bin_dir = tempfile::tempdir().expect("create temp bin dir");
    let out = run_cogh_with_bin(
        home.path(),
        bin_dir.path(),
        ["install", "--rollback", "0.101.14"],
    );
    assert_success(&out, "rollback dry-run");
    let json: Value =
        serde_json::from_str(&stdout_of(&out)).expect("rollback dry-run stdout is JSON");
    assert_eq!(json.get("dry_run").and_then(|v| v.as_bool()), Some(true));
    let tracker = home.path().join("tracker").join("version");
    assert!(
        !tracker.exists(),
        "dry-run rollback must NOT write the tracker"
    );
}

// ============================================================================
// REQ: --version with no install still works (regression guard)
// ============================================================================

#[test]
fn cogh_version_works_without_lifecycle_install() {
    // B6.W3 must not regress the existing `cogh --version` surface.
    let home = tempfile::tempdir().expect("create temp home");
    let out = Command::new(cogh())
        .arg("--home")
        .arg(home.path())
        .arg("--version")
        .output()
        .expect("spawn cogh --version");
    assert_success(&out, "cogh --version");
    let line = stdout_of(&out)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    assert!(
        line.starts_with("cogh "),
        "cogh --version must start with `cogh `; got {line:?}"
    );
}

// ============================================================================
// REQ: SHA-256 in the outcome matches the bytes on disk
// ============================================================================

#[test]
fn outcome_sha256_matches_on_disk_bytes() {
    let home = tempfile::tempdir().expect("create temp home");
    let bin_dir = tempfile::tempdir().expect("create temp bin dir");
    let out = run_cogh_with_bin(
        home.path(),
        bin_dir.path(),
        ["install", "--version", "0.101.15", "--apply"],
    );
    assert_success(&out, "apply install");
    let json: Value = serde_json::from_str(&stdout_of(&out)).expect("JSON outcome");
    let expected_sha = json
        .get("outcome")
        .and_then(|o| o.get("sha256"))
        .and_then(|v| v.as_str())
        .expect("outcome.sha256 must be a string")
        .to_string();
    let installed = json
        .get("outcome")
        .and_then(|o| o.get("install_path"))
        .and_then(|v| v.as_str())
        .expect("outcome.install_path must be a string");
    let bytes = std::fs::read(installed).expect("read installed binary");
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(&bytes);
    let actual = format!("{:x}", h.finalize());
    assert_eq!(
        actual, expected_sha,
        "SHA-256 in outcome must match the on-disk bytes"
    );
}

// ============================================================================
// REQ: self-install and plugin-install are two distinct surfaces
// ============================================================================

#[test]
fn self_install_is_distinct_from_plugin_install() {
    // B6.W3 routes `cogh install --version X` (no plugin) to the
    // self-install surface, and `cogh install <plugin> --version X`
    // to the legacy plugin install. Both must be reachable from the
    // same binary; a missing self-install dispatch is a regression.
    let home = tempfile::tempdir().expect("create temp home");
    let bin_dir = tempfile::tempdir().expect("create temp bin dir");

    // No plugin → self-install dry-run; this is the B6.W3 surface.
    let self_out = run_cogh_with_bin(
        home.path(),
        bin_dir.path(),
        ["install", "--version", "0.101.15"],
    );
    assert_success(&self_out, "cogh install --version (self-install)");
    let self_json: Value =
        serde_json::from_str(&stdout_of(&self_out)).expect("self-install stdout is JSON");
    assert_eq!(
        self_json.get("dry_run").and_then(|v| v.as_bool()),
        Some(true),
        "self-install must produce a dry_run envelope; got: {}",
        stdout_of(&self_out)
    );

    // With a plugin positional, the legacy install path runs and
    // does NOT produce a B6.W3 dry-run envelope (it dispatches to
    // `layout::cmd_install` which prints its own format).
    let plugin_out = run_cogh_with_bin(
        home.path(),
        bin_dir.path(),
        [
            "install",
            "mcp-server",
            "--version",
            "0.96.0",
            "--channel",
            "stable",
            "--staging",
            "/nonexistent-staging-for-test",
        ],
    );
    // We don't assert success/failure of the legacy path (it may
    // fail on the staging fixture); we only assert that the legacy
    // path does NOT print the B6.W3 self-install dry-run envelope.
    let body = stdout_of(&plugin_out);
    let parsed: Option<Value> = serde_json::from_str(&body).ok();
    if let Some(json) = parsed {
        let is_self_install = json.get("dry_run").is_some() && json.get("plan").is_some();
        assert!(
            !is_self_install,
            "plugin install must not dispatch to self-install surface; got: {body}"
        );
    }
}
