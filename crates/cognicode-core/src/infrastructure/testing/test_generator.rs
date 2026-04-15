//! Test generator infrastructure

use crate::domain::aggregates::symbol::Symbol;

/// Generates test code for symbols
pub struct TestGenerator;

impl TestGenerator {
    /// Creates a new test generator
    pub fn new() -> Self {
        Self
    }

    /// Generates a test for a function symbol
    pub fn generate_function_test(&self, symbol: &Symbol) -> String {
        let name = symbol.name();

        format!(
            r#"#[cfg(test)]
mod {} {{
    #[test]
    fn test_{}() {{
        // TODO: Implement test
    }}
}}"#,
            name, name
        )
    }

    /// Generates a test for a class symbol
    pub fn generate_class_test(&self, symbol: &Symbol) -> String {
        let name = symbol.name();

        format!(
            r#"#[cfg(test)]
mod {} {{
    #[test]
    fn test_creation() {{
        // TODO: Implement test for class {{}}
    }}
}}"#,
            name
        )
    }
}

impl Default for TestGenerator {
    fn default() -> Self {
        Self::new()
    }
}
