//! Test that #[newtype] on a tuple struct with multiple fields fails.
//!
//! The #[newtype] macro only works on newtype structs with exactly one field.
use cognicode_macros::newtype;

#[newtype]
pub struct Point(f64, f64);

fn main() {}
