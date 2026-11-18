//! The `dependencies` goal on the CLI and on the MCP.
//!
//! The goal is declared once and both surfaces run the same function, so what
//! this file measures is what a user gets: the real `cognicode` binary run as
//! a sub-process, and the MCP dispatch fed the same repository and the same
//! question.
//!
//! * The two surfaces answer with the same document, for the inventory, for
//!   the transitive dependents and for the lockfile delta.
//! * An SBOM supplied as a workspace file is compared with the inventory the
//!   same way on both, and a pinned digest that does not match refuses it on
//!   both.
//! * Asking leaves the workspace tree and Git exactly as they were.

use crate::common;

use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_mcp::rmcp_adapter::call_tool_handler_with_window;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../sandbox/fixtures/dependency-inventory/cargo-workspace"
);

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

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create directory");
    for entry in std::fs::read_dir(from).expect("read fixture") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy file");
        }
    }
}

/// The fixture workspace committed twice: the second commit bumps `memchr`.
fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("repository");
    let root = directory.path();
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "CogniCode tests"]);
    git(root, &["config", "user.email", "tests@example.invalid"]);
    copy_tree(Path::new(FIXTURE), root);
    git(root, &["add", "-A"]);
    git(root, &["commit", "--quiet", "-m", "base"]);
    git(root, &["tag", "base"]);
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).expect("lock");
    std::fs::write(root.join("Cargo.lock"), lock.replace("2.8.3", "2.8.4")).expect("lock");
    git(root, &["commit", "--quiet", "-am", "bump memchr"]);
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

fn question() -> Value {
    json!({ "dependents_of": "memchr", "base_ref": "base" })
}

/// The question as the flags of the CLI: every key is the flag of the same
/// name with dashes.
fn flags(question: &Value) -> Vec<String> {
    let mut flags = Vec::new();
    for (key, value) in question.as_object().expect("an object") {
        flags.push(format!("--{}", key.replace('_', "-")));
        flags.push(value.as_str().expect("a string").to_string());
    }
    flags
}

fn cli_answer(workspace: &Path, question: &Value) -> Value {
    let cache = common::isolated_cache();
    let output = Command::new(common::binary_path("cognicode"))
        .current_dir(workspace)
        .args(["dependencies", "--format", "json"])
        .args(flags(question))
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

async fn mcp_answer(workspace: &Path, question: &Value) -> Value {
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build();
    let request = CallToolRequestParams::new("dependencies".to_string())
        .with_arguments(question.as_object().cloned().unwrap_or_default());
    let text = call_tool_handler_with_window(&ctx, request, None)
        .await
        .expect("the MCP answers");
    serde_json::from_str(&text).expect("the MCP answers with JSON")
}

#[tokio::test]
async fn the_cli_and_the_mcp_answer_with_the_same_document() {
    let repo = repository();

    let from_cli = cli_answer(repo.path(), &question());
    let from_mcp = mcp_answer(repo.path(), &question()).await;

    assert_eq!(from_cli, from_mcp, "the two surfaces publish one answer");
    assert_eq!(from_cli["goal"], "dependencies");
    let unit = &from_cli["units"][0];
    assert_eq!(unit["freshness"], "consistent");
    assert_eq!(unit["packages"].as_array().map(Vec::len), Some(10));
    let affected = &from_cli["dependents"][0]["report"]["affected_members"];
    // Nearest dependents first: `util` depends on `memchr` directly.
    assert_eq!(affected, &json!(["util@0.1.0", "app@0.3.0"]));
    let updated = &from_cli["comparison"]["units"][0]["delta"]["updated"];
    assert_eq!(updated[0]["name"], "memchr");
    assert_eq!(updated[0]["to"], json!(["2.8.4"]));
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

    cli_answer(repo.path(), &question());
    mcp_answer(repo.path(), &question()).await;

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

const SBOM: &str = "cargo-workspace.cargo-cyclonedx.cdx.json";

fn sbom_bytes() -> Vec<u8> {
    std::fs::read(Path::new(FIXTURE).join("..").join(SBOM)).expect("the SBOM fixture")
}

fn sbom_question(digest: &str) -> Value {
    json!({
        "sbom": SBOM,
        "sbom_expected_digest": format!("sha256:{digest}"),
        "artifact_digest": "1".repeat(64),
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    cognicode_core::domain::scm::ContentDigest::from_bytes(bytes)
        .as_hex()
        .to_string()
}

#[tokio::test]
async fn an_sbom_is_compared_the_same_way_on_both_surfaces() {
    let repo = repository();
    std::fs::write(repo.path().join(SBOM), sbom_bytes()).expect("sbom");
    let digest = sha256_hex(&sbom_bytes());

    let from_cli = cli_answer(repo.path(), &sbom_question(&digest));
    let from_mcp = mcp_answer(repo.path(), &sbom_question(&digest)).await;

    assert_eq!(from_cli, from_mcp, "the two surfaces publish one answer");
    let distributed = &from_cli["distributed"];
    assert_eq!(distributed["status"], "usable");
    assert_eq!(distributed["document"]["digest"], json!(digest));
    assert_eq!(distributed["document"]["pinned"], json!(true));
    let unit = &distributed["units"][0];
    assert_eq!(unit["evaluation"], "evaluated");
    assert_eq!(unit["report"]["claim"]["stage"], "unstated");
    assert_eq!(
        unit["report"]["artifact_binding"]["state"], "not_stated_by_sbom",
        "a document that does not hash its subject cannot be bound to an artifact"
    );
    let layers = from_cli["units"][0]["layers"].as_array().expect("layers");
    let distributed_layer = layers
        .iter()
        .find(|layer| layer["layer"] == "distributed")
        .expect("distributed layer");
    assert_eq!(distributed_layer["status"], "partial");
}

#[tokio::test]
async fn a_pinned_digest_that_does_not_match_refuses_the_sbom_on_both_surfaces() {
    let repo = repository();
    std::fs::write(repo.path().join(SBOM), sbom_bytes()).expect("sbom");

    let from_cli = cli_answer(repo.path(), &sbom_question(&"f".repeat(64)));
    let from_mcp = mcp_answer(repo.path(), &sbom_question(&"f".repeat(64))).await;

    assert_eq!(from_cli, from_mcp);
    assert_eq!(from_cli["distributed"]["status"], "digest_mismatch");
    assert_eq!(
        from_cli["distributed"]["units"][0]["evaluation"],
        "not_evaluated"
    );
    let layers = from_cli["units"][0]["layers"].as_array().expect("layers");
    assert!(
        layers
            .iter()
            .any(|layer| layer["layer"] == "distributed" && layer["status"] == "not_collected")
    );
}

#[tokio::test]
async fn asking_with_an_sbom_leaves_the_workspace_as_it_was() {
    let repo = repository();
    std::fs::write(repo.path().join(SBOM), sbom_bytes()).expect("sbom");
    let tree_before = tree(repo.path());

    cli_answer(repo.path(), &json!({ "sbom": SBOM }));
    mcp_answer(repo.path(), &json!({ "sbom": SBOM })).await;

    assert_eq!(tree(repo.path()), tree_before);
}
