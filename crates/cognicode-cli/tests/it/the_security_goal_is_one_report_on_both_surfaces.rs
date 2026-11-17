//! The `security` goal on the CLI and on the MCP.
//!
//! The goal is declared once and both surfaces run the same function, so what
//! this file measures is what a user gets: the real `cognicode` binary run as
//! a sub-process, and the MCP dispatch fed the same workspace.
//!
//! * The two surfaces answer with the same document.
//! * Without the `helm` engine a chart is `not_rendered` and every verdict
//!   that needs the render is `unknown`: never clean.
//! * With an engine the chart is `rendered` and the finding appears.
//! * The text form names the four verdicts by their own words and carries the
//!   capability matrix with its limits.
//!
//! The engine in the rendered cases is a labelled test double (a script that
//! prints a fixed `helm template` stream), not the real binary.

use crate::common;
use crate::fixtures;

use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_mcp::rmcp_adapter::call_tool_handler_with_window;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output};

/// Runs the real CLI in `workspace` with `path` as its `PATH`.
fn cli(workspace: &Path, path: &Path, args: &[&str]) -> Output {
    let cache = common::isolated_cache();
    Command::new(common::binary_path("cognicode"))
        .current_dir(workspace)
        .args(args)
        .env("PATH", path)
        .env("XDG_CACHE_HOME", cache.path())
        .output()
        .expect("the CLI starts")
}

fn cli_json(workspace: &Path, path: &Path) -> Value {
    let output = cli(workspace, path, &["security", "--format", "json"]);
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

fn unit<'a>(chart: &'a Value, path: &str) -> &'a Value {
    chart["units"]
        .as_array()
        .and_then(|units| units.iter().find(|unit| unit["path"] == path))
        .unwrap_or_else(|| panic!("no unit {path} in {chart}"))
}

fn verdict(unit: &Value, family: &str) -> String {
    unit["families"]
        .as_array()
        .and_then(|families| families.iter().find(|f| f["family"] == family))
        .and_then(|f| f["verdict"].as_str())
        .unwrap_or_else(|| panic!("no verdict for {family} in {unit}"))
        .to_string()
}

#[tokio::test]
async fn the_cli_and_the_mcp_answer_with_the_same_report() {
    let scratch = tempfile::tempdir().expect("scratch");
    let workspace = fixtures::security_pack_workspace(scratch.path());
    let path = std::env::var_os("PATH").unwrap_or_default();

    let from_cli = cli_json(&workspace, Path::new(&path));

    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.clone())
        .build();
    let request = CallToolRequestParams::new("security".to_string())
        .with_arguments(json!({}).as_object().cloned().unwrap_or_default());
    let text = call_tool_handler_with_window(&ctx, request, None)
        .await
        .expect("the MCP answers");
    let from_mcp: Value = serde_json::from_str(&text).expect("the MCP answers with JSON");

    assert_eq!(from_cli, from_mcp, "the two surfaces publish one report");
    assert_eq!(from_cli["goal"], "security");
    assert_eq!(from_cli["terraform"].as_array().map(Vec::len), Some(1));
    assert_eq!(from_cli["helm"].as_array().map(Vec::len), Some(1));
}

#[test]
fn without_the_engine_a_chart_is_not_rendered_and_never_clean() {
    let scratch = tempfile::tempdir().expect("scratch");
    let workspace = fixtures::security_pack_workspace(scratch.path());
    let no_engine = scratch.path().join("empty-path");
    std::fs::create_dir_all(&no_engine).expect("empty directory");

    let report = cli_json(&workspace, &no_engine);

    let chart = &report["helm"][0];
    assert_eq!(chart["render"]["status"], "not_rendered", "{chart}");
    assert_eq!(chart["coverage"]["state"], "unknown", "{chart}");
    let template = unit(chart, "demo/Chart.yaml");
    for family in [
        "infra_public_exposure",
        "infra_permissions",
        "infra_workload_hardening",
    ] {
        assert_eq!(verdict(template, family), "unknown", "{family}");
    }
    // The Terraform unit that a variable leaves open is unknown too.
    let terraform = unit(&report["terraform"][0], "main.tf");
    assert_eq!(verdict(terraform, "infra_public_exposure"), "unknown");
    assert!(report["totals"]["unknown"].as_u64().unwrap() >= 4);
}

#[cfg(unix)]
#[test]
fn with_an_engine_the_chart_is_rendered_and_the_finding_is_reported() {
    let scratch = tempfile::tempdir().expect("scratch");
    let workspace = fixtures::security_pack_workspace(scratch.path());
    let engine = fixtures::fake_helm_directory(scratch.path());

    let report = cli_json(&workspace, &engine);

    let chart = &report["helm"][0];
    assert_eq!(chart["render"]["status"], "rendered", "{chart}");
    assert_eq!(chart["render"]["documents"], 1, "{chart}");
    let template = unit(chart, "demo/templates/deployment.yaml");
    assert_eq!(
        verdict(template, "infra_permissions"),
        "found",
        "{template}"
    );
    assert_eq!(
        verdict(template, "infra_encryption"),
        "unsupported",
        "{template}"
    );
}

#[cfg(unix)]
#[test]
fn the_text_names_each_verdict_and_publishes_the_matrix_with_its_limits() {
    let scratch = tempfile::tempdir().expect("scratch");
    let workspace = fixtures::security_pack_workspace(scratch.path());
    let engine = fixtures::fake_helm_directory(scratch.path());

    let output = cli(&workspace, &engine, &["security"]);
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);

    for word in ["FOUND", "UNKNOWN", "UNSUPPORTED", "NOT OBSERVED"] {
        assert!(text.contains(word), "the text names {word}: {text}");
    }
    assert!(text.contains("rendered (1 document(s))"), "{text}");
    assert!(text.contains("capability matrix"), "{text}");
    assert!(
        text.contains("limit:"),
        "the matrix carries its limits: {text}"
    );
    assert!(text.contains("not a gate"), "{text}");
}

#[test]
fn the_matrix_widens_to_every_language_on_request() {
    let scratch = tempfile::tempdir().expect("scratch");
    let workspace = fixtures::security_pack_workspace(scratch.path());
    let empty = scratch.path().join("empty-path");
    std::fs::create_dir_all(&empty).expect("empty directory");

    let narrow = cli_json(&workspace, &empty);
    let output = cli(
        &workspace,
        &empty,
        &["security", "--all-languages", "--format", "json"],
    );
    assert!(output.status.success());
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("JSON");
    let wide = envelope["result"].clone();

    let rows = |report: &Value| report["matrix"].as_array().map(Vec::len).unwrap();
    assert!(
        rows(&wide) > rows(&narrow),
        "{} rows, then {}",
        rows(&narrow),
        rows(&wide)
    );
}
