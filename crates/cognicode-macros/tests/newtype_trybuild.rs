//! Trybuild tests for the `#[newtype]` proc macro.
//!
//! This module runs compile-fail tests to verify the macro rejects invalid inputs:
//! - Non-tuple structs
//! - Enums
//! - Tuple structs with multiple fields
//! - Invalid derive syntax

use std::path::PathBuf;

/// Run all UI compile-fail tests for the `#[newtype]` macro
///
/// These tests verify that the macro correctly rejects invalid inputs:
/// - Non-tuple structs
/// - Enums
/// - Tuple structs with multiple fields
#[test]
fn test_newtype_ui_compile_fail() {
    let ui_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("newtype-ui");

    let glob_pattern = ui_dir.join("*.rs");

    trybuild::TestCases::new().compile_fail(glob_pattern.to_str().unwrap());
}
