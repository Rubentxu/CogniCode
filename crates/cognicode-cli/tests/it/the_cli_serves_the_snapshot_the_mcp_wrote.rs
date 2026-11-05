//! The CLI and the MCP keep ONE snapshot, not one each.
//!
//! ## What this measures
//!
//! The durable snapshot of the graph is the product's own state. It used to
//! live in the analysed tree, and it was written by one interface only: the
//! MCP's `build_graph`. The CLI rebuilt the whole graph on every run, and
//! nothing it calculated was kept.
//!
//! Now the snapshot has one owner and both interfaces go through it: what
//! one of them calculates, the other one serves. The CLI says so on stderr —
//! the channel the CLI already uses for what it is doing — and its stdout
//! stays exactly the same document it always published.
//!
//! ## Why the assertion is on the cache and on stderr
//!
//! The guarantee is about the work, not about the shape of the answer: a
//! second run over an unchanged tree must not parse the tree again. The
//! two things that show it are the snapshot in the cache (the product kept
//! what it calculated) and the line the CLI says (it served it). The stdout
//! is compared too, because a consumer that cannot tell the two runs apart
//! is the whole point: the answer does not change when the work does not.

use crate::common;

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The phrase the CLI says when it serves a graph it calculated before.
///
/// The same words the MCP uses ("cache (durable snapshot)"): one vocabulary
/// for one fact.
const SERVED_FROM_CACHE: &str = "loaded from cache";

/// Runs the CLI over `dir` with `cache` as the cache, and returns its
/// answer and what it said on stderr.
fn run_cli(dir: &Path, cache: &Path) -> (Value, String) {
    let binary = common::binary_path("cognicode");
    let output = Command::new(&binary)
        .arg("--format")
        .arg("json")
        .arg("graph")
        .arg("full")
        .arg(dir)
        .env("XDG_CACHE_HOME", cache)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", binary.display()));
    assert!(
        output.status.success(),
        "the CLI exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let answer = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "the CLI did not answer with JSON: {e}: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    (answer, String::from_utf8_lossy(&output.stderr).into_owned())
}

/// A workspace with one source file, and the temporary root that holds it.
fn workspace_with_root() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let root = tempfile::tempdir().expect("a temporary root");
    let ws = root.path().join("ws");
    std::fs::create_dir_all(&ws).expect("the workspace is created");
    std::fs::write(
        ws.join("a.rs"),
        "fn callee() -> u32 { 1 }\nfn caller() -> u32 { callee() }\n",
    )
    .expect("the source is written");
    let cache = root.path().join("cache");
    (root, ws, cache)
}

/// The answer without the time it took.
///
/// The elapsed time is the ONE thing that has to differ: the second run
/// did less work. Everything else — the symbols, the relations, the
/// verdict — is the same answer, and a consumer that reads the document
/// cannot tell which of the two runs produced it.
fn without_the_clock(mut answer: Value) -> Value {
    if let Some(object) = answer.as_object_mut() {
        object.remove("elapsed_ms");
    }
    answer
}

#[test]
fn the_second_run_of_the_cli_serves_the_snapshot_the_first_one_wrote() {
    let (_root, workspace, cache) = workspace_with_root();

    let (first, first_stderr) = run_cli(&workspace, &cache);
    assert!(
        !first_stderr.contains(SERVED_FROM_CACHE),
        "the first run calculated the graph, and it says so: a line that \
         claims it served a snapshot it had not written yet would be a lie. \
         stderr: {first_stderr}"
    );

    let snapshot = cognicode_core::application::graph_cache::snapshot_path(&cache, &workspace);
    assert!(
        snapshot.exists(),
        "the CLI keeps what it calculated: the snapshot is in the cache the \
         CLI was given, at {snapshot:?}"
    );

    let (second, second_stderr) = run_cli(&workspace, &cache);
    assert!(
        second_stderr.contains(SERVED_FROM_CACHE),
        "the second run served the graph the first one calculated, and it \
         says so: stderr: {second_stderr}"
    );

    assert_eq!(
        without_the_clock(first),
        without_the_clock(second),
        "a consumer cannot tell the served answer from the calculated one, \
         so they have to be the same answer"
    );
}
