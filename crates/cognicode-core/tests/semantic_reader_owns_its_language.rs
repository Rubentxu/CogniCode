//! A language that is not a program is read by a reader of its own.
//!
//! Terraform is not a program: a resource is not a function, a data source
//! is not a function, a module is not a function and an output is not a
//! function. The generic walker reads what a grammar gives it, and what it
//! gave for a block was a symbol whose name was the block's header —
//! `resource "aws_s3_bucket" "app_bucket"` published as one function
//! called "resource". The graph then said a thing about the project that
//! the file did not say.
//!
//! The same is true of a playbook: a YAML document is data, and a mapping
//! is not a class.
//!
//! ## What this file pins
//!
//! That the class a language publishes is the one its own reader publishes:
//! a resource is a resource, a data source is a binding, a module is a
//! module, and a play is a play. And that the generic walker, which has
//! nothing to say about these languages, says nothing about them.

use cognicode_core::application::ingest::extractor::extract_file;
use cognicode_core::application::ingest::types::ExtractionResult;
use cognicode_core::domain::value_objects::{Language, SymbolKind};
use cognicode_core::infrastructure::parser::language_config::{HCL_CONFIG, YAML_CONFIG};
use std::collections::BTreeMap;

/// The published symbols of a file, by label.
fn published(result: &ExtractionResult) -> BTreeMap<String, SymbolKind> {
    result
        .nodes
        .iter()
        .filter_map(|node| {
            let kind = match &node.kind {
                cognicode_core::domain::value_objects::NodeKind::Symbol(kind) => Some(*kind),
                _ => None,
            }?;
            Some((node.label.clone(), kind))
        })
        .collect()
}

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

#[test]
fn a_terraform_block_is_published_as_what_it_is() {
    let result = extract_file(
        &HCL_CONFIG,
        std::path::Path::new("main.tf"),
        TERRAFORM,
        "terraform",
    );
    let published = published(&result);

    let expected = [
        ("aws_s3_bucket.app_bucket", SymbolKind::Function),
        ("data.aws_ami.ubuntu", SymbolKind::Variable),
        ("var.region", SymbolKind::Variable),
        ("module.network", SymbolKind::Module),
        ("output.bucket_arn", SymbolKind::Property),
    ];

    for (label, kind) in expected {
        assert_eq!(
            published.get(label),
            Some(&kind),
            "the reader of this language publishes `{label}` as {kind:?}; \
             it published {:?}",
            published.get(label)
        );
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

#[test]
fn a_play_is_published_as_a_play_and_its_tasks_as_tasks() {
    let result = extract_file(
        &YAML_CONFIG,
        std::path::Path::new("site.yml"),
        PLAYBOOK,
        "playbook",
    );
    let published = published(&result);

    let plays: Vec<&String> = published
        .keys()
        .filter(|l| l.contains("configure the servers"))
        .collect();
    assert!(
        !plays.is_empty(),
        "the play is published; the labels are {published:?}"
    );

    let tasks: Vec<&String> = published
        .keys()
        .filter(|l| l.contains("install the package"))
        .collect();
    assert!(
        !tasks.is_empty(),
        "the task is published; the labels are {published:?}"
    );
}
