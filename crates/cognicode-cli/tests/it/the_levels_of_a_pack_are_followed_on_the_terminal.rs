//! The terminal follows the levels of knowledge a `pack` opens.
//!
//! The `cognicode` binary answers a pack in JSON with the envelope the MCP
//! publishes. The consumers link it offers names a symbol by identity, and
//! the terminal's own `trace` answers it with the callers the pack counted.

use crate::common;

use serde_json::Value;
use std::path::Path;
use std::process::Command;

const LIB: &str = "pub fn leaf() -> u32 {\n    1\n}\n\npub fn left() -> u32 {\n    leaf()\n}\n\npub fn right() -> u32 {\n    leaf() + 1\n}\n";

fn workspace() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("workspace");
    let src = directory.path().join("src");
    std::fs::create_dir_all(&src).expect("src");
    std::fs::write(src.join("lib.rs"), LIB).expect("lib.rs");
    directory
}

fn cognicode(workspace: &Path, cache: &Path, args: &[&str]) -> Value {
    let output = Command::new(common::binary_path("cognicode"))
        .current_dir(workspace)
        .args(args)
        .env("XDG_CACHE_HOME", cache)
        .output()
        .expect("the CLI starts");
    assert!(
        output.status.success(),
        "{args:?} exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("the CLI answers with JSON")
}

#[test]
fn the_consumers_link_of_a_pack_is_a_trace_the_terminal_answers() {
    let repo = workspace();
    let cache = common::isolated_cache();
    let envelope = cognicode(
        repo.path(),
        cache.path(),
        &["pack", "--intent", "leaf", "--format", "json"],
    );

    let leaf = envelope["result"]["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|item| item["name"] == "leaf")
        .expect("the pack delivers the symbol it was asked about");
    let reach = envelope["result"]["navigation"]["reach"]
        .as_array()
        .expect("reach")
        .iter()
        .find(|reach| reach["id"] == leaf["id"])
        .expect("what expanding it reaches");
    assert_eq!(reach["direct_callers"], 2, "{envelope}");

    let links = envelope["_links"].as_array().expect("links");
    let consumers = links
        .iter()
        .find(|link| link["rel"] == "trace-consumers")
        .expect("the pack offers the way back to the consumers");
    assert_eq!(consumers["tool"], "trace");
    assert_eq!(consumers["authority"], "read");
    assert!(
        consumers["handle"]
            .as_str()
            .is_some_and(|handle| handle.starts_with("sym:")),
        "{consumers}"
    );

    let symbol = consumers["required_args"]["symbol"].as_str().expect("id");
    let traced = cognicode(
        repo.path(),
        cache.path(),
        &[
            "trace",
            "--symbol",
            symbol,
            "--direction",
            "callers",
            "--format",
            "json",
        ],
    );
    let text = traced.to_string();
    assert!(
        text.contains("left") && text.contains("right"),
        "the trace names the callers the pack counted: {text}"
    );
}
