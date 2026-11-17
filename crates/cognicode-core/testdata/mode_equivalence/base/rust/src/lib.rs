//! Demo crate used as an analysis corpus.

pub mod shapes;

pub fn greet(name: &str) -> String {
    format!("hello {name}")
}

pub fn shout(name: &str) -> String {
    greet(name).to_uppercase()
}

pub fn total(values: &[i64]) -> i64 {
    values.iter().sum()
}
