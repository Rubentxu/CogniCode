//! The `security` goal over the real MCP wire.
//!
//! What a client sees: the tool is listed with its schema, a chart without the
//! `helm` engine is `not_rendered` with every dependent verdict `unknown`, and
//! with an engine it is rendered and reports its finding. The engine in the
//! rendered case is a labelled test double (a script that prints a fixed
//! `helm template` stream), not the real binary.

use crate::common::McpSession;
use crate::fixtures;
use serde_json::{Value, json};
use std::path::Path;

async fn call(workspace: &Path, path: &Path, arguments: Value) -> Value {
    let cache = tempfile::tempdir().expect("cache");
    let mut session = McpSession::spawn_with_env(
        workspace,
        &[("PATH", path), ("XDG_CACHE_HOME", cache.path())],
    )
    .await
    .expect("spawn");
    let response = session
        .request(
            "tools/call",
            json!({"name": "security", "arguments": arguments}),
        )
        .await
        .expect("tools/call");
    session.shutdown().await;
    response
}

fn answer(response: &Value) -> Value {
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

fn verdict(report: &Value, unit_path: &str, family: &str) -> String {
    let unit = report["units"]
        .as_array()
        .and_then(|units| units.iter().find(|u| u["path"] == unit_path))
        .unwrap_or_else(|| panic!("no unit {unit_path} in {report}"));
    unit["families"]
        .as_array()
        .and_then(|fs| fs.iter().find(|f| f["family"] == family))
        .and_then(|f| f["verdict"].as_str())
        .unwrap_or_else(|| panic!("no verdict {family} in {unit}"))
        .to_string()
}

#[tokio::test]
async fn the_tool_is_listed_with_its_own_arguments() {
    let scratch = tempfile::tempdir().expect("scratch");
    let mut session = McpSession::spawn_with_flags(scratch.path(), &["--profile", "full"])
        .await
        .expect("spawn");
    let tools = session.list_tools().await.expect("tools/list");
    session.shutdown().await;
    assert!(tools.iter().any(|name| name == "security"), "{tools:?}");
}

#[tokio::test]
async fn without_the_engine_a_chart_is_not_rendered_and_its_checks_are_unknown() {
    let scratch = tempfile::tempdir().expect("scratch");
    let workspace = fixtures::security_pack_workspace(scratch.path());
    let no_engine = scratch.path().join("empty-path");
    std::fs::create_dir_all(&no_engine).expect("empty directory");

    let report = answer(&call(&workspace, &no_engine, json!({})).await);

    let chart = &report["helm"][0];
    assert_eq!(chart["render"]["status"], "not_rendered", "{chart}");
    assert_eq!(chart["coverage"]["state"], "unknown", "{chart}");
    assert_eq!(
        verdict(chart, "demo/Chart.yaml", "infra_permissions"),
        "unknown"
    );
    assert_eq!(report["totals"]["found"], 0, "{report}");
    assert!(
        report["limits"]
            .as_array()
            .is_some_and(|limits| !limits.is_empty()),
        "{report}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn with_an_engine_the_chart_is_rendered_and_the_finding_is_reported() {
    let scratch = tempfile::tempdir().expect("scratch");
    let workspace = fixtures::security_pack_workspace(scratch.path());
    let engine = fixtures::fake_helm_directory(scratch.path());

    let report = answer(&call(&workspace, &engine, json!({})).await);

    let chart = &report["helm"][0];
    assert_eq!(chart["render"]["status"], "rendered", "{chart}");
    assert_eq!(
        verdict(chart, "demo/templates/deployment.yaml", "infra_permissions"),
        "found"
    );
    // The Terraform root says what its variable left open.
    let terraform = &report["terraform"][0];
    assert_eq!(terraform["coverage"]["complete"], false, "{terraform}");
    assert_eq!(
        verdict(terraform, "main.tf", "infra_public_exposure"),
        "unknown"
    );
}

#[tokio::test]
async fn arguments_that_are_not_the_goals_own_are_a_failure_not_an_empty_report() {
    let scratch = tempfile::tempdir().expect("scratch");
    let workspace = fixtures::security_pack_workspace(scratch.path());

    let response = call(&workspace, scratch.path(), json!({"all_languages": "yes"})).await;

    assert_eq!(
        response["result"]["isError"].as_bool(),
        Some(true),
        "{response}"
    );
}
