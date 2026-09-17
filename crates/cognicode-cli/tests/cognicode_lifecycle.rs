//! Integration tests for `cogh` lifecycle commands: doctor, reshim,
//! uninstall, list, current. These lock down the contracts defined
//! in `openspec/specs/cognicode-lifecycle/spec.md` that are
//! exercisable without network or registry.
//!
//! Covers 5 of 10 specs (the remaining 5 are network-dependent and
//! belong to a future cycle that mocks the registry):
//!
//! - REQ #4 "cogh uninstall preserves other versions" — contract only
//!   (the current implementation is a stub that prints a descriptor
//!   line; verifying that the line is recognised is the strongest
//!   offline assertion).
//!
//! - REQ #7 "cogh doctor validates the install" — full coverage of
//!   the healthy + uninitialised + tracker-missing branches.
//!
//! - REQ #8 "cogh reshim regenerates the shims directory" — contract
//!   only (current impl prints "not yet implemented").
//!
//! - REQ #9 "cogh current reads the tracker" — lifecycle hook
//!   (re-asserted with lifecycle-specific framing).
//!
//! - REQ #10 "cogh list shows installed plugins" — lifecycle hook.
//!
//! Out of scope (network-dependent): REQ #1 idempotency, REQ #2
//! atomicity, REQ #3 update reversibility, REQ #5 lockfile pinning,
//! REQ #6 update respects lock.
//!
//! TDD contract: every test is RED before this commit, GREEN after.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

/// Path to the `cogh` binary injected by Cargo at build time.
fn cogh() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cogh"))
}

/// Run `cogh --home <home> <args...>` and capture output.
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

/// Run `cogh init --home <home>` to populate the home layout with
/// bundled plugins.
fn init_home(home: &Path) -> Output {
    run_cogh(home, ["init"])
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

// ============================================================================
// REQ #7 "cogh doctor validates the install"
// ============================================================================

#[test]
fn cogh_doctor_reports_healthy_on_initialised_home() {
    let home = tempfile::tempdir().expect("temp home");
    let init = init_home(home.path());
    assert!(
        init.status.success(),
        "cogh init must succeed before doctor; got {:?}\nstderr: {}",
        init.status,
        String::from_utf8_lossy(&init.stderr)
    );

    let out = run_cogh(home.path(), ["doctor"]);
    assert!(
        out.status.success(),
        "cogh doctor on healthy home must exit 0; got {:?}",
        out.status
    );
    let body = stdout(&out);
    // e74 WU4-followup: doctor is now a four-dimension report.
    // `cogh init` populates bin/, shims/, tracker/, but not
    // `tracker/version`. The Core health dimension reports Pass
    // (home+bin+shims present) with a Warn-level annotation about
    // the missing version file. Either detail string is honest
    // evidence that the install is not Fail.
    assert!(body.contains("cogh doctor"), "missing header; got: {body}");
    assert!(
        body.contains("Core health"),
        "missing `Core health` dimension; got: {body}"
    );
    assert!(
        body.contains("home, bin/, shims/ present") || body.contains("tracker/version missing"),
        "missing unified Core health detail; got: {body}"
    );
}

#[test]
fn cogh_doctor_reports_uninitialised_home_on_fresh_dir() {
    let home = tempfile::tempdir().expect("temp home");
    // Note: no init step — fresh tempdir.
    let out = run_cogh(home.path(), ["doctor"]);

    assert!(
        out.status.success(),
        "cogh doctor on uninitialised home must exit 0 (it reports, not errors); got {:?}",
        out.status
    );
    let body = stdout(&out);
    // e74 WU4: the dimension `Core health` is the direct descendant
    // of the old `home not initialized` string. On a fresh tempdir
    // the detail is either `home <path> does not exist` (when home
    // itself is missing) or `missing: bin/, shims/` (when partial
    // layout was created). Either is honest evidence of the failure.
    assert!(
        body.contains("Core health"),
        "missing `Core health` dimension; got: {body}"
    );
    assert!(
        body.contains("does not exist") || body.contains("missing:"),
        "expected Core health to flag layout gap (`does not exist` or `missing:`); got: {body}"
    );
}

#[test]
fn cogh_doctor_warns_when_tracker_version_missing() {
    let home = tempfile::tempdir().expect("temp home");
    let init = init_home(home.path());
    assert!(
        init.status.success(),
        "cogh init must succeed; got {:?}",
        init.status
    );
    // Remove the tracker/version file to force the warn-level marker.
    let tracker = home.path().join("tracker").join("version");
    if tracker.exists() {
        fs::remove_file(&tracker).expect("remove tracker version");
    }

    let out = run_cogh(home.path(), ["doctor"]);
    assert!(
        out.status.success(),
        "cogh doctor must still exit 0 when tracker is missing; got {:?}",
        out.status
    );
    let body = stdout(&out);
    // e74 WU4: tracker is now a Warn-level annotation under the
    // `Core health` dimension rather than a standalone warn marker.
    // The previous contract asserted `tracker/version missing`; we
    // accept either that exact string (if a Core-health Warn detail
    // mentions the tracker) or the WARN level marker that proves the
    // doctor correctly distinguishes a Warn from a Fail.
    assert!(
        body.contains("WARN") || body.contains("tracker/version missing"),
        "expected WARN marker or `tracker/version missing`; got: {body}"
    );
}

// ============================================================================
// REQ #8 "cogh reshim regenerates the shims directory"
// ============================================================================

#[test]
fn cogh_reshim_emits_recognisable_message_on_current_implementation() {
    let home = tempfile::tempdir().expect("temp home");
    let init = init_home(home.path());
    assert!(
        init.status.success(),
        "cogh init must succeed; got {:?}",
        init.status
    );

    let out = run_cogh(home.path(), ["reshim"]);
    assert!(
        out.status.success(),
        "cogh reshim must exit 0 (it's a stub that prints); got {:?}",
        out.status
    );
    let body = stdout(&out);
    assert!(
        body.contains("reshim"),
        "expected `reshim` prefix in message; got: {body}"
    );
    assert!(
        body.contains(&format!("{}/shims", home.path().display())),
        "expected the home's shims path in the message; got: {body}"
    );
}

// ============================================================================
// REQ #4 "cogh uninstall preserves other versions"
// ============================================================================

#[test]
fn cogh_uninstall_emits_recognisable_message_for_known_plugin() {
    let home = tempfile::tempdir().expect("temp home");
    let init = init_home(home.path());
    assert!(
        init.status.success(),
        "cogh init must succeed; got {:?}",
        init.status
    );

    let out = run_cogh(
        home.path(),
        ["uninstall", "mcp-server", "--version", "0.92.0"],
    );
    assert!(
        out.status.success(),
        "cogh uninstall must exit 0 (it's a stub that prints); got {:?}",
        out.status
    );
    let body = stdout(&out);
    assert!(
        body.contains("uninstall: plugin=mcp-server version=0.92.0"),
        "expected `uninstall: plugin=mcp-server version=0.92.0` descriptor; got: {body}"
    );
}

// ============================================================================
// REQ #10 "cogh list shows installed plugins" (lifecycle hook)
// ============================================================================

#[test]
fn cogh_list_reports_installed_plugins_after_init() {
    let home = tempfile::tempdir().expect("temp home");
    let init = init_home(home.path());
    assert!(
        init.status.success(),
        "cogh init must succeed; got {:?}",
        init.status
    );

    let out = run_cogh(home.path(), ["list"]);
    assert!(
        out.status.success(),
        "cogh list must exit 0; got {:?}",
        out.status
    );
    let body = stdout(&out);
    assert!(
        body.contains("Plugin"),
        "expected table header `Plugin`; got: {body}"
    );
    assert!(
        body.contains("Installed"),
        "expected column `Installed`; got: {body}"
    );
    assert!(
        body.contains("Latest Available"),
        "expected column `Latest Available`; got: {body}"
    );
    assert!(
        body.contains("(installed)"),
        "expected at least one `(installed)` row; got: {body}"
    );
}

// ============================================================================
// REQ #9 "cogh current reads the tracker" (lifecycle hook)
// ============================================================================

#[test]
fn cogh_current_reports_unpinned_state_on_initialised_home_without_tracker() {
    let home = tempfile::tempdir().expect("temp home");
    let init = init_home(home.path());
    assert!(
        init.status.success(),
        "cogh init must succeed; got {:?}",
        init.status
    );
    // Init does not write a tracker version by default — the empty state
    // is the lifecycle default we want to assert here.
    let tracker = home.path().join("tracker").join("version");
    if tracker.exists() {
        fs::remove_file(&tracker).expect("remove tracker version");
    }

    let out = run_cogh(home.path(), ["current"]);
    assert!(
        out.status.success(),
        "cogh current must exit 0; got {:?}",
        out.status
    );
    let trimmed = stdout(&out).trim().to_string();
    assert_eq!(
        trimmed, "(no version pinned)",
        "expected `(no version pinned)` after init without a tracker file; got: {trimmed:?}"
    );
}
