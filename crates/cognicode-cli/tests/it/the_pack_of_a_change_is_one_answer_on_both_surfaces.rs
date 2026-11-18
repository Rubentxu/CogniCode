//! The `pack` goal seeded by a change, on the CLI and on the MCP.
//!
//! Both surfaces run the one goal over a real Git repository, so this file
//! measures what a user gets: the `cognicode` binary as a sub-process and the
//! MCP dispatch, fed the same workspace and the same arguments.
//!
//! * The two surfaces publish one document for one change scope.
//! * Staged and working-tree scopes show exactly their own lines, and a
//!   commit does not inherit the workspace.
//! * A change that cannot be read is partial, never complete, and the pack
//!   offers no mutation or verification affordance.

use crate::common;

use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_mcp::rmcp_adapter::call_tool_handler_with_window;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

const LIB: &str = "pub fn parse(input: &str) -> u32 {\n    input.len() as u32\n}\n\nfn helper(x: u32) -> u32 {\n    x + 1\n}\n\npub fn run() -> u32 {\n    parse(\"x\") + helper(1)\n}\n\n#[test]\nfn parse_counts_bytes() {\n    let n = parse(\"ab\");\n    assert_eq!(n, 2);\n}\n";
const OTHER: &str = "pub fn unrelated() -> u32 {\n    1\n}\n";

fn git(repo: &Path, args: &[&str]) {
    let none = repo.join("no-git-configuration");
    let output = Command::new("git")
        .env("GIT_CONFIG_GLOBAL", &none)
        .env("GIT_CONFIG_SYSTEM", &none)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git is available");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write(repo: &Path, relative: &str, text: &str) {
    let path = repo.join(relative);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("directory");
    std::fs::write(path, text).expect("file");
}

fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("workspace");
    let repo = directory.path();
    git(repo, &["init", "-q", "-b", "main"]);
    write(repo, "src/lib.rs", LIB);
    write(repo, "src/other.rs", OTHER);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "base"]);
    directory
}

/// The arguments as terminal flags.
fn flags_of(arguments: &Value) -> Vec<String> {
    let mut flags = Vec::new();
    for (name, value) in arguments.as_object().expect("an object") {
        flags.push(format!("--{}", name.replace('_', "-")));
        flags.push(
            value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string()),
        );
    }
    flags
}

fn cli_envelope(workspace: &Path, arguments: &Value) -> Value {
    let cache = common::isolated_cache();
    let output = Command::new(common::binary_path("cognicode"))
        .current_dir(workspace)
        .args(["pack", "--format", "json"])
        .args(flags_of(arguments))
        .env("XDG_CACHE_HOME", cache.path())
        .output()
        .expect("the CLI starts");
    assert!(
        output.status.success(),
        "the CLI exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("the CLI answers with JSON")
}

async fn mcp_answer(workspace: &Path, arguments: &Value) -> Value {
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build();
    let request = CallToolRequestParams::new("pack".to_string())
        .with_arguments(arguments.as_object().cloned().unwrap_or_default());
    let text = call_tool_handler_with_window(&ctx, request, None)
        .await
        .expect("the MCP answers");
    serde_json::from_str(&text).expect("the MCP answers with JSON")
}

fn names(answer: &Value) -> Vec<String> {
    answer["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["name"].as_str().expect("a name").to_string())
        .collect()
}

#[tokio::test]
async fn a_change_scope_is_one_answer_on_both_surfaces() {
    let repo = repository();
    write(
        repo.path(),
        "src/lib.rs",
        &LIB.replace(
            "fn parse(input: &str) -> u32",
            "fn parse(input: &str) -> u64",
        ),
    );

    let arguments = json!({"change_scope": "working_tree"});
    let envelope = cli_envelope(repo.path(), &arguments);
    let cli = envelope["result"].clone();
    let mcp = mcp_answer(repo.path(), &arguments).await;
    assert_eq!(cli, mcp, "the two surfaces publish one answer");

    let reasons: Vec<(String, String)> = cli["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| {
            (
                item["name"].as_str().expect("a name").to_string(),
                item["reason"].as_str().expect("a reason").to_string(),
            )
        })
        .collect();
    for expected in [
        ("parse", "changed_symbol"),
        ("run", "consumer_of_change"),
        ("parse_counts_bytes", "test_of_change"),
    ] {
        assert!(
            reasons.contains(&(expected.0.to_string(), expected.1.to_string())),
            "{expected:?} is selected with its reason: {cli}"
        );
    }
    assert_eq!(reasons.len(), 3, "nothing else is selected: {cli}");
    assert_eq!(cli["change"]["scope"], "working_tree");
    assert_eq!(
        cli["change"]["files"][0]["changed_lines"][0]["start_line"],
        1
    );
    assert!(cli["change"]["advisory"].is_string());
}

#[tokio::test]
async fn staged_and_a_commit_each_show_only_their_own_scope() {
    let repo = repository();
    write(
        repo.path(),
        "src/other.rs",
        "pub fn unrelated() -> u32 {\n    2\n}\n",
    );
    git(repo.path(), &["add", "src/other.rs"]);
    write(repo.path(), "src/lib.rs", &LIB.replace("x + 1", "x + 2"));

    let staged = json!({"change_scope": "staged"});
    let cli = cli_envelope(repo.path(), &staged)["result"].clone();
    assert_eq!(cli, mcp_answer(repo.path(), &staged).await);
    assert_eq!(
        names(&cli),
        ["unrelated"],
        "the workspace edit is not staged"
    );

    git(repo.path(), &["commit", "-q", "-m", "other"]);
    let commit = json!({"change_scope": "commit", "head": "HEAD"});
    let cli = cli_envelope(repo.path(), &commit)["result"].clone();
    assert_eq!(cli, mcp_answer(repo.path(), &commit).await);
    assert_eq!(
        names(&cli),
        ["unrelated"],
        "a commit inherits no workspace edit"
    );
    assert_eq!(cli["change"]["files"][0]["path"], "src/other.rs");
}

#[tokio::test]
async fn a_pack_of_a_change_is_partial_and_offers_no_mutation_or_verification() {
    let repo = repository();
    write(repo.path(), "src/lib.rs", &LIB.replace("x + 1", "x + 2"));

    let arguments = json!({"change_scope": "working_tree"});
    let envelope = cli_envelope(repo.path(), &arguments);
    let answer = &envelope["result"];
    assert_eq!(names(answer), ["helper"]);
    assert_eq!(answer["status"], "partial", "missing coverage is not clean");
    assert_eq!(answer["change"]["gaps"][0]["kind"], "no_test_found");

    let links = envelope["links"].as_array().cloned().unwrap_or_default();
    for link in &links {
        let tool = link["tool"].as_str().unwrap_or_default();
        assert!(
            !["verify", "edit", "apply", "write"]
                .iter()
                .any(|banned| tool.contains(banned)),
            "a pack offers no mutation or verification: {link}"
        );
    }
}

#[tokio::test]
async fn a_scope_that_cannot_be_read_is_refused_on_both_surfaces() {
    let directory = tempfile::tempdir().expect("workspace");
    write(directory.path(), "src/lib.rs", LIB);

    let cache = common::isolated_cache();
    let output = Command::new(common::binary_path("cognicode"))
        .current_dir(directory.path())
        .args(["pack", "--format", "json", "--change-scope", "staged"])
        .env("XDG_CACHE_HOME", cache.path())
        .output()
        .expect("the CLI starts");
    assert!(!output.status.success(), "no repository, no change");

    let ctx = HandlerContext::builder()
        .with_working_dir(directory.path().to_path_buf())
        .build();
    let request = CallToolRequestParams::new("pack".to_string()).with_arguments(
        json!({"change_scope": "staged"})
            .as_object()
            .cloned()
            .unwrap(),
    );
    assert!(
        call_tool_handler_with_window(&ctx, request, None)
            .await
            .is_err()
    );
}
