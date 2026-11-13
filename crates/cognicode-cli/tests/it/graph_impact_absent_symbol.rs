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

/// Two modules that each declare a `helper`, each with its own caller.
fn project_with_homonyms() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "pub mod a;\npub mod b;\n").unwrap();
    std::fs::write(
        dir.path().join("src/a.rs"),
        "pub fn helper() {}\npub fn caller_of_a() { helper(); }\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/b.rs"),
        "pub fn helper() {}\npub fn caller_of_b() { helper(); }\n",
    )
    .unwrap();
    dir
}

#[test]
fn a_name_two_symbols_carry_is_refused_with_its_candidates() {
    let dir = project_with_homonyms();
    let out = impact("helper", dir.path(), "json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        !out.status.success(),
        "an ambiguous name exited 0: {stdout}"
    );
    assert!(
        stderr.contains("ambiguous")
            && stderr.contains("src/a.rs:helper:")
            && stderr.contains("src/b.rs:helper:"),
        "the refusal must name both candidates: {stderr}"
    );
    assert!(
        !stdout.contains("\"risk_level\""),
        "no risk may be reported for a name that is not one symbol: {stdout}"
    );
}

#[test]
fn the_identity_of_one_homonym_reaches_only_its_own_callers() {
    let dir = project_with_homonyms();
    let in_a = format!("{}:helper:0", dir.path().join("src/a.rs").display());
    let out = impact(&in_a, dir.path(), "json");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success(),
        "the identity names one symbol: {stdout} {stderr}"
    );
    assert!(
        stdout.contains("\"caller_of_a\""),
        "the caller of this helper is impacted: {stdout}"
    );
    assert!(
        !stdout.contains("\"caller_of_b\""),
        "the caller of the OTHER helper is not impacted: {stdout}"
    );
}
