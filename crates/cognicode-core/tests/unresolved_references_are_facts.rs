//! A reference the graph could not tie to a declaration is a fact.
//!
//! A graph can read every file of a project and still not know what depends
//! on what: a block that names a resource nobody declares is a reference the
//! graph could not resolve. Dropping it says nothing at all; keeping it says
//! exactly what is missing and where it is missing it.
//!
//! The consumer asks one question — "do you know everything?" — and there
//! are two answers to give: which files were read, and which references were
//! tied to a declaration. Both travel with the answer, and "complete" is
//! only said when both are whole.

mod fixtures;

use cognicode_core::application::analysis_basis::{
    AnalysisBasis, AnalysisOp, Analyze, OutcomeState,
};
use cognicode_core::application::services::analyze::ServiceAnalyzer;
use cognicode_core::interface::cli::presenter_bridge::present_graph_full;
use cognicode_core::interface::cli::result::GraphResult;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};
use cognicode_core::interface::mcp::schemas::PendingReason;

/// The line the reference is written on, counted from one.
fn line_of(source: &str, needle: &str) -> u32 {
    source
        .lines()
        .position(|line| line.contains(needle))
        .map(|index| index as u32 + 1)
        .expect("the reference is in the file the test wrote")
}

/// The reference nobody declares: a bucket that does not exist.
const GHOST: &str = "aws_s3_bucket.no_existe.id";

#[tokio::test]
async fn a_reference_to_a_declaration_that_does_not_exist_is_declared_pending() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = fixtures::copy_fixture(dir.path(), "terraform-iac");
    let main = fixture.join("main.tf");

    let mut source = std::fs::read_to_string(&main).expect("the fixture is readable");
    source.push_str(
        "\nresource \"aws_s3_bucket_policy\" \"ghost_policy\" {\n  bucket = aws_s3_bucket.no_existe.id\n  policy = \"{}\"\n}\n",
    );
    std::fs::write(&main, &source).expect("the fixture is writable");

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();

    let answer = handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");

    assert_ne!(
        answer.status, "complete",
        "a reference that was not tied to a declaration is not a complete answer"
    );

    let pending = answer
        .pending_references
        .expect("the pending references travel with the answer");

    let ghost = pending
        .iter()
        .find(|reference| reference.target == GHOST)
        .expect("the reference that names nothing is declared, not dropped");

    assert_eq!(
        ghost.file,
        main.display().to_string(),
        "the origin is the file that writes the reference"
    );
    assert_eq!(
        ghost.line,
        Some(line_of(&source, GHOST)),
        "the origin is the line the reference is written on"
    );
    assert_eq!(
        ghost.reason,
        PendingReason::Unresolved,
        "no declaration of the project carries that name"
    );
}

#[tokio::test]
async fn a_reference_that_another_file_declares_is_a_relation_and_not_a_gap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = fixtures::copy_fixture(dir.path(), "terraform-iac");
    let variables = fixture.join("variables.tf");

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();

    let answer = handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");

    let pending = answer
        .pending_references
        .expect("the pending references travel with the answer");

    assert!(
        !pending
            .iter()
            .any(|reference| reference.target == "var.vpc_cidr"),
        "the variable is declared in another file of the same project: the \
         reference is resolved between files, it is not a gap"
    );

    // The relation is not merely absent from the gaps: it is in the graph,
    // pointing at the declaration of the other file.
    let graph = ctx.analysis_service().get_project_graph();
    let relation = "var.vpc_cidr";
    assert!(
        graph
            .all_dependencies()
            .any(|(_, target, _)| target.as_str().ends_with(relation)
                && target.as_str().contains(&variables.display().to_string())),
        "the reference that another file declares became a relation of the graph"
    );
}

#[tokio::test]
async fn a_project_without_dangling_references_says_it_knows_everything() {
    let dir = tempfile::tempdir().expect("tempdir");
    fixtures::copy_fixture(dir.path(), "terraform-iac");

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();

    let answer = handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");

    // The fixture declares everything it mentions, so the graph has nothing
    // to report as missing. A name the language binds itself — the index of
    // an instance, the name a `for` expression binds — names no
    // declaration, so it is not a gap either.
    let pending = answer
        .pending_references
        .expect("the pending references travel with the answer");
    assert!(
        pending.is_empty(),
        "the project declares everything it mentions, and the answer says \
         otherwise: {pending:?}"
    );
    assert_eq!(
        answer.status, "complete",
        "a project whose references are all tied is a complete answer"
    );
}

#[tokio::test]
async fn a_module_of_the_language_is_not_a_gap() {
    let dir = tempfile::tempdir().expect("tempdir");
    fixtures::copy_fixture(dir.path(), "ansible-playbook");

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();

    let answer = handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");

    let pending = answer
        .pending_references
        .expect("the pending references travel with the answer");

    // The modules a task calls live in Ansible, not in the project: the
    // graph says nothing about the world outside it, so they are not gaps.
    assert!(
        !pending
            .iter()
            .any(|reference| reference.target.starts_with("ansible:builtin:")),
        "a module of the language is not a reference to a declaration of \
         the project: {pending:?}"
    );

    // The roles the playbook uses are in the project, and the graph cannot
    // name them yet: they are published, with their origin.
    let roles: Vec<&str> = pending
        .iter()
        .map(|reference| reference.target.as_str())
        .filter(|target| target.starts_with("ansible:role:"))
        .collect();
    assert_eq!(
        roles.len(),
        3,
        "the three roles the playbook uses are published as pending: {pending:?}"
    );
    assert!(
        pending
            .iter()
            .filter(|reference| reference.target.starts_with("ansible:role:"))
            .all(|reference| reference.file.ends_with("site.yml") && reference.line.is_some()),
        "a pending reference says which file writes it and on which line"
    );
}

#[tokio::test]
async fn the_cli_does_not_say_complete_when_a_reference_is_pending() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = fixtures::copy_fixture(dir.path(), "terraform-iac");
    let main = fixture.join("main.tf");

    let mut source = std::fs::read_to_string(&main).expect("the fixture is readable");
    source.push_str(
        "\nresource \"aws_s3_bucket_policy\" \"ghost_policy\" {\n  bucket = aws_s3_bucket.no_existe.id\n  policy = \"{}\"\n}\n",
    );
    std::fs::write(&main, &source).expect("the fixture is writable");

    // The path the CLI takes: the same facade, the same op and the same
    // presenter that `cogh graph full` runs.
    let analyzer = ServiceAnalyzer::new();
    let basis = AnalysisBasis::for_workspace(dir.path().to_path_buf());
    let outcome = Analyze::execute(
        &analyzer,
        &basis,
        AnalysisOp::GraphFull {
            path: dir.path().to_path_buf(),
            rebuild: true,
        },
    );

    assert_eq!(
        outcome.state(),
        OutcomeState::Partial,
        "a reference that was not tied to a declaration is not a complete answer"
    );

    let result = present_graph_full(&outcome, &dir.path().to_string_lossy(), true);
    let GraphResult::Full {
        status,
        pending_references,
        ..
    } = &result
    else {
        panic!("esperaba GraphResult::Full, obtuvo {result:?}")
    };

    assert_eq!(
        status, "partial",
        "the CLI says partial when a reference is pending"
    );
    assert!(
        pending_references
            .iter()
            .any(|reference| reference.target == GHOST),
        "the CLI publishes which reference is pending and why: {pending_references:?}"
    );
}

#[tokio::test]
async fn a_declaration_of_another_module_does_not_satisfy_a_reference() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = dir.path().join("project");
    std::fs::create_dir_all(project.join("app")).expect("mkdir app");
    std::fs::create_dir_all(project.join("net")).expect("mkdir net");

    // The variable is declared in one module...
    std::fs::write(
        project.join("net/main.tf"),
        "variable \"only_here\" {\n  default = \"10.0.0.0/16\"\n}\n",
    )
    .expect("write net");
    // ...and named by the other one. A module does not see the internals of
    // its sibling: the name is not declared where it is used.
    std::fs::write(
        project.join("app/main.tf"),
        "resource \"aws_s3_bucket\" \"b\" {\n  bucket = var.only_here\n}\n",
    )
    .expect("write app");

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();

    let answer = handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");

    let pending = answer
        .pending_references
        .expect("the pending references travel with the answer");

    assert!(
        pending
            .iter()
            .any(|reference| reference.target == "var.only_here"),
        "the declaration of the sibling module does not satisfy the \
         reference: it stays pending: {pending:?}"
    );
    assert_ne!(
        answer.status, "complete",
        "a reference that names a declaration of another module is a hole, \
         and the answer says so"
    );

    // And no edge was invented for it.
    let graph = ctx.analysis_service().get_project_graph();
    assert!(
        !graph
            .all_dependencies()
            .any(|(_, target, _)| target.as_str().contains("only_here")),
        "a relation without a demonstrated far end is not invented"
    );
}

#[tokio::test]
async fn a_name_that_two_declarations_carry_is_ambiguous() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).expect("mkdir project");

    // Two declarations of the same name in the same scope: the graph cannot
    // prove which one the reference means.
    std::fs::write(
        project.join("first.tf"),
        "variable \"dup\" {\n  default = \"one\"\n}\n",
    )
    .expect("write first");
    std::fs::write(
        project.join("second.tf"),
        "variable \"dup\" {\n  default = \"two\"\n}\n",
    )
    .expect("write second");
    std::fs::write(
        project.join("uses.tf"),
        "resource \"aws_s3_bucket\" \"b\" {\n  bucket = var.dup\n}\n",
    )
    .expect("write uses");

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();

    let answer = handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");

    let pending = answer
        .pending_references
        .expect("the pending references travel with the answer");

    let ambiguous = pending
        .iter()
        .find(|reference| reference.target == "var.dup")
        .expect("the reference that two declarations carry is published");

    assert_eq!(
        ambiguous.reason,
        PendingReason::Ambiguous,
        "two declarations carry the name and the graph cannot tell them apart"
    );
    assert_eq!(
        ambiguous.candidates,
        Some(2),
        "the answer says how many declarations carry the name"
    );
    assert_ne!(
        answer.status, "complete",
        "an ambiguous reference is a hole, and the answer says so"
    );

    // And it was not wired to one of the homonyms.
    let graph = ctx.analysis_service().get_project_graph();
    assert!(
        !graph
            .all_dependencies()
            .any(|(_, target, _)| target.as_str().contains("dup")),
        "an ambiguous reference is never wired to a homonym"
    );
}

#[tokio::test]
async fn a_name_that_a_for_expression_binds_is_not_a_gap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).expect("mkdir project");

    std::fs::write(
        project.join("variables.tf"),
        "variable \"pairs\" {\n  type = map(object({ name = string }))\n  default = {}\n}\n",
    )
    .expect("write variables");
    // The expression binds two names, `k` and `v`. Both name no
    // declaration of the project, so neither of them is a hole.
    std::fs::write(
        project.join("main.tf"),
        "resource \"aws_s3_bucket\" \"b\" {\n  bucket = [for k, v in var.pairs : v.name]\n}\n",
    )
    .expect("write main");

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();

    let answer = handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");

    let pending = answer
        .pending_references
        .expect("the pending references travel with the answer");

    assert!(
        pending.is_empty(),
        "the names a `for` expression binds are not references to a \
         declaration: {pending:?}"
    );
    assert_eq!(
        answer.status, "complete",
        "a project that binds its own names is a complete answer"
    );
}

#[tokio::test]
async fn a_declaration_of_the_parent_module_does_not_satisfy_a_reference() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = dir.path().join("project");
    std::fs::create_dir_all(project.join("app")).expect("mkdir app");
    std::fs::create_dir_all(&project).expect("mkdir project");

    // The variable belongs to the parent module...
    std::fs::write(
        project.join("variables.tf"),
        "variable \"only_here\" {\n  default = \"10.0.0.0/16\"\n}\n",
    )
    .expect("write parent");
    // ...and the child module names it without declaring it. A module has
    // its own namespace: a parent hands its values down by filling the
    // child's own variables, never by exposing its own.
    std::fs::write(
        project.join("app/main.tf"),
        "resource \"aws_s3_bucket\" \"b\" {\n  bucket = var.only_here\n}\n",
    )
    .expect("write child");

    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .build();

    let answer = handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");

    let pending = answer
        .pending_references
        .expect("the pending references travel with the answer");

    assert!(
        pending
            .iter()
            .any(|reference| reference.target == "var.only_here"),
        "the declaration of the parent module does not satisfy the \\
         reference: the child names something it does not declare: {pending:?}"
    );
    assert_ne!(
        answer.status, "complete",
        "a reference that names a declaration of another module is a hole, \\
         and the answer says so"
    );
}
