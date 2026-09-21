//! Equivalence oracle fixture (F2.W3, R4) — broken syntax.
//!
//! Scenario 7. The closing `)` and the function body are missing.
//! After F2.W2 the `per_file` strategy must surface this in its
//! `SkippedFile`s; `full` (which has not been fixed) silently
//! ignores the file.

pub fn oops(
