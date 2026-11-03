//! `reparse_on_edit` tells a file that is gone from a file it cannot see.
//!
//! Over a real stdio session, a file that was deleted is reported as
//! removed. A file whose directory cannot be read is reported as a failure
//! instead: it is not called deleted, and what the server already knew
//! about it (graph and manifest) is still there once the directory can be
//! read again.

mod common;

use std::path::PathBuf;

use common::McpSession;
use serde_json::{Value, json};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Gives a directory its permissions back when the test ends, pass or fail.
struct RestorePermissions {
    dir: PathBuf,
    original: std::fs::Permissions,
}

impl Drop for RestorePermissions {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.dir, self.original.clone());
    }
}

fn reparse_args(path: &str) -> Value {
    json!({"name": "reparse_on_edit", "arguments": {"file_paths": [path]}})
}

fn is_error(response: &Value) -> Option<bool> {
    response["result"]["isError"].as_bool()
}

fn text(response: &Value) -> String {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// The counts a successful `reparse_on_edit` published.
fn counts(response: &Value) -> (u64, u64, u64) {
    assert_ne!(
        is_error(response),
        Some(true),
        "the call was expected to succeed: {response}"
    );
    let output: Value = serde_json::from_str(&text(response))
        .unwrap_or_else(|e| panic!("the answer is not JSON ({e}): {response}"));
    let count = |key: &str| output[key].as_u64().unwrap_or(u64::MAX);
    (
        count("files_parsed"),
        count("files_skipped"),
        count("files_removed"),
    )
}

async fn reparse(session: &mut McpSession, path: &str) -> Value {
    session
        .request("tools/call", reparse_args(path))
        .await
        .expect("tools/call")
}

fn workspace_with_two_files() -> (tempfile::TempDir, PathBuf) {
    let workspace = tempfile::tempdir().expect("tempdir");
    let src = workspace.path().join("src");
    std::fs::create_dir(&src).expect("src");
    std::fs::write(src.join("main.rs"), "fn one() {}\nfn two() {}\n").expect("main.rs");
    std::fs::write(src.join("gone.rs"), "fn soon_gone() {}\n").expect("gone.rs");
    (workspace, src)
}

#[tokio::test]
#[cfg(unix)]
async fn a_deleted_file_is_removed_and_an_unreadable_one_is_a_failure() {
    let (workspace, src) = workspace_with_two_files();

    // The permission bits of a directory do not stop its owner when that
    // owner is root: without this guard the test would pass for nothing.
    std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o0)).expect("chmod");
    let can_read_anyway = std::fs::metadata(src.join("main.rs")).is_ok();
    std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    if can_read_anyway {
        eprintln!("SKIP: the process ignores directory permissions (running as root)");
        return;
    }

    let mut session = McpSession::spawn(workspace.path()).await.expect("spawn");
    let built = session
        .request(
            "tools/call",
            json!({"name": "build_graph", "arguments": {}}),
        )
        .await
        .expect("build_graph");
    assert_ne!(
        is_error(&built),
        Some(true),
        "the graph must build: {built}"
    );

    // Present: the files are read once, then found unchanged.
    let first = counts(&reparse(&mut session, "src/main.rs").await);
    counts(&reparse(&mut session, "src/gone.rs").await);
    let unchanged = counts(&reparse(&mut session, "src/main.rs").await);
    assert_eq!(
        unchanged,
        (0, 1, 0),
        "present and unchanged (first: {first:?})"
    );

    // Absent: a file known to the manifest that no longer exists.
    std::fs::remove_file(src.join("gone.rs")).expect("remove");
    let removed = counts(&reparse(&mut session, "src/gone.rs").await);
    assert_eq!(removed.2, 1, "a deleted file is reported removed");

    // Failed: the directory cannot be read. Two layers refuse it (the path
    // validator and the metadata classification); either one alone keeps the
    // answer a failure, so only losing both turns it into a deletion.
    let restore = RestorePermissions {
        dir: src.clone(),
        original: std::fs::metadata(&src).expect("metadata").permissions(),
    };
    std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o0)).expect("chmod");
    let refused = reparse(&mut session, "src/main.rs").await;
    std::fs::set_permissions(&src, restore.original.clone()).expect("restore");

    assert_eq!(
        is_error(&refused),
        Some(true),
        "an unreadable file must fail, not complete: {refused}"
    );
    let message = text(&refused).to_lowercase();
    assert!(
        !message.contains("files_removed"),
        "a failure must not carry a deletion report: {message}"
    );

    // What was known is still known: the file is unchanged, not new, not gone.
    let after = counts(&reparse(&mut session, "src/main.rs").await);
    assert_eq!(after, (0, 1, 0), "graph and manifest survived the failure");

    session.shutdown().await;
}
