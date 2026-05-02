//! Location - Value object representing a location in source code

use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a location in source code defined by file path, line, and column.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Location {
    /// Path to the source file
    file: String,
    /// Zero-indexed line number
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

    /// Returns the line number (zero-indexed).
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
