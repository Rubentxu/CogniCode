//! The infrastructure query answers with the edge it actually has.
//!
//! The tool that answers questions about infrastructure used to answer with
//! three things it did not measure: the dependents were always published as
//! `references` with a confidence of `1.0`, the depth was a cap on how many
//! results came back rather than how far the walk went, and a name that
//! answered for more than one declaration was resolved by taking the first
//! one it found.
//!
//! ## What this file pins
//!
//! That the relation carries the metadata of the edge that exists, that the
//! depth is a distance and not a quota, and that an ambiguous name is
//! declared ambiguous instead of being resolved by chance.
//!
//! The assertions read the published answer as JSON rather than as a Rust
//! struct: what a consumer receives is the bytes, and a shape that hides
//! the difference between a measured value and a constant is exactly what
//! this file is about.

use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::traits::GraphStore;
use cognicode_core::domain::value_objects::declaration::Declaration;
use cognicode_core::domain::value_objects::dependency_type::DependencyType;
use cognicode_core::domain::value_objects::location::Location;
use cognicode_core::domain::value_objects::symbol_kind::SymbolKind;
use cognicode_core::infrastructure::persistence::InMemoryGraphStore;
use cognicode_core::interface::mcp::handlers::consolidated_handlers::{
    IacQueryInput, IacQueryOutput, handle_iac_query,
};
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};
use serde_json::Value;

/// The real fixture, copied into a scratch directory so the repository
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

/// A symbol that declares infrastructure, named the way the language
/// names it.
///
/// The identity is the one its reader gave it: that is what the query
/// looks the declaration up by.
fn symbol(fqn: &str, name: &str, type_name: &str) -> Symbol {
    let mut symbol = Symbol::new(
        name.to_string(),
        SymbolKind::Module,
        Location::new("/repo/main.tf", 1, 0),
    )
    .with_declaration(Declaration::with_type("resource", type_name));
    symbol.set_fqn_override(fqn);
    symbol
}

/// The published answer, as the consumer receives it.
fn published(answer: &IacQueryOutput) -> Value {
    serde_json::to_value(answer).expect("the answer is serialisable")
}

/// The relations the answer publishes.
fn relations(answer: &IacQueryOutput) -> Vec<Value> {
    let payload = published(answer);
    let mut all: Vec<Value> = payload["dependencies"]
        .as_array()
        .expect("dependencies is a list")
        .clone();
    all.extend(
        payload["dependents"]
            .as_array()
            .expect("dependents is a list")
            .clone(),
    );
    all
}

/// How many relations the answer carries.
fn relation_count(answer: &IacQueryOutput) -> usize {
    answer.dependencies.len() + answer.dependents.len()
}

/// The canonical identity of a declaration of the fixture.
///
/// The graph identifies a declaration by the file it was read from and
/// the name the reader gave it; that pair is what removes the doubt when
/// a name answers for more than one declaration.
fn canonical(dir: &std::path::Path, file: &str, name: &str) -> String {
    let root = dir.canonicalize().expect("the scratch directory exists");
    format!("tf:{}:{}", root.join(file).display(), name)
}

#[tokio::test]
async fn a_dependent_carries_the_metadata_of_the_edge_that_points_at_it() {
    // A graph whose edge was not observed but inferred: the edge says
    // so, and a tool that answers with the edge has to say it too.
    let mut graph = CallGraph::new();
    let bucket = graph.add_symbol(symbol(
        "tf:/repo/main.tf:aws_s3_bucket.app_bucket",
        "aws_s3_bucket.app_bucket",
        "aws_s3_bucket",
    ));
    let versioning = graph.add_symbol(symbol(
        "tf:/repo/main.tf:aws_s3_bucket_versioning.app_bucket_versioning",
        "aws_s3_bucket_versioning.app_bucket_versioning",
        "aws_s3_bucket_versioning",
    ));
    graph
        .add_dependency_with_provenance(
            &versioning,
            &bucket,
            DependencyType::References,
            ExtractionContext::Heuristic { score: 0.7 },
        )
        .expect("the edge is added");

    let store = InMemoryGraphStore::new();
    store.save_graph(&graph).expect("the graph is stored");
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = HandlerContext::builder()
        .with_working_dir(dir.path())
        .with_graph_store(store)
        .build();

    let answer = handle_iac_query(
        &ctx,
        IacQueryInput {
            resource_id: "aws_s3_bucket.app_bucket".to_string(),
            depth: 1,
        },
    )
    .await
    .expect("the consumer finds the resource");

    let payload = published(&answer);
    let dependent = &payload["dependents"][0];

    assert_eq!(
        dependent["edge_type"], "references",
        "the dependent carries the type of the edge that exists"
    );
    assert_eq!(
        dependent["confidence"], 0.7,
        "the dependent carries the confidence of the edge that exists, not a certainty that was invented"
    );
    assert_eq!(
        dependent["provenance"], "inferred",
        "the dependent carries the provenance of the edge that exists"
    );
}

#[tokio::test]
async fn a_depth_is_a_distance_and_not_a_quota() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = graph_of(dir.path(), "terraform-iac").await;

    let at_one = handle_iac_query(
        &ctx,
        IacQueryInput {
            resource_id: "module.compute".to_string(),
            depth: 1,
        },
    )
    .await
    .expect("the consumer finds the resource");

    let at_two = handle_iac_query(
        &ctx,
        IacQueryInput {
            resource_id: "module.compute".to_string(),
            depth: 2,
        },
    )
    .await
    .expect("the consumer finds the resource");

    let at_three = handle_iac_query(
        &ctx,
        IacQueryInput {
            resource_id: "module.compute".to_string(),
            depth: 3,
        },
    )
    .await
    .expect("the consumer finds the resource");

    assert!(
        relation_count(&at_two) > relation_count(&at_one),
        "walking two hops reaches further than walking one: {} vs {}",
        relation_count(&at_two),
        relation_count(&at_one)
    );
    assert!(
        relation_count(&at_three) >= relation_count(&at_two),
        "walking three hops does not reach less far than walking two: {} vs {}",
        relation_count(&at_three),
        relation_count(&at_two)
    );
}

#[tokio::test]
async fn a_name_that_answers_for_more_than_one_declaration_is_ambiguous() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = graph_of(dir.path(), "terraform-iac").await;

    // The fixture declares `var.environment` in three files: the root
    // one and the one of each module. A name that answers for more than
    // one declaration has no single answer, and the tool says so instead
    // of picking one of them.
    let answer = handle_iac_query(
        &ctx,
        IacQueryInput {
            resource_id: "var.environment".to_string(),
            depth: 1,
        },
    )
    .await;

    let error = match answer {
        Ok(_) => panic!("a name that answers for more than one declaration is ambiguous, not resolved in silence"),
        Err(error) => error.to_string(),
    };

    assert!(
        error.contains("ambiguous"),
        "the error declares the ambiguity: {error}"
    );
    assert!(
        error.contains("tf:"),
        "the error offers the canonical identity that removes the doubt: {error}"
    );
}

#[tokio::test]
async fn the_canonical_identity_answers_even_when_the_name_is_ambiguous() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = graph_of(dir.path(), "terraform-iac").await;

    let answer = handle_iac_query(
        &ctx,
        IacQueryInput {
            resource_id: canonical(
                dir.path(),
                "terraform-iac/variables.tf",
                "var.environment",
            ),
            depth: 1,
        },
    )
    .await;

    let answer = answer.expect("the canonical identity answers without doubt");
    assert_eq!(
        answer.resource_type, "variable",
        "the canonical identity names the declaration it points at"
    );
}

#[tokio::test]
async fn the_relations_of_the_fixture_carry_the_metadata_of_the_edge() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = graph_of(dir.path(), "terraform-iac").await;

    let answer = handle_iac_query(
        &ctx,
        IacQueryInput {
            resource_id: "aws_s3_bucket.app_bucket".to_string(),
            depth: 1,
        },
    )
    .await
    .expect("the consumer finds the resource");

    let relations = relations(&answer);
    let dependent = relations
        .iter()
        .find(|relation| relation["name"] == "aws_s3_bucket_versioning.app_bucket_versioning")
        .expect("the bucket has the versioning resource as a dependent");

    assert_eq!(
        dependent["edge_type"], "references",
        "the dependent carries the type of the edge that exists"
    );
    assert_eq!(
        dependent["provenance"], "extracted",
        "the dependent carries the provenance of the edge that exists"
    );
    assert_eq!(
        dependent["confidence"], 1.0,
        "the dependent carries the confidence of the edge that exists"
    );
}
