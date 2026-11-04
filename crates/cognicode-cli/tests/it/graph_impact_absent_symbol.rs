//! `cognicode graph impact` on a symbol the project graph does not hold must
//! say its impact is unknown. "No impact" and "risk NONE" are answers about a
//! symbol that exists; they must never be given for one that does not.

use std::path::Path;
use std::process::{Command, Output};

use crate::common;

fn impact(symbol: &str, project: &Path, format: &str) -> Output {
    Command::new(common::binary_path("cognicode"))
        .args(["--format", format, "graph", "impact", symbol])
        .arg(project)
        .output()
        .expect("run cognicode")
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "pub fn present() {}\npub fn caller() { present(); }\n",
    )
    .unwrap();
    dir
}

#[test]
fn an_absent_symbol_fails_and_says_its_impact_is_unknown() {
    let dir = project();
    let out = impact("never_defined_anywhere", dir.path(), "json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(!out.status.success(), "absent symbol exited 0: {stdout}");
    assert!(
        stderr.contains("not in the project graph") && stderr.contains("unknown"),
        "stderr must say the impact is unknown: {stderr}"
    );
    assert!(
        !stdout.contains("\"risk_level\""),
        "no risk may be reported for an absent symbol: {stdout}"
    );
}

#[test]
fn a_present_symbol_still_reports_its_dependents() {
    let dir = project();
    let out = impact("present", dir.path(), "json");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(out.status.success(), "present symbol failed: {stdout}");
    assert!(
        stdout.contains("\"caller\""),
        "caller not impacted: {stdout}"
    );
}
