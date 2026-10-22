//! Un análisis sobre un workspace sin material no se publica como un
//! análisis perfecto.
//!
//! ## El defecto que este contrato mide
//!
//! `cognicode analyze <dir>` walks the workspace, asks for the architecture
//! of what it found, and prints the score. When the workspace has nothing
//! analyzable —an empty directory, or a directory with only a text file—
//! the walk returns a graph with zero symbols, the architecture check finds
//! no cycles and no violations, and the command printed:
//!
//! ```
//!   Score: 100.0/100
//! ```
//!
//! A perfect score over nothing. The caller cannot tell it apart from a
//! perfect score over a well-structured project, and the number is the one
//! thing an agent reads.
//!
//! ## Lo que no cambia
//!
//! The exit code. A valid directory with nothing to analyze is an operation
//! that ran, and `cli_exit_code_propagation.rs` fixes that exit code at 0.
//! What changes is the claim: the command says what it found instead of
//! publishing a score about a project nobody looked at.
//!
//! ## Los dos sentidos
//!
//! The empty workspace alone would also pass if the command stopped
//! publishing scores altogether. So the happy case is here too: a workspace
//! with code in it still gets a score.

use std::path::PathBuf;
use std::process::{Command, Output};

mod common;

fn cognicode_bin() -> PathBuf {
    common::binary_path("cognicode")
}

fn run(args: &[&str]) -> Output {
    Command::new(cognicode_bin())
        .args(args)
        .output()
        .expect("spawn cognicode binary")
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

/// A workspace with no analyzable source gets no score, and says why.
#[test]
fn analyze_over_a_workspace_without_material_publishes_no_score() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(tmp.path().join("notes.txt"), "sin codigo\n").expect("write");

    let out = run(&["analyze", tmp.path().to_str().unwrap()]);
    let stdout = stdout_of(&out);
    let stderr = stderr_of(&out);

    assert!(
        !stdout.contains("Score:"),
        "un workspace sin material analizable no puede puntuarse: {stdout}"
    );
    assert!(
        stderr.contains("no produjo símbolos"),
        "la respuesta tiene que decir por que no hay puntuación: {stderr}"
    );
}

/// The happy case: a workspace with code still gets a score. A contract
/// that only forbids the score would also pass if the command stopped
/// scoring anything.
#[test]
fn analyze_over_a_workspace_with_code_still_publishes_a_score() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("lib.rs"),
        "pub fn answer() -> u8 { 42 }\n",
    )
    .expect("write");

    let out = run(&["analyze", tmp.path().to_str().unwrap()]);
    let stdout = stdout_of(&out);

    assert!(
        stdout.contains("Score:"),
        "un workspace con material sigue puntuándose: {stdout}"
    );
}
