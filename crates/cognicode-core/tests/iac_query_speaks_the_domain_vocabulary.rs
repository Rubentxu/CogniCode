//! The infrastructure tool answers in the vocabulary of the domain.
//!
//! The graph knows what a Terraform block is: the reader that understands
//! the language declares it and the graph carries that declaration. The tool
//! that answers questions about infrastructure used to answer with the name
//! of the Rust type that happened to hold the declaration, so a consumer
//! asking about an S3 bucket was told the bucket was a `Function`.
//!
//! ## What this file pins
//!
//! That the answer names the declaration the way the language names it, that
//! the relations answer in the same vocabulary, and that no answer carries a
//! name that only exists inside the implementation.

use cognicode_core::interface::mcp::handlers::consolidated_handlers::{
    IacQueryInput, handle_iac_query,
};
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};

/// The real fixtures, copied into a scratch directory so the repository
/// stays clean. The content is the versioned one: this test runs against
/// what the repository really holds, not against a toy.
fn copy_fixture(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures")
        .join(name);
    let target = dir.join(name);
    copy_tree(&source, &target);
    target
}

fn copy_tree(source: &std::path::Path, target: &std::path::Path) {
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

/// The graph of a scratch directory that holds the given fixture.
async fn graph_of(dir: &std::path::Path, fixture: &str) -> HandlerContext {
    copy_fixture(dir, fixture);
    let ctx = HandlerContext::builder().with_working_dir(dir).build();
    handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("the graph builds");
    ctx
}

/// The names of the implementation that used to escape through the tool.
const IMPLEMENTATION_NAMES: [&str; 6] = [
    "Function",
    "Variable",
    "Module",
    "Property",
    "References",
    "Contains",
];

#[tokio::test]
async fn the_answer_names_the_resource_the_way_terraform_names_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = graph_of(dir.path(), "terraform-iac").await;

    let answer = handle_iac_query(
        &ctx,
        IacQueryInput {
            resource_id: "aws_s3_bucket.app_bucket".to_string(),
            depth: 2,
        },
    )
    .await
    .expect("the consumer finds the resource");

    assert_eq!(
        answer.resource_type, "aws_s3_bucket",
        "the answer names the type of the resource, not the kind of the type that holds it"
    );
}

#[tokio::test]
async fn a_declaration_without_a_type_answers_with_its_kind() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = graph_of(dir.path(), "terraform-iac").await;

    for (resource_id, expected) in [
        ("var.environment", "variable"),
        ("module.vpc", "module"),
        ("output.s3_bucket_name", "output"),
    ] {
        let answer = handle_iac_query(
            &ctx,
            IacQueryInput {
                resource_id: resource_id.to_string(),
                depth: 1,
            },
        )
        .await
        .expect("the consumer finds the declaration");

        assert_eq!(
            answer.resource_type, expected,
            "a declaration that names no type answers with the kind it declares"
        );
    }
}

#[tokio::test]
async fn the_relations_answer_in_the_same_vocabulary() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = graph_of(dir.path(), "terraform-iac").await;

    let answer = handle_iac_query(
        &ctx,
        IacQueryInput {
            resource_id: "aws_s3_bucket_versioning.app_bucket_versioning".to_string(),
            depth: 2,
        },
    )
    .await
    .expect("the consumer finds the resource");

    let bucket = answer
        .dependencies
        .iter()
        .find(|relation| relation.name == "aws_s3_bucket.app_bucket")
        .expect("the versioning resource depends on the bucket");

    assert_eq!(
        bucket.kind, "aws_s3_bucket",
        "a relation names the type of what it points at"
    );
    assert_eq!(
        bucket.edge_type, "references",
        "a relation names what it is in the vocabulary of the domain"
    );
}

#[tokio::test]
async fn an_ansible_play_answers_with_its_kind() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = graph_of(dir.path(), "ansible-playbook").await;

    let answer = handle_iac_query(
        &ctx,
        IacQueryInput {
            resource_id: "Configure infrastructure".to_string(),
            depth: 2,
        },
    )
    .await
    .expect("the consumer finds the play");

    assert_eq!(
        answer.resource_type, "play",
        "a play answers with the kind the reader gave it"
    );
}

#[tokio::test]
async fn no_answer_carries_a_name_that_only_exists_inside() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = graph_of(dir.path(), "terraform-iac").await;

    let answer = handle_iac_query(
        &ctx,
        IacQueryInput {
            resource_id: "aws_db_instance.postgres".to_string(),
            depth: 2,
        },
    )
    .await
    .expect("the consumer finds the resource");

    let mut published = vec![answer.resource_type.clone()];
    for relation in answer.dependencies.iter().chain(answer.dependents.iter()) {
        published.push(relation.kind.clone());
        published.push(relation.edge_type.clone());
    }

    for value in published {
        assert!(
            !IMPLEMENTATION_NAMES.contains(&value.as_str()),
            "the answer publishes `{value}`, which only exists inside the implementation"
        );
    }
}
