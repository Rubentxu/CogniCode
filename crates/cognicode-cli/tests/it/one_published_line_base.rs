//! The product publishes every line with one base: one.
//!
//! ## What this measures
//!
//! A line number is a position, and a position is only a position if
//! everybody counts the same way. The product had two ways of
//! counting: the graph's positions are zero-indexed (that is what the
//! parser that reads a file hands out) and the code-intelligence
//! positions are one-indexed (that is what a person counts), and every
//! surface that published a graph position published it zero-indexed
//! while the surface next to it published one-indexed. A consumer that
//! reads two tools of the same product and compares the line of the
//! same symbol gets two different answers, and nothing in the
//! published schemas said which was which.
//!
//! The contract this file pins: **the published line is the line the
//! file declares** — the one a person sees in their editor — and it is
//! one-indexed, on every surface.
//!
//! ## Why the fixture carries known lines
//!
//! The expected line is a fact about the file, not a number copied
//! from the implementation. `alpha` is declared on line 1 and `beta`
//! on line 5 of the fixture below; a zero-indexed publication would
//! answer 0 and 4.
//!
//! ## Why the CLI is the place to measure it
//!
//! The CLI is the surface a person reads, and it is a real binary
//! run as a sub-process: the contract is about what a user gets, not
//! about an internal function.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

use crate::common;

/// A file whose declarations are on known lines.
///
/// `alpha` is declared on line 1 and `beta` on line 5; the call to
/// `beta` is on line 2.
const FIXTURE: &str = "\
pub fn alpha() -> u32 {
    beta()
}

pub fn beta() -> u32 {
    2
}
";

/// A workspace with one file, whose declarations are on known lines.
fn fixture() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cognicode-one-line-base-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).expect("the fixture directory");
    std::fs::write(dir.join("lib.rs"), FIXTURE).expect("the fixture file");
    dir
}

/// The CLI's answer, parsed as the document it claims to be.
fn run_cli(args: &[&str]) -> Value {
    let binary = common::binary_path("cognicode");
    let output = Command::new(&binary)
        .arg("--format")
        .arg("json")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", binary.display()));
    assert!(
        output.status.success(),
        "the CLI exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "the CLI did not answer with one JSON document: {e}: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

/// The line the file declares the symbol on.
fn declared_line(symbol: &str) -> Option<u64> {
    FIXTURE
        .lines()
        .position(|line| line.contains(&format!("fn {symbol}(")))
        .map(|index| index as u64 + 1)
}

/// Every symbol the answer publishes, with the line it published.
fn published(answer: &Value) -> Vec<(String, u64)> {
    answer["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("the answer publishes a list of symbols: {answer}"))
        .iter()
        .map(|entry| {
            (
                entry["name"]
                    .as_str()
                    .unwrap_or_else(|| panic!("the entry names a symbol: {entry}"))
                    .to_string(),
                entry["line"]
                    .as_u64()
                    .unwrap_or_else(|| panic!("the entry publishes a line: {entry}")),
            )
        })
        .collect()
}

/// Every symbol the answer publishes says the line the file declares it on.
fn assert_every_symbol_says_the_line_the_file_declares(answer: &Value) {
    let symbols = published(answer);
    assert!(
        !symbols.is_empty(),
        "the answer publishes at least one symbol: {answer}"
    );
    for (name, line) in symbols {
        let Some(declared) = declared_line(&name) else {
            continue;
        };
        assert_eq!(
            line, declared,
            "the published line of {name} is not the line the file declares it on: {answer}"
        );
    }
}

/// The graph publishes the line the file declares.
///
/// The entry points of the fixture are the functions nobody calls, and
/// the answer says where each one lives. The line it says is the line
/// of the file.
#[test]
fn the_graph_publishes_the_line_the_file_declares() {
    let dir = fixture();
    let answer = run_cli(&["graph", "entry-points", &dir.to_string_lossy()]);
    assert_every_symbol_says_the_line_the_file_declares(&answer);
}

/// The leaf functions publish the line the file declares.
///
/// The leaf functions are the other end of the same graph, and they
/// answer with the same kind of position. They used to answer with
/// the same base as the entry points and a different one as the hot
/// paths, which is how one product ended up counting lines two ways.
#[test]
fn the_leaf_functions_publish_the_line_the_file_declares() {
    let dir = fixture();
    let answer = run_cli(&["graph", "leaf-functions", &dir.to_string_lossy()]);
    assert_every_symbol_says_the_line_the_file_declares(&answer);
}

/// The navigation answers about the line the user asked for.
///
/// `navigate definition <file>:<line>:<column>` takes the line the
/// user reads — one-indexed — and asks the backend about that line.
/// The answer says where the definition lives, and it says it in the
/// same base: the definition of `beta` is on line 5 of the fixture,
/// not on line 6.
///
/// The arithmetic that used to sit here (`line - 1` on the way in,
/// `line + 1` on the way out) asked the backend about the line above
/// the one the user named, and then published the answer one line
/// below where it really was.
#[test]
fn the_navigation_answers_about_the_line_the_user_asked_for() {
    let dir = fixture();
    let file = dir.join("lib.rs");
    // The call to `beta` is on line 2 of the fixture; `beta` itself is
    // declared on line 5.
    let position = format!("{}:2:5", file.display());
    let answer = run_cli(&["navigate", "definition", &position, &dir.to_string_lossy()]);

    assert_eq!(
        answer["kind"], "definition",
        "the CLI answered with another command: {answer}"
    );
    assert_eq!(
        answer["location"]["line"], 5,
        "the answer does not say where the definition lives: {answer}"
    );
}
