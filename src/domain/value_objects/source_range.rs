//! SourceRange - Value object representing a range in source code

use crate::domain::value_objects::location::Location;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a range in source code defined by start and end locations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceRange {
    /// Start position (inclusive)
    start: Location,
    /// End position (exclusive)
    end: Location,
}

impl SourceRange {
    /// Creates a new SourceRange from start and end locations.
    pub fn new(start: Location, end: Location) -> Self {
        Self { start, end }
    }

    /// Returns the start location.
    pub fn start(&self) -> &Location {
        &self.start
    }

    /// Returns the end location.
    pub fn end(&self) -> &Location {
        &self.end
    }

    /// Returns the number of lines covered by this range.
    pub fn line_count(&self) -> u32 {
        if self.start.line() == self.end.line() {
            1
        } else {
            self.end.line() - self.start.line() + 1
        }
    }

    /// Returns true if this range is empty (start equals end).
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns the start byte offset (approximation).
    pub fn start_offset(&self) -> u32 {
        self.start.line() * 100 + self.start.column()
    }

    /// Returns the end byte offset (approximation).
    pub fn end_offset(&self) -> u32 {
        self.end.line() * 100 + self.end.column()
    }
}

impl fmt::Display for SourceRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {}", self.start, self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_range_creation() {
        let start = Location::new("test.rs", 1, 5);
        let end = Location::new("test.rs", 3, 10);
        let range = SourceRange::new(start.clone(), end.clone());
        assert_eq!(range.start(), &start);
        assert_eq!(range.end(), &end);
    }

    #[test]
    fn test_source_range_line_count() {
        let start = Location::new("test.rs", 1, 5);
        let end = Location::new("test.rs", 3, 10);
        let range = SourceRange::new(start, end);
        assert_eq!(range.line_count(), 3);
    }

    #[test]
    fn test_source_range_same_line() {
        let start = Location::new("test.rs", 1, 5);
        let end = Location::new("test.rs", 1, 10);
        let range = SourceRange::new(start, end);
        assert_eq!(range.line_count(), 1);
    }
}