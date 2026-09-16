//! Integration tests for the `cogh` CLI binary.
//!
//! Each test spawns the `cogh` binary (located via Cargo's
//! `CARGO_BIN_EXE_cogh` env var) against an isolated `--home`
//! fixture so tests are independent and parallel-safe.
//!
//! Locks down the four `cogh` capability contracts in
//! `openspec/specs/cognicode-cli/spec.md` that are exercisable
//! without network or production filesystem state:
//! - `cogh --version` reports the binary version (REQ: `cogh` binary)
//! - `cogh list` renders the plugin table (REQ: `cogh list`)
//! - `cogh current` reads the tracker (REQ: `cogh current`)
//! - `cogh doctor` validates installation (REQ: `cogh doctor`)
//!
//! TDD contract: every test is RED before this commit, GREEN after.

use std::path::Path;
use std::process::{Command, Output};

/// Path to the `cogh` binary injected by Cargo at build time.
///
/// `env!` resolves at compile time; if `CARGO_BIN_EXE_cogh` is not
/// defined (e.g. when this test is built outside Cargo's test harness),
/// the build fails loudly rather than silently picking a wrong path.
fn cogh() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cogh"))
}

/// Run `cogh --home <home> <args>` and capture the result.
fn run_cogh<I, S>(home: &Path, args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new(cogh())
        .arg("--home")
        .arg(home)
        .args(args)
        .output()
        .expect("spawn cogh binary")
}

/// Run `cogh init --home <home>` so the home layout is populated.
fn init_home(home: &Path) -> Output {
    run_cogh(home, ["init"])
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

// ============================================================================
// REQ: `cogh` binary is a single static executable
// ============================================================================

#[test]
fn cogh_version_reports_semver_token() {
    let home = tempfile::tempdir().expect("create temp home");
    let out = run_cogh(home.path(), ["--version"]);

    assert!(
        out.status.success(),
        "cogh --version must exit 0; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    let body = stdout(&out);
    let line = body.lines().next().unwrap_or("").trim().to_string();
    assert!(
        line.starts_with("cogh "),
        "expected first stdout line to start with `cogh `, got: {line:?}\nfull stdout: {body}"
    );

    // The semver token follows `cogh `. We don't pin the exact value
    // (it's CARGO_PKG_VERSION), but we do assert the shape: digits,
    // dots, optional pre-release suffix.
    let tail = line.strip_prefix("cogh ").unwrap_or("");
    let head = tail
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .next()
        .unwrap_or("");
    assert!(
        head.chars().next().map_or(false, |c| c.is_ascii_digit()) && head.contains('.'),
        "expected cogh <semver> on stdout, got: {line:?}"
    );
}

// ============================================================================
// REQ: `cogh list` shows installed plugins and versions
// ============================================================================

#[test]
fn cogh_list_on_uninitialised_home_reports_not_initialized() {
    let home = tempfile::tempdir().expect("create temp home");
    let out = run_cogh(home.path(), ["list"]);

    assert!(
        out.status.success(),
        "cogh list on uninitialised home must exit 0; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout(&out).contains("(home not initialized)"),
        "expected stdout to mention `(home not initialized)`; got: {}",
        stdout(&out)
    );
}

#[test]
fn cogh_list_on_initialised_home_renders_table() {
    let home = tempfile::tempdir().expect("create temp home");
    let init = init_home(home.path());
    assert!(
        init.status.success(),
        "cogh init must succeed to seed the layout; got {:?}\nstderr: {}",
        init.status,
        String::from_utf8_lossy(&init.stderr)
    );

    let out = run_cogh(home.path(), ["list"]);
    assert!(
        out.status.success(),
        "cogh list on initialised home must exit 0; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let body = stdout(&out);
    assert!(
        body.contains("Plugin"),
        "expected stdout to include `Plugin` table header; got: {body}"
    );
    assert!(
        body.contains("Installed"),
        "expected stdout to include `Installed` table header; got: {body}"
    );
}

// ============================================================================
// REQ: `cogh current` shows the active version pin
// ============================================================================

#[test]
fn cogh_current_with_no_pinned_version_reports_unset() {
    let home = tempfile::tempdir().expect("create temp home");
    let out = run_cogh(home.path(), ["current"]);

    assert!(
        out.status.success(),
        "cogh current on uninitialised home must exit 0; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let trimmed = stdout(&out).trim().to_string();
    assert_eq!(
        trimmed, "(no version pinned)",
        "expected `(no version pinned)` on stdout; got: {trimmed:?}"
    );
}

#[test]
fn cogh_current_reads_pinned_version_from_tracker() {
    let home = tempfile::tempdir().expect("create temp home");
    let init = init_home(home.path());
    assert!(
        init.status.success(),
        "cogh init must succeed to seed the layout; got {:?}\nstderr: {}",
        init.status,
        String::from_utf8_lossy(&init.stderr)
    );

    // Write the tracker directly so we don't depend on install/network.
    let tracker = home.path().join("tracker").join("version");
    std::fs::create_dir_all(tracker.parent().unwrap()).expect("mkdir tracker");
    std::fs::write(&tracker, "0.92.0\n").expect("write tracker");

    let out = run_cogh(home.path(), ["current"]);
    assert!(
        out.status.success(),
        "cogh current must exit 0; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let trimmed = stdout(&out).trim().to_string();
    assert_eq!(
        trimmed, "0.92.0",
        "expected tracker content on stdout; got: {trimmed:?}"
    );
}

// ============================================================================
// REQ: `cogh doctor` validates installation
// ============================================================================

#[test]
fn cogh_doctor_on_uninitialised_home_reports_not_initialized() {
    let home = tempfile::tempdir().expect("create temp home");
    let out = run_cogh(home.path(), ["doctor"]);

    assert!(
        out.status.success(),
        "cogh doctor on uninitialised home must exit 0; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let body = stdout(&out);
    // e74 WU4: doctor is now a four-dimension report. On an
    // uninitialised home (where `cogh init` was never run), the
    // Core health dimension must report Fail. The detail string
    // is either `does not exist` (home itself missing) or
    // `missing:` (home dir created but bin/ shims/ absent after
    // a partial init). Either is honest evidence of the failure.
    assert!(
        body.contains("cogh doctor"),
        "expected stdout to include `cogh doctor` header; got: {body}"
    );
    assert!(
        body.contains("Core health"),
        "expected stdout to include `Core health` dimension; got: {body}"
    );
    assert!(
        body.contains("does not exist") || body.contains("missing:"),
        "expected stdout to flag missing layout (`does not exist` or `missing:`); got: {body}"
    );
}

#[test]
fn cogh_doctor_on_initialised_home_reports_healthy() {
    let home = tempfile::tempdir().expect("create temp home");
    let init = init_home(home.path());
    assert!(
        init.status.success(),
        "cogh init must succeed to seed the layout; got {:?}\nstderr: {}",
        init.status,
        String::from_utf8_lossy(&init.stderr)
    );

    let out = run_cogh(home.path(), ["doctor"]);
    assert!(
        out.status.success(),
        "cogh doctor on initialised home must exit 0; got {:?}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let body = stdout(&out);
    // e74 WU4-followup: doctor is now a four-dimension report.
    // `cogh init` populates bin/, shims/, tracker/, but not
    // `tracker/version`. The Core health dimension reports Pass
    // (home+bin+shims present) and adds a Warn-level note about
    // the missing version file. Either `home, bin/, shims/
    // present` (Pass detail) or the unified warn-detail that
    // `tracker/version missing` produces is honest evidence that
    // Core health is healthy (not Fail).
    assert!(
        body.contains("Core health"),
        "expected `Core health` dimension; got: {body}"
    );
    assert!(
        body.contains("home, bin/, shims/ present")
            || body.contains("tracker/version missing"),
        "expected Core health detail (`home, bin/, shims/ present` for Pass or `tracker/version missing` for Warn); got: {body}"
    );
}
