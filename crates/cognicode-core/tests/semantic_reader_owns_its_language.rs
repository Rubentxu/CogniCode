//! A language that is not a program is read by a reader of its own.
//!
//! Terraform is not a program: a resource is not a function, a data source
//! is not a function, a module is not a function and an output is not a
//! function. The generic walker reads what a grammar gives it, and what it
//! gave for a block was a symbol whose name was the block's header —
//! `resource "aws_s3_bucket" "app_bucket"` published as one function
//! called "resource". The graph then said a thing about the project that
//! the file did not say, and the analysis of code counted a bucket as a
//! function of the project.
//!
//! The same is true of a playbook: a YAML document is data, and a mapping
//! is not a class.
//!
//! ## What this file pins
//!
//! That a block of a language with a reader of its own is published as the
//! declaration it is — a resource is a resource of type `aws_s3_bucket`, a
//! play is a play — and never as a symbol of programming. And that the
//! generic walker, which has nothing to say about these languages, says
//! nothing about them.
//!
//! The last part of the chain matters as much as the first: the kind a
//! consumer reads is not the one the reader wrote down but the one the
//! transport published. A declaration that the graph knows and the
//! transport renames is still a lie the consumer reads.

mod fixtures;

use cognicode_core::application::ingest::extractor::extract_file;
use cognicode_core::application::ingest::types::ExtractionResult;
use cognicode_core::domain::value_objects::{Declaration, Language, SymbolKind};
use cognicode_core::infrastructure::graph::semantic_layer::semantic_facts;
use cognicode_core::infrastructure::parser::language_config::{HCL_CONFIG, YAML_CONFIG};
use cognicode_core::interface::mcp::handlers::handle_get_all_symbols;
use cognicode_core::interface::mcp::schemas::GetAllSymbolsInput;

/// What the graph published for one node.
#[derive(Debug, PartialEq)]
struct Published {
    /// The identifier the graph gave the node.
    id: String,
    /// The name the graph gave the node.
    label: String,
    /// The coarse kind of the node, as the graph publishes it.
    kind: String,
    /// The kind of the declaration the node carries, when it carries one.
    declaration: Option<String>,
    /// The type the declaration carries, when it carries one.
    declaration_type: Option<String>,
}

/// What every node of the result published.
fn published(result: &ExtractionResult) -> Vec<Published> {
    result
        .nodes
        .iter()
        .map(|node| {
            let properties = node.properties.as_object();
            let text = |key: &str| {
                properties
                    .and_then(|properties| properties.get(key))
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            };
            Published {
                id: node.id.as_str().to_string(),
                label: node.label.clone(),
                kind: node.kind.as_str(),
                declaration: text(Declaration::KIND_PROPERTY),
                declaration_type: text(Declaration::TYPE_PROPERTY),
            }
        })
        .collect()
}

/// The labels every node published, for a diagnostic that says what is there.
fn labels(nodes: &[Published]) -> Vec<&str> {
    nodes.iter().map(|node| node.label.as_str()).collect()
}

/// The node published for a label.
fn node_with_label<'a>(nodes: &'a [Published], label: &str) -> Option<&'a Published> {
    nodes.iter().find(|node| node.label == label)
}

/// What each block of a Terraform configuration declares: the label it is
/// published under, the kind of the declaration, and the type it carries
/// when it carries one. A bucket is a resource of type `aws_s3_bucket`; a
/// variable declares no type and is a variable.
const TERRAFORM_DECLARATIONS: &[(&str, &str, Option<&str>)] = &[
    (
        "aws_s3_bucket.app_bucket",
        "resource",
        Some("aws_s3_bucket"),
    ),
    ("data.aws_ami.ubuntu", "data", Some("aws_ami")),
    ("var.region", "variable", None),
    ("module.network", "module", None),
    ("output.bucket_arn", "output", None),
];

/// What each block of a playbook declares.
const PLAYBOOK_DECLARATIONS: &[(&str, &str, Option<&str>)] = &[
    ("configure the servers", "play", None),
    ("install the package", "task", None),
];

const TERRAFORM: &str = r#"
resource "aws_s3_bucket" "app_bucket" {
  bucket = "x"
}

data "aws_ami" "ubuntu" {
  most_recent = true
}

variable "region" {
  default = "eu-west-1"
}

module "network" {
  source = "./network"
}

output "bucket_arn" {
  value = "arn"
}
"#;

const PLAYBOOK: &str = r#"
- hosts: all
  name: configure the servers
  tasks:
    - name: install the package
      apt:
        name: nginx
        state: present
"#;

/// A block is published as the declaration it names, and as nothing else.
fn assert_declares(nodes: &[Published], label: &str, kind: &str, resource_type: Option<&str>) {
    let node = node_with_label(nodes, label)
        .unwrap_or_else(|| panic!("`{label}` is published; the labels are {:?}", labels(nodes)));
    assert_eq!(
        node.kind, "declaration",
        "the reader of this language publishes `{label}` as {}; a block of a \
         language with a reader of its own is a declaration, not a symbol of \
         programming",
        node.kind,
    );
    assert_eq!(
        node.declaration.as_deref(),
        Some(kind),
        "`{label}` does not declare what the file says it declares",
    );
    assert_eq!(
        node.declaration_type.as_deref(),
        resource_type,
        "`{label}` does not carry the type the file gives it",
    );
}

#[test]
fn a_terraform_block_is_published_as_the_declaration_it_is() {
    let result = extract_file(
        &HCL_CONFIG,
        std::path::Path::new("main.tf"),
        TERRAFORM,
        "terraform",
    );
    let nodes = published(&result);

    for (label, kind, resource_type) in TERRAFORM_DECLARATIONS {
        assert_declares(&nodes, label, kind, *resource_type);
    }
}

#[test]
fn a_play_and_its_tasks_are_published_as_declarations() {
    let result = extract_file(
        &YAML_CONFIG,
        std::path::Path::new("site.yml"),
        PLAYBOOK,
        "playbook",
    );
    let nodes = published(&result);

    for (label, kind, resource_type) in PLAYBOOK_DECLARATIONS {
        assert_declares(&nodes, label, kind, *resource_type);
    }
}

#[test]
fn a_language_with_a_reader_of_its_own_publishes_no_symbol_of_programming() {
    // The file itself is a file, and that is the truth about it. Everything
    // else the reader published is a declaration.
    for (language, config, source, path, expected) in [
        (
            Language::Hcl,
            &HCL_CONFIG,
            TERRAFORM,
            "main.tf",
            TERRAFORM_DECLARATIONS.len(),
        ),
        (
            Language::Yaml,
            &YAML_CONFIG,
            PLAYBOOK,
            "site.yml",
            PLAYBOOK_DECLARATIONS.len(),
        ),
    ] {
        let result = extract_file(config, std::path::Path::new(path), source, "fixture");
        let nodes = published(&result);

        let declarations = nodes
            .iter()
            .filter(|node| node.declaration.is_some())
            .count();
        assert_eq!(
            declarations, expected,
            "{language:?} published {declarations} declarations and {expected} \
             were expected; a rule that counts nothing would pass without \
             checking anything",
        );

        for node in &nodes {
            if node.id == path {
                continue;
            }
            assert_eq!(
                node.kind, "declaration",
                "{language:?} publishes `{}` as {}; a language with a reader of \
                 its own publishes declarations, not symbols of programming",
                node.label, node.kind,
            );
        }
    }
}

#[test]
fn the_generic_walker_has_nothing_to_say_about_a_configuration() {
    // The two languages below are read by a reader of their own. The
    // generic walker is left with nothing to look for, which is why the
    // published symbols are the ones the language's own reader found.
    for language in [Language::Hcl, Language::Yaml] {
        let symbols = language.symbols();
        assert!(
            symbols.function_types.is_empty()
                && symbols.class_types.is_empty()
                && symbols.variable_types.is_empty()
                && symbols.call_types.is_empty()
                && symbols.import_types.is_empty(),
            "{language:?} is read by a reader of its own, and the generic walker \
             must not claim anything about it"
        );
    }
}

/// The graph carries the declaration as a declaration, and carries what it
/// declares.
///
/// The graph is what the tools ask. A declaration that the reader named
/// reaches it whole: the kind the reader gave and the type the file
/// carries, both of them.
#[test]
fn the_graph_carries_a_declaration_as_a_declaration() {
    let facts = semantic_facts(Language::Hcl, TERRAFORM, "main.tf")
        .expect("the reader of this language says something about it");

    for (label, kind, resource_type) in TERRAFORM_DECLARATIONS {
        let symbol = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name() == *label)
            .unwrap_or_else(|| {
                panic!(
                    "`{label}` is in the graph; the names are {:?}",
                    facts
                        .symbols
                        .iter()
                        .map(|symbol| symbol.name())
                        .collect::<Vec<_>>()
                )
            });

        assert_eq!(
            *symbol.kind(),
            SymbolKind::Declaration,
            "the graph carries `{label}` as {}; a declaration of infrastructure \
             is not a symbol of programming",
            symbol.kind().name(),
        );

        let declaration = symbol
            .declaration()
            .unwrap_or_else(|| panic!("`{label}` declares nothing"));
        assert_eq!(
            declaration.kind(),
            *kind,
            "the graph does not carry the kind the reader gave `{label}`"
        );
        assert_eq!(
            declaration.resource_type(),
            *resource_type,
            "the graph does not carry the type the file gives `{label}`"
        );
    }
}

/// The consumer reads a declaration, and never a function.
///
/// This is the last link of the chain: the reader names what the block
/// declares, the graph carries it, and the transport publishes the kind
/// the consumer reads. A consumer that asks about a bucket has to be told
/// that the bucket is a bucket and not a function of the project.
#[tokio::test]
async fn the_consumer_reads_a_declaration_and_never_a_function() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("main.tf"), TERRAFORM).expect("the configuration is written");
    let ctx = fixtures::graph_of_dir(dir.path()).await;

    let answer = handle_get_all_symbols(
        &ctx,
        GetAllSymbolsInput {
            limit: Some(100),
            offset: None,
        },
    )
    .await
    .expect("the consumer reads the symbols");

    for (label, _, _) in TERRAFORM_DECLARATIONS {
        let symbol = answer
            .symbols
            .iter()
            .find(|symbol| symbol.name == *label)
            .unwrap_or_else(|| {
                panic!(
                    "`{label}` is in the answer; the names are {:?}",
                    answer
                        .symbols
                        .iter()
                        .map(|symbol| symbol.name.as_str())
                        .collect::<Vec<_>>()
                )
            });

        let published = serde_json::to_value(&symbol.kind).expect("the kind is published");
        assert_eq!(
            published,
            serde_json::json!("declaration"),
            "the consumer reads `{label}` as {published}; a declaration of \
             infrastructure is not a symbol of programming"
        );
    }
}
