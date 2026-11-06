//! The CLI speaks the mermaid vocabulary the graph publishes.
//!
//! Measured: the CLI accepted a `--format` that named something the graph
//! cannot publish — "png" among them, which is not a rendering format but a
//! hint — and it passed those words straight into the field the domain
//! documents as "code"|"svg". A word the CLI did not recognise was not
//! refused either: it was answered as if it had been "txt", in silence.
//!
//! The law this file pins:
//!
//! - every format the CLI accepts publishes the diagram, and
//! - a format the graph cannot publish is refused, naming the ones it can.
//!
//! The CLI side is the real binary, run as a sub-process: the contract is
//! about what a user gets, not about an internal function.

use crate::common;

use std::path::Path;
use std::process::Command;

/// The formats the CLI accepts.
///
/// The two the graph publishes, plus the two names the CLI has always
/// published for them. The vocabulary itself lives in one place — the
/// function that translates a name into what the graph publishes — and this
/// list is the contract that pins it.
const ACCEPTED: [&str; 4] = ["code", "txt", "svg", "png"];

/// A project that declares exactly one symbol.
fn project_with_a_symbol() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("lib.rs"), "pub fn declared_symbol() {}\n").expect("write file");
    dir
}

/// Runs the CLI and returns whether it succeeded, with what it said.
fn run_cli(args: &[&str]) -> (bool, String, String) {
    let binary = common::binary_path("cognicode");
    // The product keeps what it calculates in the user's cache. The test
    // gives it a cache of its own, so the run neither writes in the cache
    // of whoever runs the suite nor reads a snapshot another run left
    // behind.
    let cache = common::isolated_cache();
    let output = Command::new(&binary)
        .args(args)
        .env("XDG_CACHE_HOME", cache.path())
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", binary.display()));
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The diagram the CLI publishes for the given format.
fn diagram_for(dir: &Path, format: &str) -> String {
    let path = dir.to_str().expect("a path");
    // The top-level `--format` is the output format; the sub-command's is
    // the mermaid one. They are two different questions.
    let (succeeded, stdout, stderr) = run_cli(&[
        "--format", "json", "graph", "mermaid", "--format", format, path,
    ]);

    assert!(
        succeeded,
        "the CLI refused a format it accepts ({format}): {stderr}"
    );

    let answer: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("the CLI did not answer with JSON: {e}: {stdout}"));

    answer["diagram"]
        .as_str()
        .expect("the CLI publishes the diagram")
        .to_string()
}

/// Every format the CLI accepts publishes the diagram.
///
/// The names the CLI has always published keep working: they are part of
/// what a user writes in a script.
#[test]
fn every_format_the_cli_accepts_publishes_the_diagram() {
    let dir = project_with_a_symbol();

    for format in ACCEPTED {
        let diagram = diagram_for(dir.path(), format);

        assert!(
            diagram.contains("flowchart"),
            "the CLI published no diagram for the format {format}: {diagram}"
        );
    }
}

/// A format the graph cannot publish is refused.
///
/// The refusal names the formats that can be published: a refusal that does
/// not say what is possible leaves the user to guess.
#[test]
fn a_format_the_graph_cannot_publish_is_refused() {
    let dir = project_with_a_symbol();
    let path = dir.path().to_str().expect("a path");

    let (succeeded, stdout, stderr) = run_cli(&["graph", "mermaid", "--format", "png2", path]);

    assert!(
        !succeeded,
        "the CLI accepted a format the graph cannot publish: {stdout}"
    );
    for name in ACCEPTED {
        assert!(
            stderr.contains(name),
            "the refusal does not name the formats the CLI accepts ({name}): {stderr}"
        );
    }
}

/// The help names the formats the CLI accepts.
///
/// The help is what a user reads before writing the command, so the names it
/// names have to be the names the command takes. The check is over WORDS:
/// "code" is a substring of "cognicode", and a substring check would call
/// the help a pass while it says nothing about the format.
#[test]
fn the_help_names_the_formats_the_cli_accepts() {
    let (succeeded, stdout, stderr) = run_cli(&["graph", "mermaid", "--help"]);

    assert!(succeeded, "the CLI did not answer with its help: {stderr}");

    let words: Vec<&str> = stdout
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();

    for name in ACCEPTED {
        assert!(
            words.contains(&name),
            "the help does not name the format {name}: {stdout}"
        );
    }
}
