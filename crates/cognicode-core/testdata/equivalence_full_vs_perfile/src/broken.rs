//! Equivalence oracle fixture — broken syntax.
//!
//! Scenario 7. The closing `)` and the function body are missing.
//! The `per_file` strategy must surface this in its `SkippedFile`s;
//! `full` (which has not been fixed) silently ignores the file.

pub fn oops(
