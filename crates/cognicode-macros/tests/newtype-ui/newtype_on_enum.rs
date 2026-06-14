//! Test that #[newtype] on an enum fails to compile.
//!
//! The #[newtype] macro only works on newtype structs, not enums.
use cognicode_macros::newtype;

#[newtype]
pub enum Status {
    Active,
    Inactive,
}

fn main() {}
