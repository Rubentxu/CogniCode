//! The `dependencies` goal over the real MCP wire.
//!
//! What a client sees: the tool is listed, a call inventories the Cargo
//! lockfile of `HEAD` and answers with packages, identity, freshness and the
//! layers that were not collected, and a stale lockfile is reported as such.

use crate::common::McpSession;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
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
}

const MANIFEST: &str =
    "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n";

fn lock() -> String {
    format!(
        "version = 3\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\"serde\"]\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.1\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n",
        "a".repeat(64)
    )
}

fn repository(manifest: &str) -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("repository");
    let root = directory.path();
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "CogniCode tests"]);
    git(root, &["config", "user.email", "tests@example.invalid"]);
    std::fs::write(root.join("Cargo.toml"), manifest).expect("manifest");
    std::fs::write(root.join("Cargo.lock"), lock()).expect("lock");
    git(root, &["add", "-A"]);
    git(root, &["commit", "--quiet", "-m", "base"]);
    directory
}

async fn call(workspace: &Path, arguments: Value) -> Value {
    let cache = tempfile::tempdir().expect("cache");
    let mut session = McpSession::spawn_with_env(workspace, &[("XDG_CACHE_HOME", cache.path())])
        .await
        .expect("spawn");
    let response = session
        .request(
            "tools/call",
            json!({"name": "dependencies", "arguments": arguments}),
        )
        .await
        .expect("tools/call");
    session.shutdown().await;
    assert_eq!(
        response["result"]["isError"].as_bool(),
        Some(false),
        "the call is not a failure: {response}"
    );
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    serde_json::from_str(text).expect("the answer is JSON")
}

#[tokio::test]
async fn the_tool_is_listed_in_the_full_surface() {
    let scratch = tempfile::tempdir().expect("scratch");
    let mut session = McpSession::spawn_with_flags(scratch.path(), &["--profile", "full"])
        .await
        .expect("spawn");
    let tools = session.list_tools().await.expect("tools/list");
    session.shutdown().await;
    assert!(tools.iter().any(|name| name == "dependencies"), "{tools:?}");
}

#[tokio::test]
async fn a_call_inventories_the_lockfile_of_head_and_says_what_it_did_not_collect() {
    let repo = repository(MANIFEST);

    let answer = call(repo.path(), json!({})).await;

    assert_eq!(answer["goal"], "dependencies");
    assert_eq!(answer["revision"]["requested_ref"], "HEAD");
    let unit = &answer["units"][0];
    assert_eq!(unit["freshness"], "consistent");
    assert_eq!(unit["packages"][1]["purl"], "pkg:cargo/serde@1.0.1");
    assert_eq!(unit["packages"][1]["integrity"], "checksummed");
    let layers = unit["layers"].as_array().expect("layers");
    let status = |layer: &str| {
        layers
            .iter()
            .find(|entry| entry["layer"] == layer)
            .map(|entry| entry["status"].clone())
    };
    assert_eq!(status("resolved"), Some(json!("collected")));
    assert_eq!(status("observed"), Some(json!("not_collected")));
    assert_eq!(status("distributed"), Some(json!("not_collected")));
}

#[tokio::test]
async fn a_lockfile_that_misses_a_declaration_is_reported_stale() {
    let repo = repository(&format!("{MANIFEST}itoa = \"1\"\n"));

    let answer = call(repo.path(), json!({})).await;

    assert_eq!(answer["units"][0]["freshness"], "stale");
    assert_eq!(
        answer["units"][0]["discrepancies"][0]["kind"],
        "declared_not_locked"
    );
}

fn sbom() -> String {
    json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "lifecycles": [{"phase": "build"}],
            "component": {"type": "application", "bom-ref": "app", "name": "app", "version": "0.1.0"},
        },
        "components": [{
            "type": "library",
            "bom-ref": "serde",
            "name": "serde",
            "version": "1.0.1",
            "purl": "pkg:cargo/serde@1.0.1",
            "scope": "required",
            "hashes": [{"alg": "SHA-256", "content": "a".repeat(64)}],
        }],
    })
    .to_string()
}

#[tokio::test]
async fn an_sbom_in_the_workspace_is_compared_over_the_wire() {
    let repo = repository(MANIFEST);
    std::fs::write(repo.path().join("app.cdx.json"), sbom()).expect("sbom");

    let answer = call(repo.path(), json!({"sbom": "app.cdx.json"})).await;

    let distributed = &answer["distributed"];
    assert_eq!(distributed["status"], "usable");
    assert_eq!(distributed["document"]["origin"], "app.cdx.json");
    assert_eq!(distributed["units"][0]["evaluation"], "evaluated");
    assert_eq!(
        distributed["units"][0]["report"]["claim"]["stage"],
        "artifact"
    );
    assert_eq!(
        distributed["units"][0]["report"]["findings"],
        json!([]),
        "the SBOM and the lockfile agree"
    );
    let layers = answer["units"][0]["layers"].as_array().expect("layers");
    assert!(
        layers
            .iter()
            .any(|entry| entry["layer"] == "distributed" && entry["status"] == "partial")
    );
}

#[tokio::test]
async fn an_sbom_that_cannot_be_read_never_reads_as_complete_over_the_wire() {
    let repo = repository(MANIFEST);

    let answer = call(repo.path(), json!({"sbom": "missing.cdx.json"})).await;

    assert_eq!(answer["distributed"]["status"], "unreadable");
    let layers = answer["units"][0]["layers"].as_array().expect("layers");
    assert!(
        layers
            .iter()
            .any(|entry| entry["layer"] == "distributed" && entry["status"] == "not_collected")
    );
}

#[tokio::test]
async fn the_published_arguments_name_the_sbom_inputs() {
    let scratch = tempfile::tempdir().expect("scratch");
    let mut session = McpSession::spawn_with_flags(scratch.path(), &["--profile", "full"])
        .await
        .expect("spawn");
    let response = session
        .request("tools/list", json!({}))
        .await
        .expect("tools/list");
    session.shutdown().await;
    let tools = response["result"]["tools"].as_array().expect("tools");
    let tool = tools
        .iter()
        .find(|tool| tool["name"] == "dependencies")
        .expect("dependencies");
    let properties = &tool["inputSchema"]["properties"];
    for argument in [
        "sbom",
        "sbom_expected_digest",
        "artifact_digest",
        "sbom_member",
    ] {
        assert!(
            properties.get(argument).is_some(),
            "{argument}: {properties}"
        );
    }
}
