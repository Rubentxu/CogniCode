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
//!
//! The Kotlin/Java corpus is the second case, and it measures something
//! the Terraform case cannot: the Terraform fixture only loses a
//! reference that nobody declares, so a COUNT of pending references is
//! all the verdict it needs. The Kotlin corpus also loses references
//! that are AMBIGUOUS — the name is declared several times and the
//! graph cannot prove which one is meant — so the negative has an
//! identity (which call, in which file and line) and a motive (how
//! many candidates carry the name). A consumer that reads the graph
//! has to be able to SEE that, and the two surfaces have to see the
//! SAME thing: they are adapters over one service, and a service that
//! answers two ways is two services.

mod common;

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::interface::coverage::{PendingReason, PendingReferenceInfo};
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

// ─── The Kotlin/Java corpus ───────────────────────────────────────────
//
// The corpus that M2.T03 (TASK-13) built, and the one where the
// negative has an identity and a motive: `run` is declared three times
// and `invoke` twice, and neither call can be tied to one of them.
//
// The measurement above counts the pending references. A count says
// THAT something was lost, not WHICH call, not WHERE it is written and
// not WHY it could not be tied — and those are exactly the three things
// a consumer needs in order to act on the loss. So here the two
// surfaces are compared on the CONTENT of the negative, field by field.
//
// The corpus is already in the repo and the Kotlin contract already
// measures the loss inside the domain; this adds no fixture, no
// contract and no test file — it reuses both, and the file it lives in
// already knew how to run the CLI and the MCP over the same tree.

/// The Kotlin/Java corpus of the repo: twelve files, two languages.
///
/// It is copied to a directory of its own because the two surfaces have
/// to read EXACTLY the same tree, and because the tree in the repo is
/// not this test's to write to.
fn kotlin_corpus_project() -> PathBuf {
    let source = common::repo_root().join("sandbox/fixtures/kotlin-java");
    let target = std::env::temp_dir().join(format!(
        "cognicode-cli-kotlin-parity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    copy_tree(&source, &target);
    target
}

/// A negative, reduced to what the two surfaces have to say ALike: its
/// identity and its motive.
///
/// The file is made relative to the project: what it says about the
/// code is the relative part, and the rest is where the corpus was
/// copied to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Negative {
    file: String,
    line: Option<u32>,
    target: String,
    kind: String,
    reason: String,
    candidates: Option<usize>,
}

/// A file's path, relative to the project it belongs to.
fn relative_to(project: &Path, file: &str) -> String {
    Path::new(file)
        .strip_prefix(project)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| file.to_string())
}

/// The negatives the CLI publishes, as the same facts the MCP publishes.
fn cli_negatives(answer: &Value, project: &Path) -> Vec<Negative> {
    let mut negatives: Vec<Negative> = answer["pending_references"]
        .as_array()
        .expect("the CLI answers with the references it could not tie")
        .iter()
        .map(|reference| Negative {
            file: relative_to(project, reference["file"].as_str().unwrap_or_default()),
            line: reference["line"].as_u64().map(|line| line as u32),
            target: reference["target"].as_str().unwrap_or_default().to_string(),
            kind: reference["kind"].as_str().unwrap_or_default().to_string(),
            reason: reference["reason"].as_str().unwrap_or_default().to_string(),
            candidates: reference["candidates"].as_u64().map(|count| count as usize),
        })
        .collect();
    // The order of the graph is not the order of the answer, and it does
    // not have to be: what the two surfaces have to share is the SET of
    // negatives, not the order they arrive in.
    negatives.sort();
    negatives
}

/// The negatives the MCP publishes, as the same facts the CLI publishes.
fn mcp_negatives(references: &[PendingReferenceInfo], project: &Path) -> Vec<Negative> {
    let mut negatives: Vec<Negative> = references
        .iter()
        .map(|reference| Negative {
            file: relative_to(project, &reference.file),
            line: reference.line,
            target: reference.target.clone(),
            kind: reference.kind.clone(),
            reason: match reference.reason {
                PendingReason::Unresolved => "unresolved".to_string(),
                PendingReason::Ambiguous => "ambiguous".to_string(),
            },
            candidates: reference.candidates,
        })
        .collect();
    negatives.sort();
    negatives
}

/// The verdict of the MCP over one project: its status, the negatives it
/// publishes and the graph it published.
struct McpCoverage {
    status: String,
    negatives: Vec<Negative>,
    symbols: u64,
    relations: u64,
}

/// Asks the MCP for the graph of one project, and keeps what it answered.
async fn mcp_coverage(dir: &Path) -> McpCoverage {
    let ctx = HandlerContext::builder().with_working_dir(dir).build();
    let answer = handle_build_graph(
        &ctx,
        BuildGraphInput {
            directory: Some(dir.to_string_lossy().into_owned()),
        },
    )
    .await
    .expect("the MCP builds the graph");
    McpCoverage {
        status: answer.status,
        negatives: mcp_negatives(&answer.pending_references.unwrap_or_default(), dir),
        symbols: answer.symbols_found as u64,
        relations: answer.relationships_found as u64,
    }
}

#[tokio::test]
async fn the_cli_and_the_mcp_publish_the_same_negative_on_the_kotlin_corpus() {
    let project = kotlin_corpus_project();

    let answer = run_cli(&["graph", "full", &project.to_string_lossy()]);
    let cli_status = answer["status"]
        .as_str()
        .expect("the CLI answers with a status")
        .to_string();
    let cli_symbols = answer["symbols"]
        .as_u64()
        .expect("the CLI answers with a count of symbols");
    let cli_relations = answer["dependencies"]
        .as_u64()
        .expect("the CLI answers with a count of relations");
    let from_cli = cli_negatives(&answer, &project);

    let from_mcp = mcp_coverage(&project).await;

    // The negative has to EXIST. Two surfaces that publish nothing are
    // equal in the void, and an empty answer is not an equivalence: it
    // is the shape a product takes when it loses the loss.
    assert!(
        !from_cli.is_empty(),
        "the CLI publishes the negative the corpus declares: got {from_cli:?}"
    );
    assert!(
        !from_mcp.negatives.is_empty(),
        "the MCP publishes the negative the corpus declares: got {:?}",
        from_mcp.negatives
    );

    // And it has to be THE negative the corpus declares, with its
    // identity —which call, in which file and line— and its motive —
    // how many candidates carry the name—. The corpus declares `run`
    // three times and `invoke` twice, and neither call can be tied to
    // one of them.
    let expected = vec![
        Negative {
            file: "demo/gamma/Caller.kt".to_string(),
            line: Some(3),
            target: "run".to_string(),
            kind: "calls".to_string(),
            reason: "ambiguous".to_string(),
            candidates: Some(3),
        },
        Negative {
            file: "demo/gamma/UseHandler.kt".to_string(),
            line: Some(6),
            target: "invoke".to_string(),
            kind: "calls".to_string(),
            reason: "ambiguous".to_string(),
            candidates: Some(2),
        },
    ];
    assert_eq!(
        from_cli, expected,
        "the CLI publishes the negative with its identity and its motive: \
         which call, in which file and line, and how many candidates carry \
         the name"
    );
    assert_eq!(
        from_mcp.negatives, expected,
        "the MCP publishes the negative with its identity and its motive: \
         which call, in which file and line, and how many candidates carry \
         the name"
    );

    // The two surfaces are adapters over one service: they have to say
    // the same thing about the same project.
    assert_eq!(
        from_cli, from_mcp.negatives,
        "the CLI and the MCP publish a DIFFERENT negative for the same project"
    );
    assert_eq!(
        cli_status, from_mcp.status,
        "the CLI and the MCP answer with different verdicts about the same project"
    );
    assert_eq!(
        cli_status, "partial",
        "a call the graph cannot tie is a hole, and both surfaces say so"
    );

    // And the graph they published is the same one, and it is not empty:
    // an equivalence measured over an empty graph is an equivalence
    // measured over nothing.
    assert!(
        cli_symbols > 0 && cli_relations > 0,
        "the corpus publishes a graph, so the equivalence is measured over \
         something and not over nothing: got {cli_symbols} symbols and \
         {cli_relations} relations"
    );
    assert_eq!(
        cli_symbols, from_mcp.symbols,
        "the CLI and the MCP publish a different number of symbols for the same project"
    );
    assert_eq!(
        cli_relations, from_mcp.relations,
        "the CLI and the MCP publish a different number of relations for the same project"
    );
    assert!(
        from_mcp.symbols > 0 && from_mcp.relations > 0,
        "the MCP publishes the same non-empty graph the CLI publishes"
    );
}
