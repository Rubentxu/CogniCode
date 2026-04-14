//! Utility functions for indexing test fixture.

use std::collections::HashMap;

/// Process a value (used to test cross-function calls).
pub fn process(value: i32) -> String {
    format!("Processed: {}", value)
}

/// Format a result with additional metadata.
pub fn format_result(value: i32, label: &str) -> String {
    format!("{}: {}", label, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process() {
        assert_eq!(process(42), "Processed: 42");
    }

    #[test]
    fn test_format_result() {
        assert_eq!(format_result(10, "Answer"), "Answer: 10");
    }
}
