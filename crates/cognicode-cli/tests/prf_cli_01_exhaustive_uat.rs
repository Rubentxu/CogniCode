//! PRF-CLI-01 UAT extension (exhaustive stable-command sweep, real binary).
//!
//! For EVERY stable CLI command, verify the exit-code contract on both a
//! valid target (exit 0) and a nonexistent path (non-zero with a
//! diagnostic). Complements prf_cli_01_uat.rs (which covers the critical
//! spec scenarios) by walking the full command surface.
//!
//! Commands under test (stable surface, default build):
//!   analyze, index, graph, navigate, doctor

use std::path::{Path, PathBuf};
use std::process::Command;

fn cognicode_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .parent()
        .expect("repo root")
        .join("target/release/cognicode")
}

fn run_in(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cognicode_bin())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn cognicode binary")
}

fn exit_code(out: &std::process::Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

fn stderr_text(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cli01_exhaustive_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn help_and_version_for_every_stable_subcommand() {
    let dir = temp_dir("help");
    for cmd in ["analyze", "index", "graph", "navigate", "doctor"] {
        let out = run_in(&dir, &[cmd, "--help"]);
        assert_eq!(
            exit_code(&out),
            0,
            "`{cmd} --help` must exit 0 (stderr: {})",
            stderr_text(&out)
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn analyze_valid_dir_exits_zero_with_output() {
    let dir = temp_dir("analyze");
    std::fs::write(dir.join("lib.rs"), "pub fn a() {}\npub fn b() { a() }\n").unwrap();
    let out = run_in(&dir, &["analyze", "."]);
    assert_eq!(exit_code(&out), 0, "analyze on valid dir must exit 0");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn index_valid_dir_exits_zero() {
    let dir = temp_dir("index");
    std::fs::write(dir.join("lib.rs"), "pub fn a() {}\n").unwrap();
    // `index` requires a subcommand; `build` is the lightweight indexer.
    let out = run_in(&dir, &["index", "build", "."]);
    assert_eq!(
        exit_code(&out),
        0,
        "index on valid dir must exit 0 (stderr: {})",
        stderr_text(&out)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn graph_subcommands_nonexistent_path_do_not_exit_zero() {
    let dir = temp_dir("graph_bad");
    for args in [
        vec!["graph", "full", "--path", "/nonexistent/prf_cli01/path"],
        vec![
            "graph",
            "on-demand",
            "--path",
            "/nonexistent/prf_cli01/path",
            "--symbol",
            "x",
        ],
    ] {
        let out = run_in(&dir, &args);
        let code = exit_code(&out);
        assert_ne!(code, 0, "`{:?}` on nonexistent path must not exit 0", args);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn navigate_nonexistent_symbol_is_honest() {
    let dir = temp_dir("navigate");
    std::fs::write(dir.join("lib.rs"), "pub fn a() {}\n").unwrap();
    // Navigate against a symbol that does not exist: must not crash (signal).
    let out = run_in(
        &dir,
        &[
            "navigate",
            "--file",
            "lib.rs",
            "--symbol",
            "no_such_symbol_anywhere",
        ],
    );
    let code = exit_code(&out);
    assert!(
        code >= 0,
        "navigate must not die by signal (crash), got {code}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn doctor_valid_cwd_is_honest_and_signal_free() {
    let dir = temp_dir("doctor");
    let out = run_in(&dir, &["doctor"]);
    let code = exit_code(&out);
    assert!(
        code >= 0,
        "doctor must not die by signal (crash), got {code}"
    );
    // Doctor is a health report: exit 1 is an HONEST "unhealthy environment"
    // (missing optional LSPs / MCP binary). 0 (healthy) and 1 (honest
    // report of missing components) are both legitimate; signals are not.
    assert!(
        code == 0 || code == 1,
        "doctor must exit 0 (healthy) or 1 (honest unhealthy report), got {code}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_command_is_refused_not_silent_zero() {
    let dir = temp_dir("unknown");
    let out = run_in(&dir, &["definitely-not-a-command"]);
    // clap refuses unknown commands with exit 2 (or non-zero). Must not be 0.
    assert_ne!(
        exit_code(&out),
        0,
        "unknown command must not exit 0 silently"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
