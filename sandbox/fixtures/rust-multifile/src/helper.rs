//! Helper file for multifile fixture testing.
//!
//! Contains standalone functions that don't call each other.
//! Useful for testing merge of independent graphs.

/// Standalone helper function 1.
pub fn helper_a(x: i32) -> i32 {
    x + 1
}

/// Standalone helper function 2.
pub fn helper_b(x: i32) -> i32 {
    x * 2
}

/// Standalone helper function 3.
pub fn helper_c(x: i32) -> i32 {
    x + 3
}
