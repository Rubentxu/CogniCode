//! The reader of a language says what happened when it reads a file.
//!
//! Three answers, and they are not the same thing. The reader declared
//! something. The reader does not apply to this file. The reader fell over.
//!
//! The third one used to be invisible: a reader that fell over left a file
//! without declarations behind, which is exactly what a file that declares
//! nothing leaves behind, and the consumer fell back to the generic reader
//! as if the file had simply said nothing. Now the failure is told apart
//! from the absence, and the consumer can say it.

use cognicode_core::application::ingest::types::{ExtractionResult, SemanticOutcome};
use cognicode_core::infrastructure::graph::semantic_layer::{semantic_facts, semantic_facts_of};
use cognicode_core::infrastructure::parser::Language;
use cognicode_core::infrastructure::parser::language_config::{HCL_CONFIG, LanguageConfig};

/// The corpus: the files the product's own readers are asked about.
fn corpus() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/identity_corpus")
}

/// The text of one file of the corpus.
fn source_of(file: &str) -> String {
    std::fs::read_to_string(corpus().join(file)).expect("the corpus file is readable")
}

/// A reader that cannot read.
///
/// The reader of a language is a function, and a function can be handed one
/// that falls over. This is that reader.
fn a_reader_that_falls_over(
    _source_path: &str,
    _source_hash: &str,
    _source: &str,
    _result: &ExtractionResult,
) -> ExtractionResult {
    panic!("this reader cannot read what it was handed");
}

/// A reader that falls over is not a file that declares nothing.
///
/// The consumer has to be able to tell the two apart, because only one of
/// them is a fact about the file.
#[test]
fn a_reader_that_falls_over_fails_with_its_cause() {
    let config = LanguageConfig {
        language: Language::Hcl,
        ts_language: HCL_CONFIG.ts_language,
        type_ref_walker: HCL_CONFIG.type_ref_walker,
        semantic_handler: Some(a_reader_that_falls_over),
    };

    match semantic_facts_of(&config, &source_of("main.tf"), "main.tf") {
        SemanticOutcome::Failed(cause) => assert!(
            cause.contains("this reader cannot read what it was handed"),
            "the failure carries the reason the reader gave: {cause}"
        ),
        other => panic!("a reader that fell over is not a file that declares nothing: {other:?}"),
    }
}

/// A reader that declares something handles the file.
#[test]
fn a_reader_that_declares_something_handles_the_file() {
    match semantic_facts(Language::Hcl, &source_of("main.tf"), "main.tf") {
        SemanticOutcome::Handled(facts) => assert!(
            !facts.symbols.is_empty(),
            "the reader of Terraform declares the blocks of the file, and the \
             file it read is a Terraform configuration"
        ),
        other => panic!("the reader of Terraform declares the blocks of the file: {other:?}"),
    }
}

/// A file the reader has nothing to say about is not its file to claim.
///
/// A YAML file that is not a playbook is a YAML file, and the generic
/// reader is the one that answers for it.
#[test]
fn a_file_the_reader_has_nothing_to_say_about_is_not_applicable() {
    match semantic_facts(Language::Yaml, &source_of("plain.yml"), "plain.yml") {
        SemanticOutcome::NotApplicable => {}
        other => panic!(
            "a YAML file that is not a playbook is not something the reader of \
             Ansible claims: {other:?}"
        ),
    }
}

/// A language without a reader of its own is not applicable either.
#[test]
fn a_language_without_a_reader_is_not_applicable() {
    match semantic_facts(Language::Rust, "pub fn a() {}\n", "lib.rs") {
        SemanticOutcome::NotApplicable => {}
        other => panic!("Rust has no reader of its own, and the generic one answers: {other:?}"),
    }
}
