//! `find` compares the names the way the language reads them, and says so.
//!
//! ## The defect
//!
//! Measured on this repository (`main@f400088a`), `cognicode find --symbol Ok
//! --format json` answered with 67 declarations — among them
//! `crates/cognicode-cli/src/cli/doctor.rs:50`, which declares `fn ok` in
//! lower case. The index keyed its declarations by the lowercased name, so
//! the fold was the index's property and not the language's, and the answer
//! published every match under the name that was searched: the 67 read as
//! the 67 declarations of `Ok`, and one of them was not one.
//!
//! The capital of a name is meaning, not noise: `Ok` and `ok` are two names
//! in Rust, in Python and in JavaScript. Only a language that folds the case
//! —PHP, PowerShell, Fortran— reads them as one.
//!
//! ## What is measured
//!
//! - A language that tells the case apart answers with the declaration
//!   spelled the way the name was spelled, and publishes that spelling.
//! - The answer says how it compared the names.
//! - A language that folds the case keeps folding, and says that it did.

use cognicode_core::interface::goals::find::{GoalFind, find, render};
use std::path::Path;

/// A project whose two declarations differ only in the case.
fn rust_corpus(root: &Path) {
    std::fs::write(
        root.join("lib.rs"),
        "pub fn Ok() -> u8 { 1 }\n\npub fn ok() -> u8 { 2 }\n",
    )
    .expect("write");
}

/// A project in a language that does not tell the case apart: the same two
/// declarations, in a language that reads them as one name.
fn php_corpus(root: &Path) {
    std::fs::write(
        root.join("lib.php"),
        "<?php\nfunction Ok() { return 1; }\nfunction ok() { return 2; }\n",
    )
    .expect("write");
}

/// The answer of the goal, and the text it renders.
fn answer_of(root: &Path, symbol: &str) -> (GoalFind, String) {
    let answer = find(root, None, symbol).expect("the goal answers");
    let text = render(&serde_json::to_value(&answer).expect("the answer is serialisable"));
    (answer, text)
}

/// The names the answer publishes.
fn published_names(answer: &GoalFind) -> Vec<&str> {
    answer.locations.iter().map(|l| l.name.as_str()).collect()
}

/// The defect: a language that tells `Ok` from `ok` has two names in this
/// project, and the answer to "where is `Ok` declared" is the declaration
/// spelled `Ok` —not both, and not the other one published under the name
/// that was searched.
#[test]
fn a_name_that_differs_only_in_the_case_is_not_the_declaration_that_was_asked_for() {
    let tmp = tempfile::tempdir().expect("tempdir");
    rust_corpus(tmp.path());

    let (answer, _text) = answer_of(tmp.path(), "Ok");

    assert_eq!(
        answer.locations.len(),
        1,
        "Rust tells `Ok` from `ok`: the answer must name the declaration that \
         was searched, got {:?}",
        published_names(&answer)
    );
    assert_eq!(answer.locations[0].name, "Ok");
}

/// The answer has to say how it compared the names: a consumer that reads
/// the count of declarations has to know whether the count is the count of
/// the name it asked for.
#[test]
fn the_answer_says_how_it_compared_the_names() {
    let tmp = tempfile::tempdir().expect("tempdir");
    rust_corpus(tmp.path());

    let (_answer, text) = answer_of(tmp.path(), "Ok");

    assert!(
        text.contains("comparison: exact"),
        "the answer must say how it compared the names: {text}"
    );
}

/// The control: a language that folds the case answers with both spellings,
/// each one under its own name, and says that the comparison folded.
#[test]
fn a_language_that_folds_the_case_answers_with_both_spellings_and_says_so() {
    let tmp = tempfile::tempdir().expect("tempdir");
    php_corpus(tmp.path());

    let (answer, text) = answer_of(tmp.path(), "Ok");

    let names = published_names(&answer);
    assert_eq!(
        names.len(),
        2,
        "PHP folds the case: both declarations answer to `Ok`, got {names:?}"
    );
    assert!(
        names.contains(&"Ok") && names.contains(&"ok"),
        "each declaration publishes its own spelling, got {names:?}"
    );
    assert!(
        text.contains("comparison: case-insensitive"),
        "the answer must say that it folded the case: {text}"
    );
}

/// A project with two languages: a Rust declaration spelled `Ok` and a PHP
/// declaration spelled `ok`.
///
/// The answer mixes the two spellings —the PHP one answers only because
/// PHP folds the case— and says so. The word is about the answer, not
/// about each declaration: it says that the answer is not only made of
/// declarations spelled the way the name was searched.
#[test]
fn a_project_with_two_languages_says_that_its_answer_mixes_the_spellings() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(tmp.path().join("lib.rs"), "pub fn Ok() -> u8 { 1 }\n").expect("write");
    std::fs::write(
        tmp.path().join("lib.php"),
        "<?php\nfunction ok() { return 2; }\n",
    )
    .expect("write");

    let (answer, text) = answer_of(tmp.path(), "Ok");

    let names = published_names(&answer);
    assert_eq!(
        names.len(),
        2,
        "the Rust declaration and the PHP one both answer to `Ok`, got {names:?}"
    );
    assert!(
        names.contains(&"Ok") && names.contains(&"ok"),
        "each declaration publishes its own spelling, got {names:?}"
    );
    assert!(
        text.contains("comparison: case-insensitive"),
        "the answer mixes the spellings and must say so: {text}"
    );
}

/// An answer with no declarations says that the comparison was exact:
/// nothing was folded, because nothing answered.
#[test]
fn an_answer_without_declarations_says_that_the_comparison_was_exact() {
    let tmp = tempfile::tempdir().expect("tempdir");
    rust_corpus(tmp.path());

    let (answer, text) = answer_of(tmp.path(), "nowhere");

    assert!(answer.locations.is_empty());
    assert!(
        text.contains("comparison: exact"),
        "an empty answer says that nothing was folded: {text}"
    );
}
