//! A declaration knows where it is.
//!
//! The reader of a language knows where the thing it declares is written,
//! and the graph is where the tools that ask about the declaration look. A
//! declaration that carries no place is published in the first line of its
//! file, whatever line it is really written on.

use cognicode_core::application::dto::SymbolDto;
use cognicode_core::application::ingest::types::SemanticOutcome;
use cognicode_core::infrastructure::graph::semantic_layer::semantic_facts;
use cognicode_core::infrastructure::parser::Language;

/// The corpus: the files the product's own readers are asked about.
fn corpus() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/identity_corpus")
}

/// What the consumer receives about each declaration of one file.
fn published_declarations(language: Language, file: &str) -> Vec<(String, u32)> {
    let source = std::fs::read_to_string(corpus().join(file)).expect("the corpus file is readable");
    let facts = match semantic_facts(language, &source, file) {
        SemanticOutcome::Handled(facts) => facts,
        other => panic!("the reader declares the file: {other:?}"),
    };
    facts
        .symbols
        .iter()
        .map(|symbol| {
            let published = SymbolDto::from_symbol(symbol);
            (published.name, published.line)
        })
        .collect()
}

/// The line one declaration is published on.
fn line_of(declarations: &[(String, u32)], name: &str) -> u32 {
    declarations
        .iter()
        .find(|(declaration, _)| declaration == name)
        .unwrap_or_else(|| panic!("`{name}` is published; the file declares {declarations:?}"))
        .1
}

/// A Terraform declaration is published where it is written.
#[test]
fn a_terraform_declaration_is_published_where_it_is_written() {
    let declarations = published_declarations(Language::Hcl, "main.tf");

    let bucket = line_of(&declarations, "aws_s3_bucket.app_bucket");
    let policy = line_of(&declarations, "aws_s3_bucket_policy.app_policy");

    assert_eq!(
        bucket, 1,
        "the bucket is written on the first line of the file"
    );
    assert_eq!(
        policy, 5,
        "the policy is written on the fifth line of the file"
    );
}

/// An Ansible declaration is published where it is written.
#[test]
fn an_ansible_declaration_is_published_where_it_is_written() {
    let declarations = published_declarations(Language::Yaml, "site.yml");

    let play = line_of(&declarations, "Configure infrastructure");
    let task = line_of(&declarations, "Install the package");

    assert_eq!(play, 1, "the play is written on the first line of the file");
    assert_eq!(
        task, 6,
        "the task is written on the sixth line of the file, inside the play"
    );
}

/// Two declarations of one file are not in the same place.
#[test]
fn two_declarations_of_one_file_are_not_in_the_same_place() {
    let declarations = published_declarations(Language::Hcl, "main.tf");

    let bucket = line_of(&declarations, "aws_s3_bucket.app_bucket");
    let policy = line_of(&declarations, "aws_s3_bucket_policy.app_policy");

    assert_ne!(
        bucket, policy,
        "a declaration published in the first line of the file says nothing \
         about where it is"
    );
}
