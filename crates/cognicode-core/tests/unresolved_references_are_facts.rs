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

use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};
use cognicode_core::interface::mcp::schemas::PendingReason;
use std::path::{Path, PathBuf};

/// The real fixture, copied into a scratch directory so the repository
/// stays clean. The content is the versioned one: these tests run against
/// what the repository really holds, not against a toy.
fn copy_fixture(dir: &Path, name: &str) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures")
        .join(name);
    let target = dir.join(name);
    copy_tree(&source, &target);
    target
}

fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).expect("mkdir");
    for entry in std::fs::read_dir(source).expect("read fixture") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &destination);
        } else {
            std::fs::copy(&path, &destination).expect("copy fixture");
        }
    }
}

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
    let fixtures = copy_fixture(dir.path(), "terraform-iac");
    let main = fixtures.join("main.tf");

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
    let fixtures = copy_fixture(dir.path(), "terraform-iac");
    let variables = fixtures.join("variables.tf");

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
    copy_fixture(dir.path(), "terraform-iac");

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
    copy_fixture(dir.path(), "ansible-playbook");

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
