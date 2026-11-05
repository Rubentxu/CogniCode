//! In JSON mode the standard output is the document, and nothing else.
//!
//! ## What this measures
//!
//! The CLI has two channels. The standard output carries the product:
//! the JSON document, or the text of the answer. The standard error
//! carries the observability: the progress lines and the warnings.
//!
//! Four graph commands announce their work before they answer
//! —`trace-path`, `impact`, `hierarchy` and `mermaid`— and that
//! announcement used to land in the standard output, in front of the
//! document. A consumer that hands the whole standard output to a JSON
//! parser received a document with a line of prose in front of it, and
//! could not parse it.
//!
//! The contract: in JSON mode the standard output is exactly one JSON
//! document, and the progress line travels in the standard error where
//! the user still reads it.
//!
//! The CLI is the surface a person reads, and it is a real binary run
//! as a sub-process: the contract is about what a user gets, not about
//! an internal function.

use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use crate::common;

/// How long one command may take before the test gives up on it.
///
/// A test that waits for a process with no deadline is a test that may
/// never end: it holds the suite, it holds every mutation run that
/// includes it, and it hides the failure behind a timeout that says
/// nothing about what happened.
const DEADLINE: Duration = Duration::from_secs(60);

/// A workspace with one file, holding a chain of three functions.
fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("the temporary workspace");
    std::fs::write(
        dir.path().join("lib.rs"),
        "fn a() { b(); }\nfn b() { c(); }\nfn c() {}\n",
    )
    .expect("the fixture file");
    dir
}

/// Runs the command and returns what it published, or fails if it does
/// not finish in time.
///
/// The pipes are read in their own threads so a full buffer cannot
/// block the child, and the wait carries a deadline so a child that
/// hangs cannot hold the suite.
fn run(binary: &Path, args: &[&str], cwd: &Path) -> Output {
    let mut child = Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", binary.display()));

    let mut stdout = child.stdout.take().expect("the stdout pipe");
    let mut stderr = child.stderr.take().expect("the stderr pipe");
    let out_reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut buffer = Vec::new();
        let _ = stdout.read_to_end(&mut buffer);
        buffer
    });
    let err_reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut buffer = Vec::new();
        let _ = stderr.read_to_end(&mut buffer);
        buffer
    });

    let deadline = Instant::now() + DEADLINE;
    let status = loop {
        match child.try_wait().expect("the child status") {
            Some(status) => break status,
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("the CLI did not finish in {DEADLINE:?}: {args:?}");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };

    let stdout = out_reader.join().expect("the stdout reader");
    let stderr = err_reader.join().expect("the stderr reader");

    Output {
        status,
        stdout,
        stderr,
    }
}

/// The standard output of one graph command, in JSON mode.
fn stdout_of(graph_args: &[&str], dir: &Path) -> String {
    let binary = common::binary_path("cognicode");
    let path = dir.to_string_lossy().into_owned();
    let mut args = vec!["--format", "json", "graph"];
    args.extend_from_slice(graph_args);
    args.push(&path);

    let output = run(&binary, &args, dir);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The standard output is the whole document, so a document with a
    // line in front of it, or a line behind it, is not a document. The
    // standard error is in the message because that is where the line
    // that broke it went.
    if let Err(e) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
        panic!(
            "graph {graph_args:?}: the standard output is not ONE JSON document ({e}):\n\
             --- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        );
    }

    stdout
}

/// The trace path publishes one document.
#[test]
fn the_trace_path_publishes_one_document() {
    let dir = workspace();
    stdout_of(&["trace-path", "a", "c"], dir.path());
}

/// The impact publishes one document.
#[test]
fn the_impact_publishes_one_document() {
    let dir = workspace();
    stdout_of(&["impact", "c"], dir.path());
}

/// The hierarchy publishes one document.
#[test]
fn the_hierarchy_publishes_one_document() {
    let dir = workspace();
    stdout_of(&["hierarchy", "b"], dir.path());
}

/// The mermaid export publishes one document.
#[test]
fn the_mermaid_publishes_one_document() {
    let dir = workspace();
    stdout_of(&["mermaid"], dir.path());
}
