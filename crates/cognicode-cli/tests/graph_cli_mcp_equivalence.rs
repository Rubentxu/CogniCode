//! The CLI and the MCP are adapters over one service, so a command that
//! answers with a graph has to answer with the graph the MCP hands out.
//!
//! The Terraform fixture is the sharp case. The domain reader names a
//! resource the way the file declares it; the generic reader does not
//! know what a Terraform resource is. A command that builds its own
//! graph answers with the generic one, and the two surfaces disagree
//! about the same project — which is what this test measures.
//!
//! The CLI side is the real binary, run as a sub-process: the contract
//! is about what a user gets, not about an internal function.

mod common;

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

/// The project both surfaces are compared on.
///
/// Its own directory, named uniquely: a directory named after the process
/// alone is shared by every test of this file, and the tests run in
/// parallel —so one test's copy would land on top of another's, and both
/// would measure whichever write happened last.
fn terraform_project() -> PathBuf {
    let source = common::repo_root().join("sandbox/fixtures/terraform-iac");
    let target = std::env::temp_dir().join(format!(
        "cognicode-cli-graph-parity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    copy_tree(&source, &target);
    target
}

fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).expect("create the target directory");
    for entry in std::fs::read_dir(source).expect("read the fixture") {
        let entry = entry.expect("a fixture entry");
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &destination);
        } else {
            std::fs::copy(&path, &destination).expect("copy a fixture file");
        }
    }
}

/// Runs the CLI as a sub-process and returns its JSON answer.
fn run_cli(args: &[&str]) -> Value {
    let binary = common::binary_path("cognicode");
    let output = Command::new(&binary)
        .arg("--format")
        .arg("json")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", binary.display()));
    assert!(
        output.status.success(),
        "the CLI exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "the CLI did not answer with JSON: {e}: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

/// The graph the MCP hands out for the same project.
async fn mcp_graph(dir: &Path) -> Arc<CallGraph> {
    let ctx = HandlerContext::builder().with_working_dir(dir).build();
    handle_build_graph(
        &ctx,
        BuildGraphInput {
            directory: Some(dir.to_string_lossy().into_owned()),
        },
    )
    .await
    .expect("the MCP builds the graph");
    ctx.analysis_service().get_project_graph()
}

/// The leaves of a graph, as the CLI reports them: name, file, line.
fn leaves(graph: &CallGraph) -> Vec<(String, String, u64)> {
    graph
        .leaves()
        .iter()
        .take(20)
        .filter_map(|id| {
            let symbol = graph.get_symbol(id)?;
            Some((
                symbol.name().to_string(),
                symbol.location().file().to_string(),
                symbol.location().line() as u64,
            ))
        })
        .collect()
}

/// The leaves the CLI reported, as the same triples.
fn cli_leaves(answer: &Value) -> Vec<(String, String, u64)> {
    answer["symbols"]
        .as_array()
        .expect("the CLI answers with a list of symbols")
        .iter()
        .map(|symbol| {
            (
                symbol["name"].as_str().unwrap_or_default().to_string(),
                symbol["file"].as_str().unwrap_or_default().to_string(),
                symbol["line"].as_u64().unwrap_or_default(),
            )
        })
        .collect()
}

#[tokio::test]
async fn the_cli_and_the_mcp_answer_with_the_same_leaves() {
    let project = terraform_project();
    let answer = run_cli(&["graph", "leaf-functions", &project.to_string_lossy()]);
    let from_cli = cli_leaves(&answer);

    let graph = mcp_graph(&project).await;
    let from_mcp = leaves(&graph);

    assert!(
        !from_mcp.is_empty(),
        "the graph has leaves: the fixture declares symbols"
    );
    assert_eq!(
        from_cli, from_mcp,
        "the CLI and the MCP answered with different leaves for the same project"
    );
}

#[tokio::test]
async fn the_cli_names_the_terraform_resource_the_way_the_file_declares_it() {
    let project = terraform_project();
    let answer = run_cli(&["graph", "leaf-functions", &project.to_string_lossy()]);
    let names: Vec<String> = answer["symbols"]
        .as_array()
        .expect("the CLI answers with a list of symbols")
        .iter()
        .map(|symbol| symbol["name"].as_str().unwrap_or_default().to_string())
        .collect();

    // The names are the ones the file declares: a variable is `var.name`,
    // a resource is `aws_s3_bucket.name`, a data source is
    // `data.aws_ami.name`. A reader that only knows the categories would
    // answer with the category alone. Which declarations end up as leaves
    // is the graph's business — it changes as the graph learns more — and
    // how they are named is the reader's.
    assert!(
        !names.is_empty(),
        "the project declares symbols, so the CLI has leaves to publish: \
         got: {names:?}"
    );
    assert!(
        names.iter().all(|name| name.contains('.')),
        "the CLI names the declarations the way the file declares them, as \
         the MCP does. got: {names:?}"
    );

    // The exact name the file declares, on the declarations themselves:
    // which of them end up as leaves is the graph's business, but that the
    // resource is called `aws_s3_bucket.app_bucket` and not `app_bucket`
    // is the reader's.
    let project = terraform_project();
    let graph = mcp_graph(&project).await;
    let declarations: Vec<String> = graph.symbols().map(|s| s.name().to_string()).collect();
    assert!(
        declarations.contains(&"aws_s3_bucket.app_bucket".to_string()),
        "the graph declares the resource the way the file declares it. \
         got: {declarations:?}"
    );
}

/// A project with a reference that nobody satisfies, so the two surfaces
/// have something to disagree about.
///
/// Its own directory: the fixture copy that the other tests use is wiped
/// and rebuilt by whoever calls it, and two tests sharing one tree would
/// measure each other's writes.
fn project_with_a_dangling_reference() -> PathBuf {
    let source = common::repo_root().join("sandbox/fixtures/terraform-iac");
    let project = std::env::temp_dir().join(format!(
        "cognicode-cli-graph-coverage-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    copy_tree(&source, &project);
    let main = project.join("main.tf");
    let mut source = std::fs::read_to_string(&main).expect("read the fixture");
    source.push_str(
        "\nresource \"aws_s3_bucket_policy\" \"ghost_policy\" {\n  bucket = aws_s3_bucket.no_existe.id\n  policy = \"{}\"\n}\n",
    );
    std::fs::write(&main, &source).expect("write the fixture");
    project
}

/// The verdict of the MCP for the same project, as the two words both
/// surfaces use.
async fn mcp_verdict(dir: &Path) -> (String, usize) {
    let ctx = HandlerContext::builder().with_working_dir(dir).build();
    let answer = handle_build_graph(
        &ctx,
        BuildGraphInput {
            directory: Some(dir.to_string_lossy().into_owned()),
        },
    )
    .await
    .expect("the MCP builds the graph");
    let pending = answer.pending_references.unwrap_or_default();
    (answer.status, pending.len())
}

#[tokio::test]
async fn the_cli_and_the_mcp_say_the_same_thing_about_the_coverage() {
    let project = project_with_a_dangling_reference();

    // The same tree, the same question, two surfaces.
    let from_cli = run_cli(&["graph", "full", &project.to_string_lossy()]);
    let cli_status = from_cli["status"]
        .as_str()
        .expect("the CLI answers with a status")
        .to_string();
    let cli_pending = from_cli["pending_references"]
        .as_array()
        .map(|references| references.len())
        .unwrap_or(0);

    let (mcp_status, mcp_pending) = mcp_verdict(&project).await;

    assert_eq!(
        cli_status, mcp_status,
        "the CLI and the MCP answer with different verdicts about the same \\
         project: the CLI says {cli_status}, the MCP says {mcp_status}"
    );
    assert_eq!(
        cli_pending, mcp_pending,
        "the CLI and the MCP count the pending references differently"
    );
    assert_eq!(
        cli_status, "partial",
        "a reference that nobody satisfies is a hole, and both surfaces say so"
    );
    assert!(
        mcp_pending > 0,
        "and both surfaces publish the reference that is pending"
    );
}
