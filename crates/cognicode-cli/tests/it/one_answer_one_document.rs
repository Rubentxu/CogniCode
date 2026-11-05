//! A command publishes one answer, and the presenter publishes it.
//!
//! ## What this measures
//!
//! The `graph impact-composite` command printed its answer inside the
//! core and then returned an empty `PerFile` — a graph summary with
//! nothing in it — which the presenter printed after it. The command
//! published two documents where one was wanted, and a consumer that
//! reads the whole standard output could not parse it.
//!
//! The contract: the core returns the answer and the presenter writes
//! it, so the standard output carries the answer and nothing else.
//!
//! The CLI is the surface a person reads, and it is a real binary run
//! as a sub-process: the contract is about what a user gets, not about
//! an internal function.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

use crate::common;

/// A workspace with one file, with a symbol to ask about.
fn fixture() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cognicode-one-document-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).expect("the fixture directory");
    std::fs::write(
        dir.join("lib.rs"),
        "pub fn alpha() -> u32 {\n    beta()\n}\n\npub fn beta() -> u32 {\n    2\n}\n",
    )
    .expect("the fixture file");
    dir
}

/// The composite impact publishes one document.
///
/// The whole standard output is the document: a document with a line
/// in front of it, or a document with a line behind it, is not a
/// document.
#[test]
fn the_composite_impact_publishes_one_document() {
    let dir = fixture();
    let binary = common::binary_path("cognicode");
    let output = Command::new(&binary)
        .args(["--format", "json", "graph", "impact-composite", "beta", "."])
        .current_dir(&dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", binary.display()));

    assert!(
        output.status.success(),
        "the CLI exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let document: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("the CLI did not answer with ONE JSON document: {e}: {stdout}"));
    assert!(
        document.is_object(),
        "the answer is a JSON object: {stdout}"
    );
    // The composite answer says it refuses to be a verdict; that is
    // the answer, and it is the only one.
    assert_eq!(
        document["verdict_refused"],
        Value::Bool(true),
        "the answer is the composite's own: {stdout}"
    );
}
