//! The `preflight` goal on the CLI and on the MCP.
//!
//! The goal is declared once and both surfaces run the same function, so what
//! this file measures is what a user gets: the real `cognicode` binary run as
//! a sub-process, and the MCP dispatch fed the same workspace and the same
//! edits.
//!
//! * The two surfaces answer with the same document.
//! * Asking leaves the workspace tree and Git exactly as they were.

use crate::common;

use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_mcp::rmcp_adapter::call_tool_handler_with_window;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> String {
    let none = root.join("no-git-configuration");
    let output = Command::new("git")
        .env("GIT_CONFIG_GLOBAL", &none)
        .env("GIT_CONFIG_SYSTEM", &none)
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git is installed");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8")
}

fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("repository");
    let root = directory.path();
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "CogniCode tests"]);
    git(root, &["config", "user.email", "tests@example.invalid"]);
    std::fs::write(root.join("a.rs"), "pub fn alpha() { first(); }\n").expect("a.rs");
    std::fs::write(root.join("b.rs"), "pub fn beta() {}\n").expect("b.rs");
    git(root, &["add", "-A"]);
    git(root, &["commit", "--quiet", "-m", "base"]);
    directory
}

fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else {
                let name = path.strip_prefix(root).expect("under root");
                files.insert(
                    name.to_string_lossy().into_owned(),
                    std::fs::read(&path).expect("read"),
                );
            }
        }
    }
    files
}

fn edits() -> Value {
    json!({
        "a.rs": "pub fn alpha() { second(); }\n",
        "c.rs": "pub fn gamma() {}\n",
        "b.rs": null,
    })
}

fn cli_answer(workspace: &Path, edits: &Value) -> Value {
    let cache = common::isolated_cache();
    let output = Command::new(common::binary_path("cognicode"))
        .current_dir(workspace)
        .args([
            "preflight",
            "--format",
            "json",
            "--edits",
            &edits.to_string(),
        ])
        .env("XDG_CACHE_HOME", cache.path())
        .output()
        .expect("the CLI starts");
    assert!(
        output.status.success(),
        "the CLI exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value =
        serde_json::from_slice(&output.stdout).expect("the CLI answers with JSON");
    envelope["result"].clone()
}

async fn mcp_answer(workspace: &Path, edits: &Value) -> Value {
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build();
    let request = CallToolRequestParams::new("preflight".to_string()).with_arguments(
        json!({ "edits": edits })
            .as_object()
            .cloned()
            .unwrap_or_default(),
    );
    let text = call_tool_handler_with_window(&ctx, request, None)
        .await
        .expect("the MCP answers");
    serde_json::from_str(&text).expect("the MCP answers with JSON")
}

#[tokio::test]
async fn the_cli_and_the_mcp_answer_with_the_same_document() {
    let repo = repository();

    let from_cli = cli_answer(repo.path(), &edits());
    let from_mcp = mcp_answer(repo.path(), &edits()).await;

    assert_eq!(from_cli, from_mcp, "the two surfaces publish one answer");
    assert_eq!(from_cli["goal"], "preflight");
    assert_eq!(from_cli["cost"]["changed_paths"], 3);
    assert_eq!(from_cli["persisted"], false);
    assert!(
        from_cli["candidate"]["handle"]
            .as_str()
            .is_some_and(|handle| handle.starts_with("overlay:"))
    );
}

#[tokio::test]
async fn asking_from_either_surface_leaves_the_workspace_as_it_was() {
    let repo = repository();
    let tree_before = tree(repo.path());
    let git_before = git(
        repo.path(),
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    let head_before = git(repo.path(), &["rev-parse", "HEAD"]);

    cli_answer(repo.path(), &edits());
    mcp_answer(repo.path(), &edits()).await;

    assert_eq!(tree(repo.path()), tree_before);
    assert_eq!(
        git(
            repo.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        git_before
    );
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD"]), head_before);
}
