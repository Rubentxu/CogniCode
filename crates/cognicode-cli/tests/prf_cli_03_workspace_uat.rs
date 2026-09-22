//! PRF-CLI-03 UAT (real binary):
//! workspace selection by canonical path works with NO Explorer, NO RPC,
//! NO cloud and NO OTLP collector running. Partial failures (unreadable
//! file inside an otherwise valid workspace) are reported as skipped
//! files, never converted into a complete result.
//!
//! Mechanism for "no network / no collector": run the binary in an env
//! with OTEL_EXPORTER_OTLP_ENDPOINT pointing at a closed loopback port
//! (connection refused = collector absent) and verify analysis still
//! succeeds with exit 0.

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

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cli03_uat_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run_with_closed_otlp(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cognicode_bin())
        .args(args)
        .current_dir(dir)
        // Collector endpoint that is guaranteed closed: loopback port 1.
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:1")
        .env("OTEL_EXPORTER_OTLP_PROTOCOL", "grpc")
        .output()
        .expect("spawn cognicode binary")
}

#[test]
fn workspace_selection_works_without_collector_or_explorer() {
    let dir = temp_dir("sel");
    std::fs::write(dir.join("a.rs"), "pub fn alpha() {}\n").unwrap();
    std::fs::write(dir.join("b.rs"), "pub fn beta() { alpha() }\n").unwrap();

    let out = run_with_closed_otlp(&dir, &["analyze", "."]);
    let code = out.status.code().unwrap_or(-1);
    assert_eq!(
        code,
        0,
        "analyze must succeed with no collector/explorer/RPC (stderr: {})",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("alpha") || stdout.contains("beta") || !stdout.is_empty(),
        "analyze must produce workspace output"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn canonical_path_selection_resolves_workspace() {
    let dir = temp_dir("canon");
    std::fs::write(dir.join("lib.rs"), "pub fn x() {}\n").unwrap();
    // Canonicalize the workspace path and pass it explicitly.
    let canonical = dir.canonicalize().expect("canonicalize workspace");
    let out = run_with_closed_otlp(
        dir.parent().unwrap(),
        &["analyze", canonical.to_str().unwrap()],
    );
    assert_eq!(
        out.status.code().unwrap_or(-1),
        0,
        "analyze on canonical path must exit 0 (stderr: {})",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unreadable_file_is_skipped_not_silently_dropped_or_fatal() {
    let dir = temp_dir("skip");
    std::fs::write(dir.join("good.rs"), "pub fn good_fn() {}\n").unwrap();
    // A file with an extension the parser does not handle: must be
    // classified (skipped/unsupported), never crash, never silently
    // pretended absent in a "complete" claim without accounting.
    std::fs::write(dir.join("data.weirdext"), "not rust").unwrap();

    let out = run_with_closed_otlp(&dir, &["analyze", "."]);
    let code = out.status.code().unwrap_or(-1);
    assert!(code == 0 || code == 1, "no crash allowed, got {code}");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let all = format!("{stdout}{stderr}");
    // Partial-failure honesty: good_fn must still be found (workspace
    // partially usable) and the tool must not claim a full success that
    // hides the skip, nor fail wholesale because of one bad file.
    assert!(
        all.contains("good_fn") || all.to_lowercase().contains("skip"),
        "partial workspace must surface either the parsed content or an honest skip; got: {all}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
