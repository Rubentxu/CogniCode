//! Location - Value object representing a location in source code
//!
//! ## The base of a line
//!
//! Two representations of a position live in the product, and they do
//! not share a base:
//!
//!   - The **graph's** positions are zero-indexed, because that is
//!     what the parser that reads a file hands out.
//!   - The **code-intelligence** positions — this `Location` — are
//!     one-indexed, because that is the base the product publishes:
//!     the line a person reads in their editor.
//!
//! The gap between the two is closed in exactly one place:
//! [`published_line`]. Every surface that publishes a graph position
//! goes through it, and the code-intelligence paths carry the
//! published base already.
//!
//! A second copy of that arithmetic is a second truth about where a
//! line is. It was there, and it is why one product published the
//! line of the same symbol with two bases: a consumer that compared
//! two tools of the same surface got two different positions and
//! nothing told it which was which.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The base of every line the product publishes: one.
///
/// The line a person reads in their editor is the line one, and every
/// surface that publishes a line publishes it that way.
pub const PUBLISHED_LINE_BASE: u32 = 1;

/// The line as the product publishes it.
///
/// The graph's positions are zero-indexed and this is where they
/// become the line a consumer reads. It is the only place the
/// arithmetic lives.
pub fn published_line(internal_line: u32) -> u32 {
    internal_line + PUBLISHED_LINE_BASE
}

/// Represents a location in source code defined by file path, line, and column.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Location {
    /// Path to the source file
    file: String,
    /// The line of the position.
    ///
    /// The code-intelligence paths carry the base the product
    /// publishes — one-indexed — because that is the line a person
    /// reads. The graph's positions are zero-indexed, and they reach
    /// a consumer through [`published_line`].
    line: u32,
    /// Zero-indexed column number
    column: u32,
}

impl Location {
    /// Creates a new Location with the given file, line, and column.
    pub fn new(file: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }

    /// Returns the file path.
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the line number.
    pub fn line(&self) -> u32 {
        self.line
    }

    /// Returns the column number (zero-indexed).
    pub fn column(&self) -> u32 {
        self.column
    }

    /// Returns the fully qualified name (file:line:column format).
    pub fn fully_qualified_name(&self) -> String {
        format!("{}:{}:{}", self.file, self.line, self.column)
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

/// Error type for location parsing errors.
#[derive(Debug, thiserror::Error)]
pub enum ParseLocationError {
    #[error("Invalid location string format: {0}")]
    InvalidFormat(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_location_creation() {
        let loc = Location::new("test.rs", 10, 5);
        assert_eq!(loc.file(), "test.rs");
        assert_eq!(loc.line(), 10);
        assert_eq!(loc.column(), 5);
    }

    #[test]
    fn test_location_display() {
        let loc = Location::new("test.rs", 10, 5);
        assert_eq!(format!("{}", loc), "test.rs:10:5");
    }

    #[test]
    fn test_location_fully_qualified_name() {
        let loc = Location::new("test.rs", 10, 5);
        assert_eq!(loc.fully_qualified_name(), "test.rs:10:5");
    }

    #[test]
    fn the_published_line_is_the_line_a_person_reads() {
        // The graph's first line is its zero; the line a person reads
        // is one. The conversion is the only place the arithmetic
        // lives, and it is the same for every caller.
        assert_eq!(published_line(0), 1);
        assert_eq!(published_line(41), 42);
    }
}
