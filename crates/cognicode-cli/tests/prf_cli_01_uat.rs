//! PRF-CLI-01 UAT: argv, exit code, diagnóstico por comando stable.
//!
//! Contract (SPEC-CLI.md PRF-CLI-01 MUST): `--help`/`--version` and
//! every stable command have a documented exit code, argv surface and
//! diagnostic; **no `exit 0` when the operation was not performed**.
//! Scenario required by the SPEC: nonexistent path and invalid
//! configuration → NOT success.
//!
//! These tests run the real `cognicode` binary (integration test via
//! `CARGO_BIN_EXE_cognicode`) so the assertion covers the actual
//! process exit code, not an in-process approximation.

use std::path::Path;
use std::process::{Command, Output};

fn cognicode_bin() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cognicode"))
}

fn run(args: &[&str]) -> Output {
    Command::new(cognicode_bin())
        .args(args)
        .output()
        .expect("spawn cognicode binary")
}

fn exit_code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

/// `--help` and `--version` must succeed.
#[test]
fn help_and_version_exit_zero() {
    for args in [&["--help"][..], &["--version"][..]] {
        let out = run(args);
        assert_eq!(
            exit_code(&out),
            0,
            "`cognicode {:?}` must exit 0, got {} (stderr: {})",
            args,
            exit_code(&out),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// PRF-CLI-01 scenario: nonexistent path passed to `analyze` must NOT
/// exit 0. The operation was not performed; a zero exit would lie to
/// scripts and CI.
#[test]
fn analyze_nonexistent_path_does_not_exit_zero() {
    let out = run(&["analyze", "/nonexistent/prf_cli_01/definitely/missing"]);
    assert_ne!(
        exit_code(&out),
        0,
        "`analyze <nonexistent>` must not exit 0; the operation did not \
         happen. stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// PRF-CLI-01 scenario: nonexistent path passed to `graph full` must
/// NOT exit 0 (already the case; pins the contract).
#[test]
fn graph_full_nonexistent_path_does_not_exit_zero() {
    let out = run(&["graph", "full", "--path", "/nonexistent/prf_cli_01/missing"]);
    assert_ne!(
        exit_code(&out),
        0,
        "`graph full --path <nonexistent>` must not exit 0. stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// PRF-CLI-01 scenario: `doctor` on a nonexistent cwd must NOT exit 0
/// (already the case; pins the contract).
#[test]
fn doctor_nonexistent_cwd_does_not_exit_zero() {
    let out = run(&["doctor", "--cwd", "/nonexistent/prf_cli_01/missing"]);
    assert_ne!(
        exit_code(&out),
        0,
        "`doctor --cwd <nonexistent>` must not exit 0. stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// PRF-CLI-01 scenario: `analyze` on a valid empty directory IS a
/// successful operation and must exit 0 (guards against over-tightening
/// the fix into "analyze always fails").
#[test]
fn analyze_valid_empty_dir_exits_zero() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = run(&["analyze", tmp.path().to_str().unwrap()]);
    assert_eq!(
        exit_code(&out),
        0,
        "`analyze <valid empty dir>` must exit 0. stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
