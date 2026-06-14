//! Test that #[newtype] on a regular struct fails to compile.
//!
//! The #[newtype] macro only works on newtype structs (tuple structs with exactly one field).
use cognicode_macros::newtype;

#[newtype]
pub struct User {
    id: i64,
}

fn main() {}
