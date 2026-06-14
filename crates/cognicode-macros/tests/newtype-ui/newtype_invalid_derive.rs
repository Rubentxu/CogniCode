//! Test that invalid derive syntax in #[newtype(derive(...))] fails.
//!
//! The derive clause should contain valid derive traits.
use cognicode_macros::newtype;

#[newtype(derive(NotAValidTrait))]
pub struct UserId(i64);

fn main() {}
