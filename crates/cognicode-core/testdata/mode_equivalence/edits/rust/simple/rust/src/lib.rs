//! Demo crate used as an analysis corpus.
//! A comment-only and body-only edit: no definition changes.

pub mod shapes;

pub fn greet(name: &str) -> String {
    // The greeting is built here.
    format!("hello, {name}")
}

pub fn shout(name: &str) -> String {
    greet(name).to_uppercase()
}

pub fn total(values: &[i64]) -> i64 {
    values.iter().sum()
}
